use std::mem::size_of;
use std::time::Instant;

use physbox_contracts::{
    BodyCommandBatchV1, BodyCommandV1, BodyId, BodyStateV1, CapacityRecordV1, CoupledScenarioV1,
    NeutralActionV1, SCHEMA_VERSION, ShapeV1, Validate, Vec2, canonical_hash,
};
use physbox_launch::WorldCapacityPreset;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::body_state::BODY_COMMAND_CAPACITY;
use crate::body_world::{BodyCommandStepRange, BodyRenderPlan, BodyStepPlan, GpuBodyWorld};
use crate::construction::empty_construction_intent_batch;
use crate::gpu_context::{request_adapter, request_device};
use crate::grab::empty_grab_intent_batch;
use crate::particle_world::{GpuParticleDiagnosticsV4, ParticleValidationLayout};
use crate::renderer::{RenderFrameTarget, map_buffer, read_texture};
use crate::{
    AdapterDiagnostics, Camera2d, SLEEP_CYCLE_DIAGNOSTIC_BYTES, SleepCycleMetricsV1,
    SolverProfileConfig, create_instance,
};

pub const GPU_COUPLED_EVIDENCE_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCoupledEvidenceConfigV1 {
    pub viewport: [u32; 2],
    pub warmup_steps: u32,
    pub measured_steps: u32,
    pub steps_per_sample: u32,
    pub capture_body_trajectory: bool,
}

impl GpuCoupledEvidenceConfigV1 {
    pub fn validate(self) -> Result<(), String> {
        if self.viewport.into_iter().any(|value| value == 0)
            || self.measured_steps == 0
            || self.steps_per_sample == 0
            || self.steps_per_sample > 64
            || self.warmup_steps.checked_add(self.measured_steps).is_none()
        {
            return Err(
                "coupled evidence requires a nonzero viewport/step count and 1..=64 steps per sample"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCoupledTimingSummaryV1 {
    pub samples: u32,
    pub mean_seconds_per_outer_step: f64,
    pub p95_seconds_per_outer_step: f64,
    pub max_seconds_per_outer_step: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCoupledTimingV1 {
    pub method: &'static str,
    pub sample_outer_steps: Vec<u32>,
    pub cpu_encode_submit_seconds_per_outer_step: Vec<f64>,
    pub host_completion_seconds_per_outer_step: Vec<f64>,
    pub cpu_encode_submit: GpuCoupledTimingSummaryV1,
    pub host_completion: GpuCoupledTimingSummaryV1,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuParticleAggregateV1 {
    pub active_particles: u32,
    pub bounds: [f32; 4],
    pub centroid: Vec2,
    pub mean_velocity: Vec2,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCoupledBodyObservationV1 {
    pub state: BodyStateV1,
    pub nearby_particle_count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCoupledBodyTrajectoryV1 {
    pub body: BodyId,
    pub sample_count: u32,
    pub max_displacement_from_initial: f32,
    pub max_abs_angle_delta: f32,
    pub max_linear_speed: f32,
    pub max_abs_angular_speed: f32,
    pub first_wake_observed_step: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCoupledMechanicalStateV1 {
    pub particle_linear_momentum: Vec2,
    pub dynamic_body_linear_momentum: Vec2,
    pub particle_kinetic_energy: f32,
    pub dynamic_body_kinetic_energy: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCoupledExchangeV1 {
    pub initial: GpuCoupledMechanicalStateV1,
    pub final_state: GpuCoupledMechanicalStateV1,
    pub particle_linear_momentum_delta: Vec2,
    pub dynamic_body_linear_momentum_delta: Vec2,
    pub particle_kinetic_energy_delta: f32,
    pub dynamic_body_kinetic_energy_delta: f32,
}

impl GpuCoupledExchangeV1 {
    fn new(initial: GpuCoupledMechanicalStateV1, final_state: GpuCoupledMechanicalStateV1) -> Self {
        Self {
            particle_linear_momentum_delta: subtract(
                final_state.particle_linear_momentum,
                initial.particle_linear_momentum,
            ),
            dynamic_body_linear_momentum_delta: subtract(
                final_state.dynamic_body_linear_momentum,
                initial.dynamic_body_linear_momentum,
            ),
            particle_kinetic_energy_delta: final_state.particle_kinetic_energy
                - initial.particle_kinetic_energy,
            dynamic_body_kinetic_energy_delta: final_state.dynamic_body_kinetic_energy
                - initial.dynamic_body_kinetic_energy,
            initial,
            final_state,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCoupledEvidenceV2 {
    pub version: u32,
    pub source_scenario_id: String,
    pub source_scenario_hash: String,
    pub configured_projection_hash: String,
    pub solver_profile: &'static str,
    pub physics: physbox_launch::PhysicsConfigurationV3,
    pub adapter: AdapterDiagnostics,
    pub config: GpuCoupledEvidenceConfigV1,
    pub completed_steps: u32,
    pub initial_particles: GpuParticleAggregateV1,
    pub final_particles: GpuParticleAggregateV1,
    pub particle_centroid_displacement: f32,
    pub particle_diagnostics: GpuParticleDiagnosticsV4,
    pub bodies: Vec<GpuCoupledBodyObservationV1>,
    pub body_trajectories: Vec<GpuCoupledBodyTrajectoryV1>,
    pub exchange: GpuCoupledExchangeV1,
    pub sleep_cycle: Option<SleepCycleMetricsV1>,
    pub capacities: Vec<CapacityRecordV1>,
    pub timing: GpuCoupledTimingV1,
    pub exact_state_sha256: String,
    pub action_projection: &'static str,
    pub trajectory_sampling: &'static str,
    pub comparator_compiled: bool,
    pub blocking_readback_for_advancement: bool,
}

pub struct GpuCoupledEvidenceRun {
    pub evidence: GpuCoupledEvidenceV2,
    pub rgba8: Vec<u8>,
}

pub async fn run_gpu_coupled_evidence(
    source: CoupledScenarioV1,
    profile: SolverProfileConfig,
    config: GpuCoupledEvidenceConfigV1,
) -> Result<GpuCoupledEvidenceRun, String> {
    source.validate().map_err(|error| error.to_string())?;
    config.validate()?;
    let total_steps = config
        .warmup_steps
        .checked_add(config.measured_steps)
        .ok_or_else(|| "coupled evidence step count overflowed".to_owned())?;
    if total_steps > source.requested_steps {
        return Err(format!(
            "coupled evidence requests {total_steps} steps from source window {}",
            source.requested_steps
        ));
    }
    let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
    let mut configured = source.clone();
    profile.apply_to_scenario(&mut configured.rigid)?;
    let scheduled = project_grab_actions(&configured)?;
    configured.rigid.actions.clear();
    configured.validate().map_err(|error| error.to_string())?;
    let configured_projection_hash =
        canonical_hash(&configured).map_err(|error| error.to_string())?;

    let instance = create_instance();
    let adapter = request_adapter(&instance, None).await?;
    let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
    let (device, queue) = request_device(&adapter).await?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("physbox5_coupled_evidence_target"),
        size: wgpu::Extent3d {
            width: config.viewport[0],
            height: config.viewport[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let world = GpuBodyWorld::new_coupled_with_solver_profile(
        &device,
        format,
        &configured,
        WorldCapacityPreset::Slots64,
        &profile,
    )?;
    let initial_particle_bytes = read_particle_bytes(&device, &queue, &world)?;
    let initial_particles = particle_aggregate(
        world.particle_validation_layout().unwrap(),
        &initial_particle_bytes,
    )?;
    let initial_body_bytes = read_body_bytes(&device, &queue, &world)?;
    let initial_body_states = decode_body_states(
        &configured.rigid.bodies,
        world.body_capacity(),
        &initial_body_bytes,
    )?;
    let camera = evidence_camera(&configured, config.viewport)?;

    let mut completed_steps = 0u32;
    let mut body_samples = vec![(0, initial_body_states.clone())];
    submit_samples(
        &device,
        &queue,
        &world,
        &view,
        config.viewport,
        camera,
        config.warmup_steps,
        config.steps_per_sample,
        &scheduled,
        &mut completed_steps,
        None,
        &configured.rigid.bodies,
        config.capture_body_trajectory.then_some(&mut body_samples),
    )?;
    let mut timing_samples = Vec::new();
    submit_samples(
        &device,
        &queue,
        &world,
        &view,
        config.viewport,
        camera,
        config.measured_steps,
        config.steps_per_sample,
        &scheduled,
        &mut completed_steps,
        Some(&mut timing_samples),
        &configured.rigid.bodies,
        config.capture_body_trajectory.then_some(&mut body_samples),
    )?;

    let particle_bytes = read_particle_bytes(&device, &queue, &world)?;
    let particle_layout = world
        .particle_validation_layout()
        .ok_or_else(|| "coupled evidence world has no particle validation layout".to_owned())?;
    let particle_diagnostics = world.decode_particle_validation_state(&particle_bytes)?;
    let final_particles = particle_aggregate(particle_layout, &particle_bytes)?;
    let body_bytes = read_body_bytes(&device, &queue, &world)?;
    let bodies = body_observations(
        &configured.rigid.bodies,
        world.body_capacity(),
        &body_bytes,
        particle_layout,
        &particle_bytes,
        configured.materials[0].radius.get(),
    )?;
    let final_body_states = bodies
        .iter()
        .map(|observation| observation.state.clone())
        .collect::<Vec<_>>();
    if body_samples
        .last()
        .is_none_or(|(step, _)| *step != completed_steps)
    {
        body_samples.push((completed_steps, final_body_states.clone()));
    }
    let body_trajectories = body_trajectories(&configured.rigid.bodies, &body_samples)?;
    let exchange = GpuCoupledExchangeV1::new(
        mechanical_state(
            particle_layout,
            &initial_particle_bytes,
            configured.materials[0].particle_mass(),
            &initial_body_states,
        )?,
        mechanical_state(
            particle_layout,
            &particle_bytes,
            configured.materials[0].particle_mass(),
            &final_body_states,
        )?,
    );
    let sleep_cycle = world
        .sleeping_enabled()
        .then(|| read_sleep_cycle(&device, &queue, &world))
        .transpose()?;
    let capacities = world.benchmark_capacities(config.viewport[0], config.viewport[1]);
    for capacity in &capacities {
        capacity.validate().map_err(|error| error.to_string())?;
    }
    let rgba8 = read_texture(
        &device,
        &queue,
        &texture,
        config.viewport[0],
        config.viewport[1],
    )?;
    let exact_state_sha256 = exact_state_hash(
        particle_layout,
        &particle_bytes,
        &configured.rigid.bodies,
        world.body_capacity(),
        &body_bytes,
    )?;
    let particle_centroid_displacement = length(Vec2::new(
        final_particles.centroid.x - initial_particles.centroid.x,
        final_particles.centroid.y - initial_particles.centroid.y,
    ));
    let timing = timing(timing_samples);
    Ok(GpuCoupledEvidenceRun {
        evidence: GpuCoupledEvidenceV2 {
            version: GPU_COUPLED_EVIDENCE_VERSION,
            source_scenario_id: source.id,
            source_scenario_hash,
            configured_projection_hash,
            solver_profile: profile.name(),
            physics: profile.physics(),
            adapter: diagnostics,
            config,
            completed_steps,
            initial_particles,
            final_particles,
            particle_centroid_displacement,
            particle_diagnostics,
            bodies,
            body_trajectories,
            exchange,
            sleep_cycle,
            capacities,
            timing,
            exact_state_sha256,
            action_projection: "stable-body grab commands with product grab tuning",
            trajectory_sampling: if config.capture_body_trajectory {
                "initial and every completed host-wait evidence sample"
            } else {
                "initial and final state only"
            },
            comparator_compiled: cfg!(feature = "box2d-comparator"),
            blocking_readback_for_advancement: false,
        },
        rgba8,
    })
}

#[derive(Clone)]
struct ScheduledCommand {
    step: u32,
    command: BodyCommandV1,
}

fn project_grab_actions(scenario: &CoupledScenarioV1) -> Result<Vec<ScheduledCommand>, String> {
    if !scenario.actions.is_empty() {
        return Err(
            "GPU coupled evidence does not yet project authored particle emit/erase actions"
                .to_owned(),
        );
    }
    let mut active_body = None;
    let mut scheduled = Vec::new();
    for (index, action) in scenario.rigid.actions.iter().enumerate() {
        let sequence = u32::try_from(index + 1)
            .map_err(|_| "coupled action sequence exceeds u32".to_owned())?;
        let command = match action {
            NeutralActionV1::GrabBegin {
                body,
                world_anchor,
                world_target,
                ..
            } => {
                if *world_anchor != *world_target {
                    return Err(
                        "GPU coupled evidence requires a coincident authored grab anchor/target"
                            .to_owned(),
                    );
                }
                active_body = Some(*body);
                BodyCommandV1::GrabBegin {
                    sequence,
                    body: *body,
                    world_target: *world_target,
                }
            }
            NeutralActionV1::GrabTarget { world_target, .. } => {
                let body = active_body
                    .ok_or_else(|| "grab target has no active coupled body".to_owned())?;
                BodyCommandV1::GrabTarget {
                    sequence,
                    body,
                    world_target: *world_target,
                }
            }
            NeutralActionV1::GrabEnd { .. } => {
                let body = active_body
                    .take()
                    .ok_or_else(|| "grab end has no active coupled body".to_owned())?;
                BodyCommandV1::GrabEnd { sequence, body }
            }
            NeutralActionV1::ApplyImpulse { .. } | NeutralActionV1::SetGravity { .. } => {
                return Err(
                    "GPU coupled evidence rejects unsupported impulse/gravity actions".to_owned(),
                );
            }
        };
        scheduled.push(ScheduledCommand {
            step: action.step(),
            command,
        });
    }
    Ok(scheduled)
}

#[allow(clippy::too_many_arguments)]
fn submit_samples(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
    view: &wgpu::TextureView,
    viewport: [u32; 2],
    camera: crate::CameraFrame2d,
    steps: u32,
    steps_per_sample: u32,
    scheduled: &[ScheduledCommand],
    completed_steps: &mut u32,
    mut timing_samples: Option<&mut Vec<TimingSample>>,
    authored_bodies: &[BodyStateV1],
    mut body_samples: Option<&mut Vec<(u32, Vec<BodyStateV1>)>>,
) -> Result<(), String> {
    let mut remaining = steps;
    while remaining != 0 {
        let sample_steps = remaining.min(steps_per_sample);
        let (batch, ranges) = command_plan(*completed_steps, sample_steps, scheduled)?;
        let grab_batch = empty_grab_intent_batch();
        let grab_ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; sample_steps as usize];
        let construction_batch = empty_construction_intent_batch();
        let construction_ranges =
            vec![BodyCommandStepRange { start: 0, count: 0 }; sample_steps as usize];
        let particle_ranges =
            vec![BodyCommandStepRange { start: 0, count: 0 }; sample_steps as usize];
        let started = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_coupled_evidence_sample"),
        });
        world.encode_steps_with_grab_intents(
            queue,
            &mut encoder,
            sample_steps,
            BodyStepPlan {
                batch: &batch,
                ranges: &ranges,
                grab_batch: &grab_batch,
                grab_ranges: &grab_ranges,
                construction_batch: &construction_batch,
                construction_ranges: &construction_ranges,
                particle_commands: &[],
                particle_ranges: &particle_ranges,
                frame_timestamps: None,
            },
        )?;
        let next_completed = completed_steps
            .checked_add(sample_steps)
            .ok_or_else(|| "coupled evidence completed-step count overflowed".to_owned())?;
        world.encode_render_with_timestamp(
            queue,
            &mut encoder,
            RenderFrameTarget {
                color_view: view,
                size: viewport,
            },
            BodyRenderPlan {
                frame_index: u64::from(next_completed),
                camera,
                grab_pointer_active: false,
                surface_preview: None,
                sleep_visuals_enabled: false,
                current_outer_step: next_completed,
                quiet_outer_steps: 30,
                wake_flash_outer_steps: 10,
                ending_timestamp: None,
                frame_timestamps: None,
            },
        );
        queue.submit([encoder.finish()]);
        let cpu_encode_submit_seconds = started.elapsed().as_secs_f64();
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("wait for coupled evidence sample: {error}"))?;
        let host_completion_seconds = started.elapsed().as_secs_f64();
        if let Some(samples) = &mut timing_samples {
            samples.push(TimingSample {
                outer_steps: sample_steps,
                cpu_encode_submit_seconds,
                host_completion_seconds,
            });
        }
        *completed_steps = next_completed;
        if let Some(samples) = &mut body_samples {
            let bytes = read_body_bytes(device, queue, world)?;
            samples.push((
                next_completed,
                decode_body_states(authored_bodies, world.body_capacity(), &bytes)?,
            ));
        }
        remaining -= sample_steps;
    }
    Ok(())
}

fn evidence_camera(
    scenario: &CoupledScenarioV1,
    viewport: [u32; 2],
) -> Result<crate::CameraFrame2d, String> {
    let lower_x = scenario
        .regions
        .iter()
        .map(|region| region.lower.x)
        .fold(f32::INFINITY, f32::min);
    let lower_y = scenario
        .regions
        .iter()
        .map(|region| region.lower.y)
        .fold(f32::INFINITY, f32::min);
    let upper_x = scenario
        .regions
        .iter()
        .map(|region| region.upper.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let upper_y = scenario
        .regions
        .iter()
        .map(|region| region.upper.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let aspect = viewport[0] as f32 / viewport[1] as f32;
    let half_height =
        (0.5 * (upper_y - lower_y) + 1.0).max(0.5 * (upper_x - lower_x) / aspect + 1.0);
    Camera2d::try_new_at_home(
        [0.5 * (lower_x + upper_x), 0.5 * (lower_y + upper_y)],
        half_height,
    )
    .map_err(|error| error.to_string())?
    .resolve(viewport)
    .map_err(|error| error.to_string())
}

fn command_plan(
    completed_steps: u32,
    steps: u32,
    scheduled: &[ScheduledCommand],
) -> Result<(BodyCommandBatchV1, Vec<BodyCommandStepRange>), String> {
    let mut commands = Vec::new();
    let mut ranges = Vec::with_capacity(steps as usize);
    for offset in 0..steps {
        let absolute = completed_steps
            .checked_add(offset)
            .ok_or_else(|| "coupled command step overflowed".to_owned())?;
        let start = commands.len() as u32;
        commands.extend(
            scheduled
                .iter()
                .filter(|entry| entry.step == absolute)
                .map(|entry| entry.command.clone()),
        );
        ranges.push(BodyCommandStepRange {
            start,
            count: commands.len() as u32 - start,
        });
    }
    if commands.len() > BODY_COMMAND_CAPACITY as usize {
        return Err("coupled evidence body-command capacity overflowed".to_owned());
    }
    let batch = BodyCommandBatchV1 {
        schema_version: SCHEMA_VERSION,
        capacity_commands: BODY_COMMAND_CAPACITY,
        attempted_commands: commands.len() as u64,
        overflowed: false,
        commands,
    };
    batch.validate().map_err(|error| error.to_string())?;
    Ok((batch, ranges))
}

fn read_particle_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
) -> Result<Vec<u8>, String> {
    let layout = world
        .particle_validation_layout()
        .ok_or_else(|| "coupled evidence world has no particles".to_owned())?;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_coupled_particle_readback"),
        size: layout.total_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_coupled_particle_copy"),
    });
    world.copy_particle_validation_state(&mut encoder, &buffer)?;
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let bytes = mapped.to_vec();
    drop(mapped);
    buffer.unmap();
    Ok(bytes)
}

fn read_body_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
) -> Result<Vec<u8>, String> {
    let size = u64::from(world.body_capacity()) * size_of::<[u32; 4]>() as u64 * 3;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_coupled_body_readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_coupled_body_copy"),
    });
    world.copy_full_validation_state(&mut encoder, &buffer);
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let bytes = mapped.to_vec();
    drop(mapped);
    buffer.unmap();
    Ok(bytes)
}

fn read_sleep_cycle(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
) -> Result<SleepCycleMetricsV1, String> {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_coupled_sleep_readback"),
        size: u64::from(SLEEP_CYCLE_DIAGNOSTIC_BYTES),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_coupled_sleep_copy"),
    });
    world.copy_sleep_cycle_metrics(&mut encoder, &buffer)?;
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let result = crate::sleep_diagnostics::decode_sleep_cycle_metrics(&mapped)?;
    drop(mapped);
    buffer.unmap();
    Ok(result)
}

fn particle_aggregate(
    layout: ParticleValidationLayout,
    bytes: &[u8],
) -> Result<GpuParticleAggregateV1, String> {
    let lane_bytes = layout.capacity as usize * size_of::<[f32; 4]>();
    let positions = bytemuck::try_cast_slice::<u8, [f32; 4]>(
        &bytes[layout.positions_offset as usize..layout.positions_offset as usize + lane_bytes],
    )
    .map_err(|error| format!("decode coupled particle positions: {error}"))?;
    let velocities = bytemuck::try_cast_slice::<u8, [f32; 4]>(
        &bytes[layout.velocities_offset as usize..layout.velocities_offset as usize + lane_bytes],
    )
    .map_err(|error| format!("decode coupled particle velocities: {error}"))?;
    let flag_bytes = layout.capacity as usize * size_of::<u32>();
    let flags = bytemuck::try_cast_slice::<u8, u32>(
        &bytes
            [layout.active_flags_offset as usize..layout.active_flags_offset as usize + flag_bytes],
    )
    .map_err(|error| format!("decode coupled particle flags: {error}"))?;
    let mut count = 0u32;
    let mut sum = [0.0f64; 4];
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for index in 0..layout.capacity as usize {
        if flags[index] == 0 {
            continue;
        }
        let position = positions[index];
        let velocity = velocities[index];
        if !position.into_iter().all(f32::is_finite) || !velocity.into_iter().all(f32::is_finite) {
            return Err("coupled particle aggregate found non-finite state".to_owned());
        }
        count += 1;
        sum[0] += f64::from(position[0]);
        sum[1] += f64::from(position[1]);
        sum[2] += f64::from(velocity[0]);
        sum[3] += f64::from(velocity[1]);
        bounds[0] = bounds[0].min(position[0]);
        bounds[1] = bounds[1].min(position[1]);
        bounds[2] = bounds[2].max(position[0]);
        bounds[3] = bounds[3].max(position[1]);
    }
    if count == 0 {
        return Err("coupled particle aggregate has no active particles".to_owned());
    }
    let divisor = f64::from(count);
    Ok(GpuParticleAggregateV1 {
        active_particles: count,
        bounds,
        centroid: Vec2::new((sum[0] / divisor) as f32, (sum[1] / divisor) as f32),
        mean_velocity: Vec2::new((sum[2] / divisor) as f32, (sum[3] / divisor) as f32),
    })
}

fn body_observations(
    authored: &[BodyStateV1],
    capacity: u32,
    body_bytes: &[u8],
    particle_layout: ParticleValidationLayout,
    particle_bytes: &[u8],
    particle_radius: f32,
) -> Result<Vec<GpuCoupledBodyObservationV1>, String> {
    let states = decode_body_states(authored, capacity, body_bytes)?;
    let particle_lane = particle_layout.capacity as usize * size_of::<[f32; 4]>();
    let positions = bytemuck::try_cast_slice::<u8, [f32; 4]>(
        &particle_bytes[particle_layout.positions_offset as usize
            ..particle_layout.positions_offset as usize + particle_lane],
    )
    .map_err(|error| format!("decode body-overlap particle positions: {error}"))?;
    let flag_bytes = particle_layout.capacity as usize * size_of::<u32>();
    let flags = bytemuck::try_cast_slice::<u8, u32>(
        &particle_bytes[particle_layout.active_flags_offset as usize
            ..particle_layout.active_flags_offset as usize + flag_bytes],
    )
    .map_err(|error| format!("decode body-overlap particle flags: {error}"))?;

    states
        .into_iter()
        .map(|state| {
            let nearby_particle_count = positions
                .iter()
                .zip(flags)
                .filter(|(_, flag)| **flag != 0)
                .filter(|(position, _)| {
                    particle_near_body(
                        [position[0], position[1]],
                        state.position,
                        state.angle.get(),
                        state.shape,
                        particle_radius,
                    )
                })
                .count() as u32;
            Ok(GpuCoupledBodyObservationV1 {
                state,
                nearby_particle_count,
            })
        })
        .collect()
}

fn decode_body_states(
    authored: &[BodyStateV1],
    capacity: u32,
    bytes: &[u8],
) -> Result<Vec<BodyStateV1>, String> {
    let lane_bytes = capacity as usize * size_of::<[u32; 4]>();
    if bytes.len() != lane_bytes * 3 {
        return Err("coupled body readback has the wrong byte count".to_owned());
    }
    let poses = bytemuck::try_cast_slice::<u8, [f32; 4]>(&bytes[..lane_bytes])
        .map_err(|error| format!("decode coupled body poses: {error}"))?;
    let motions = bytemuck::try_cast_slice::<u8, [f32; 4]>(&bytes[lane_bytes..lane_bytes * 2])
        .map_err(|error| format!("decode coupled body motions: {error}"))?;
    let metadata = bytemuck::try_cast_slice::<u8, [u32; 4]>(&bytes[lane_bytes * 2..lane_bytes * 3])
        .map_err(|error| format!("decode coupled body metadata: {error}"))?;
    authored
        .iter()
        .map(|body| {
            let index = usize::try_from(body.id.slot)
                .map_err(|_| "body slot does not fit usize".to_owned())?;
            let pose = *poses
                .get(index)
                .ok_or_else(|| "authored body exceeds GPU capacity".to_owned())?;
            let motion = motions[index];
            let meta = metadata[index];
            if meta[0] != body.id.generation || meta[1] & 1 == 0 {
                return Err(format!(
                    "authored body {}:{} is not live in sampled GPU state",
                    body.id.slot, body.id.generation
                ));
            }
            if !pose.into_iter().all(f32::is_finite) || !motion.into_iter().all(f32::is_finite) {
                return Err("coupled body sample found non-finite state".to_owned());
            }
            let mut state = body.clone();
            state.id = BodyId {
                slot: body.id.slot,
                generation: meta[0],
            };
            state.position = Vec2::new(pose[0], pose[1]);
            state.angle =
                physbox_contracts::FiniteF32::new(pose[2]).map_err(|error| error.to_string())?;
            state.linear_velocity = Vec2::new(motion[0], motion[1]);
            state.angular_velocity =
                physbox_contracts::FiniteF32::new(motion[2]).map_err(|error| error.to_string())?;
            state.sleeping = meta[1] & (1 << 1) != 0;
            state.validate().map_err(|error| error.to_string())?;
            Ok(state)
        })
        .collect()
}

fn body_trajectories(
    authored: &[BodyStateV1],
    samples: &[(u32, Vec<BodyStateV1>)],
) -> Result<Vec<GpuCoupledBodyTrajectoryV1>, String> {
    if samples.is_empty() {
        return Err("coupled body trajectory has no samples".to_owned());
    }
    authored
        .iter()
        .enumerate()
        .filter(|(_, body)| body.inverse_mass.get() > 0.0)
        .map(|(index, initial)| {
            let mut result = GpuCoupledBodyTrajectoryV1 {
                body: initial.id,
                sample_count: 0,
                max_displacement_from_initial: 0.0,
                max_abs_angle_delta: 0.0,
                max_linear_speed: 0.0,
                max_abs_angular_speed: 0.0,
                first_wake_observed_step: None,
            };
            for (step, states) in samples {
                let state = states
                    .get(index)
                    .ok_or_else(|| "coupled body sample is incomplete".to_owned())?;
                if state.id != initial.id {
                    return Err("coupled body sample identity changed".to_owned());
                }
                result.sample_count += 1;
                result.max_displacement_from_initial = result
                    .max_displacement_from_initial
                    .max(distance(state.position, initial.position));
                result.max_abs_angle_delta = result
                    .max_abs_angle_delta
                    .max((state.angle.get() - initial.angle.get()).abs());
                result.max_linear_speed =
                    result.max_linear_speed.max(length(state.linear_velocity));
                result.max_abs_angular_speed = result
                    .max_abs_angular_speed
                    .max(state.angular_velocity.get().abs());
                if initial.sleeping && !state.sleeping && result.first_wake_observed_step.is_none()
                {
                    result.first_wake_observed_step = Some(*step);
                }
            }
            Ok(result)
        })
        .collect()
}

fn mechanical_state(
    layout: ParticleValidationLayout,
    particle_bytes: &[u8],
    particle_mass: f32,
    bodies: &[BodyStateV1],
) -> Result<GpuCoupledMechanicalStateV1, String> {
    let lane_bytes = layout.capacity as usize * size_of::<[f32; 4]>();
    let velocities = bytemuck::try_cast_slice::<u8, [f32; 4]>(
        &particle_bytes
            [layout.velocities_offset as usize..layout.velocities_offset as usize + lane_bytes],
    )
    .map_err(|error| format!("decode exchange particle velocities: {error}"))?;
    let flag_bytes = layout.capacity as usize * size_of::<u32>();
    let flags = bytemuck::try_cast_slice::<u8, u32>(
        &particle_bytes
            [layout.active_flags_offset as usize..layout.active_flags_offset as usize + flag_bytes],
    )
    .map_err(|error| format!("decode exchange particle flags: {error}"))?;
    let mut particle_linear_momentum = Vec2::ZERO;
    let mut particle_kinetic_energy = 0.0f32;
    for (velocity, flag) in velocities.iter().zip(flags) {
        if *flag == 0 {
            continue;
        }
        let value = Vec2::new(velocity[0], velocity[1]);
        value.validate().map_err(|error| error.to_string())?;
        particle_linear_momentum.x += particle_mass * value.x;
        particle_linear_momentum.y += particle_mass * value.y;
        particle_kinetic_energy += 0.5 * particle_mass * dot(value, value);
    }
    let mut dynamic_body_linear_momentum = Vec2::ZERO;
    let mut dynamic_body_kinetic_energy = 0.0f32;
    for body in bodies {
        if body.inverse_mass.get() <= 0.0 {
            continue;
        }
        let mass = body.inverse_mass.get().recip();
        dynamic_body_linear_momentum.x += mass * body.linear_velocity.x;
        dynamic_body_linear_momentum.y += mass * body.linear_velocity.y;
        dynamic_body_kinetic_energy += 0.5 * mass * dot(body.linear_velocity, body.linear_velocity);
        if body.inverse_inertia.get() > 0.0 {
            let inertia = body.inverse_inertia.get().recip();
            dynamic_body_kinetic_energy +=
                0.5 * inertia * body.angular_velocity.get() * body.angular_velocity.get();
        }
    }
    let state = GpuCoupledMechanicalStateV1 {
        particle_linear_momentum,
        dynamic_body_linear_momentum,
        particle_kinetic_energy,
        dynamic_body_kinetic_energy,
    };
    if ![
        state.particle_linear_momentum.x,
        state.particle_linear_momentum.y,
        state.dynamic_body_linear_momentum.x,
        state.dynamic_body_linear_momentum.y,
        state.particle_kinetic_energy,
        state.dynamic_body_kinetic_energy,
    ]
    .into_iter()
    .all(f32::is_finite)
    {
        return Err("coupled mechanical state is non-finite".to_owned());
    }
    Ok(state)
}

fn particle_near_body(
    particle: [f32; 2],
    body: Vec2,
    angle: f32,
    shape: ShapeV1,
    radius: f32,
) -> bool {
    let sin = angle.sin();
    let cos = angle.cos();
    let delta = [particle[0] - body.x, particle[1] - body.y];
    let local = [
        cos * delta[0] + sin * delta[1],
        -sin * delta[0] + cos * delta[1],
    ];
    match shape {
        ShapeV1::Circle {
            radius: body_radius,
        } => local[0] * local[0] + local[1] * local[1] <= (body_radius.get() + radius).powi(2),
        ShapeV1::Rectangle { half_extents } => {
            local[0].abs() <= half_extents.x + radius && local[1].abs() <= half_extents.y + radius
        }
    }
}

#[derive(Clone, Copy)]
struct TimingSample {
    outer_steps: u32,
    cpu_encode_submit_seconds: f64,
    host_completion_seconds: f64,
}

fn timing(samples: Vec<TimingSample>) -> GpuCoupledTimingV1 {
    let sample_outer_steps = samples.iter().map(|sample| sample.outer_steps).collect();
    let cpu = samples
        .iter()
        .map(|sample| sample.cpu_encode_submit_seconds / f64::from(sample.outer_steps))
        .collect::<Vec<_>>();
    let host = samples
        .iter()
        .map(|sample| sample.host_completion_seconds / f64::from(sample.outer_steps))
        .collect::<Vec<_>>();
    GpuCoupledTimingV1 {
        method: "host-wait-per-sample-includes-physics-and-direct-render",
        sample_outer_steps,
        cpu_encode_submit_seconds_per_outer_step: cpu.clone(),
        host_completion_seconds_per_outer_step: host.clone(),
        cpu_encode_submit: timing_summary(&cpu),
        host_completion: timing_summary(&host),
    }
}

fn timing_summary(values: &[f64]) -> GpuCoupledTimingSummaryV1 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let p95_index = sorted
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    GpuCoupledTimingSummaryV1 {
        samples: values.len() as u32,
        mean_seconds_per_outer_step: values.iter().sum::<f64>() / values.len().max(1) as f64,
        p95_seconds_per_outer_step: sorted.get(p95_index).copied().unwrap_or(0.0),
        max_seconds_per_outer_step: sorted.last().copied().unwrap_or(0.0),
    }
}

fn exact_state_hash(
    particle_layout: ParticleValidationLayout,
    particles: &[u8],
    authored_bodies: &[BodyStateV1],
    body_capacity: u32,
    bodies: &[u8],
) -> Result<String, String> {
    let particle_lane_bytes = particle_layout.capacity as usize * size_of::<[u32; 4]>();
    let positions = bytemuck::try_cast_slice::<u8, [u32; 4]>(
        &particles[particle_layout.positions_offset as usize
            ..particle_layout.positions_offset as usize + particle_lane_bytes],
    )
    .map_err(|error| format!("hash particle positions: {error}"))?;
    let velocities = bytemuck::try_cast_slice::<u8, [u32; 4]>(
        &particles[particle_layout.velocities_offset as usize
            ..particle_layout.velocities_offset as usize + particle_lane_bytes],
    )
    .map_err(|error| format!("hash particle velocities: {error}"))?;
    let metadata = bytemuck::try_cast_slice::<u8, [u32; 4]>(
        &particles[particle_layout.metadata_offset as usize
            ..particle_layout.metadata_offset as usize + particle_lane_bytes],
    )
    .map_err(|error| format!("hash particle metadata: {error}"))?;
    let densities = bytemuck::try_cast_slice::<u8, [u32; 4]>(
        &particles[particle_layout.density_offset as usize
            ..particle_layout.density_offset as usize + particle_lane_bytes],
    )
    .map_err(|error| format!("hash particle density: {error}"))?;
    let flag_bytes = particle_layout.capacity as usize * size_of::<u32>();
    let flags = bytemuck::try_cast_slice::<u8, u32>(
        &particles[particle_layout.active_flags_offset as usize
            ..particle_layout.active_flags_offset as usize + flag_bytes],
    )
    .map_err(|error| format!("hash particle flags: {error}"))?;

    let body_lane_bytes = body_capacity as usize * size_of::<[u32; 4]>();
    if bodies.len() != body_lane_bytes * 3 {
        return Err("hash body state has the wrong byte count".to_owned());
    }
    let body_poses = bytemuck::try_cast_slice::<u8, [u32; 4]>(&bodies[..body_lane_bytes])
        .map_err(|error| format!("hash body poses: {error}"))?;
    let body_motions =
        bytemuck::try_cast_slice::<u8, [u32; 4]>(&bodies[body_lane_bytes..body_lane_bytes * 2])
            .map_err(|error| format!("hash body motions: {error}"))?;
    let body_metadata =
        bytemuck::try_cast_slice::<u8, [u32; 4]>(&bodies[body_lane_bytes * 2..body_lane_bytes * 3])
            .map_err(|error| format!("hash body metadata: {error}"))?;

    let mut hasher = Sha256::new();
    for index in 0..particle_layout.capacity as usize {
        if flags[index] == 0 {
            continue;
        }
        hasher.update((index as u32).to_le_bytes());
        hasher.update(bytemuck::bytes_of(&positions[index]));
        hasher.update(bytemuck::bytes_of(&velocities[index]));
        hasher.update(bytemuck::bytes_of(&metadata[index]));
        hasher.update(bytemuck::bytes_of(&densities[index]));
    }
    for body in authored_bodies {
        let index = body.id.slot as usize;
        hasher.update(body.id.slot.to_le_bytes());
        hasher.update(bytemuck::bytes_of(&body_poses[index]));
        hasher.update(bytemuck::bytes_of(&body_motions[index]));
        hasher.update(bytemuck::bytes_of(&body_metadata[index]));
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn length(value: Vec2) -> f32 {
    (value.x * value.x + value.y * value.y).sqrt()
}

fn distance(left: Vec2, right: Vec2) -> f32 {
    length(subtract(left, right))
}

fn subtract(left: Vec2, right: Vec2) -> Vec2 {
    Vec2::new(left.x - right.x, left.y - right.y)
}

fn dot(left: Vec2, right: Vec2) -> f32 {
    left.x * right.x + left.y * right.y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_grab_projection_is_bounded_and_generation_targeted() {
        let scenario =
            crate::canonical_coupled_scenario(crate::CoupledScenarioKind::GrabbedBoundary).unwrap();
        let scheduled = project_grab_actions(&scenario).unwrap();
        assert_eq!(scheduled.len(), 4);
        assert_eq!(scheduled[0].step, 30);
        assert!(matches!(
            scheduled[0].command,
            BodyCommandV1::GrabBegin {
                body: BodyId {
                    slot: 3,
                    generation: 1
                },
                ..
            }
        ));
        let (batch, ranges) = command_plan(0, 64, &scheduled).unwrap();
        assert_eq!(batch.commands.len(), 1);
        assert_eq!(ranges[30].count, 1);
    }

    #[test]
    fn evidence_configuration_rejects_unbounded_sampling() {
        let config = GpuCoupledEvidenceConfigV1 {
            viewport: [960, 600],
            warmup_steps: 0,
            measured_steps: 1,
            steps_per_sample: 65,
            capture_body_trajectory: false,
        };
        assert!(config.validate().is_err());
    }
}
