use std::collections::BTreeMap;
use std::mem::size_of;
use std::time::Instant;

use physbox_contracts::{
    BodyCommandBatchV1, BodyStateV1, COLLISION_CAPACITY_COUNTER_BYTES, ConstructionIntentBatchV1,
    ProvenanceV1, SCHEMA_VERSION, Validate, canonical_hash,
};
use physbox_launch::{FluidSolidCouplingQuality, WorldCapacityPreset};
use physbox_sim::__private::gpu_timing::{GpuFrameTimestampPlan, GpuParticleTimingStage};
use sha2::{Digest, Sha256};

use crate::body_state::BODY_COMMAND_CAPACITY;
use crate::body_world::{BodyCommandStepRange, BodyStepPlan, GpuBodyWorld};
use crate::construction::empty_construction_intent_batch;
use crate::generated_contacts::decode_construction_graph_evidence;
use crate::gpu_context::{
    AdapterDiagnostics, create_instance, request_adapter, request_device_with_features,
};
use crate::grab::empty_grab_intent_batch;
use crate::particle_world::ParticleValidationLayout;
use crate::renderer::map_buffer;
use crate::{
    LIQUID_SCALE_ARTIFACT_VERSION, LIQUID_SCALE_PERCENTILE_METHOD, LiquidScaleBenchmarkArtifactV1,
    LiquidScaleBenchmarkConfigV1, LiquidScaleCorrectnessV1, LiquidScaleGpuStageTimingV1,
    LiquidScaleGpuStageV1, LiquidScaleGpuTimestampStatusV1, LiquidScaleHostKindV1,
    LiquidScaleTimingModeV1, LiquidScaleTimingSeriesV1, LiquidScaleTimingsV1,
    LiquidScaleWorkVectorV1, SolverProfileConfig, liquid_scale_benchmark_source,
};

const FRAME_QUERY_COUNT: u32 = 3;

pub struct LiquidScalePhysicsBenchmarkRun {
    pub artifact: LiquidScaleBenchmarkArtifactV1,
    pub adapter: AdapterDiagnostics,
}

pub async fn run_liquid_scale_physics_benchmark(
    config: LiquidScaleBenchmarkConfigV1,
    host_kind: LiquidScaleHostKindV1,
    mut provenance: ProvenanceV1,
) -> Result<LiquidScalePhysicsBenchmarkRun, String> {
    config.validate().map_err(|error| error.to_string())?;
    if config.timing_mode != LiquidScaleTimingModeV1::PhysicsOnlyExplicitSteps {
        return Err("native liquid physics benchmark requires explicit-step timing".to_owned());
    }
    if cfg!(feature = "box2d-comparator") {
        return Err("liquid benchmark rejects a comparator-enabled build".to_owned());
    }
    if cfg!(debug_assertions) {
        return Err("liquid benchmark requires a release build".to_owned());
    }
    let expected_host = if cfg!(target_os = "android") {
        LiquidScaleHostKindV1::AndroidNative
    } else {
        LiquidScaleHostKindV1::NativeDesktop
    };
    if host_kind != expected_host {
        return Err(format!(
            "liquid benchmark host kind {host_kind:?} does not match {expected_host:?}"
        ));
    }

    let source = liquid_scale_benchmark_source(config.solid_tier, config.particle_count)
        .map_err(|error| error.to_string())?;
    let total_steps = config
        .warmup_steps
        .checked_add(config.retained_samples)
        .ok_or_else(|| "liquid benchmark step count overflowed".to_owned())?;
    if total_steps > source.scenario.requested_steps {
        return Err(format!(
            "liquid benchmark requests {total_steps} steps from source window {}",
            source.scenario.requested_steps
        ));
    }
    let mut scenario = source.scenario.clone();
    let profile = SolverProfileConfig::default();
    profile.apply_to_scenario(&mut scenario.rigid)?;
    scenario.validate().map_err(|error| error.to_string())?;
    let scenario_hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;

    let instance = create_instance();
    let adapter = request_adapter(&instance, None).await?;
    let adapter_diagnostics = AdapterDiagnostics::from_adapter(&adapter);
    let timestamps_available = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let required_features = if timestamps_available {
        wgpu::Features::TIMESTAMP_QUERY
    } else {
        wgpu::Features::empty()
    };
    let (device, queue) = request_device_with_features(&adapter, required_features).await?;
    let world = GpuBodyWorld::new_coupled_with_configuration(
        &device,
        &scenario,
        WorldCapacityPreset::Slots64,
        profile.physics(),
        FluidSolidCouplingQuality::Standard,
    )?;
    if world.body_capacity() != source.expected_body_capacity {
        return Err(format!(
            "liquid benchmark expected body capacity {}, created {}",
            source.expected_body_capacity,
            world.body_capacity()
        ));
    }
    if world.initial_particle_count() != config.particle_count
        || world.particle_capacity() != config.particle_count
    {
        return Err(
            "liquid benchmark world did not create the exact particle population".to_owned(),
        );
    }

    let empty_batch = BodyCommandBatchV1 {
        schema_version: SCHEMA_VERSION,
        capacity_commands: BODY_COMMAND_CAPACITY,
        attempted_commands: 0,
        overflowed: false,
        commands: Vec::new(),
    };
    let grab_batch = empty_grab_intent_batch();
    let construction_batch = empty_construction_intent_batch();

    encode_untimed_steps(
        &device,
        &queue,
        &world,
        config.warmup_steps,
        &empty_batch,
        &grab_batch,
        &construction_batch,
    )?;

    let particle_stages = world.particle_timestamp_stages_per_step();
    if particle_stages.len() as u32 != world.particle_dispatches_per_step() {
        return Err("liquid particle timestamp layout does not match dispatch count".to_owned());
    }
    let query_count = FRAME_QUERY_COUNT
        .checked_add(particle_stages.len() as u32)
        .ok_or_else(|| "liquid timestamp query count overflowed".to_owned())?;
    let timestamps = timestamps_available
        .then(|| TimestampResources::new(&device, query_count, config.retained_samples));
    let mut cpu_seconds = Vec::with_capacity(config.retained_samples as usize);
    let mut host_seconds = Vec::with_capacity(config.retained_samples as usize);
    for sample in 0..config.retained_samples {
        let started = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_liquid_scale_sample"),
        });
        let ranges = [BodyCommandStepRange { start: 0, count: 0 }];
        world.encode_steps_with_grab_intents(
            &queue,
            &mut encoder,
            1,
            BodyStepPlan {
                batch: &empty_batch,
                ranges: &ranges,
                grab_batch: &grab_batch,
                grab_ranges: &ranges,
                construction_batch: &construction_batch,
                construction_ranges: &ranges,
                particle_commands: &[],
                particle_ranges: &ranges,
                frame_timestamps: timestamps.as_ref().map(|timestamps| GpuFrameTimestampPlan {
                    query_set: &timestamps.query_set,
                    query_base: 0,
                    particle_query_base: Some(FRAME_QUERY_COUNT),
                }),
            },
        )?;
        if let Some(timestamps) = &timestamps {
            encoder.resolve_query_set(
                &timestamps.query_set,
                0..timestamps.query_count,
                &timestamps.resolve,
                u64::from(sample) * timestamps.sample_stride,
            );
        }
        queue.submit([encoder.finish()]);
        cpu_seconds.push(started.elapsed().as_secs_f64());
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("wait for liquid benchmark sample {sample}: {error}"))?;
        host_seconds.push(started.elapsed().as_secs_f64());
    }

    let particle_layout = world
        .particle_validation_layout()
        .ok_or_else(|| "liquid benchmark world has no particle validation layout".to_owned())?;
    let particle_readback = readback_buffer(
        &device,
        "physbox5_liquid_scale_particle_readback",
        particle_layout.total_bytes,
    );
    let body_bytes = u64::from(world.body_capacity()) * size_of::<[u32; 4]>() as u64 * 3;
    let body_readback = readback_buffer(&device, "physbox5_liquid_scale_body_readback", body_bytes);
    let graph_bytes = u64::from(COLLISION_CAPACITY_COUNTER_BYTES) * 4;
    let graph_readback =
        readback_buffer(&device, "physbox5_liquid_scale_graph_readback", graph_bytes);
    let mut final_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_liquid_scale_final_readback"),
    });
    world.copy_particle_validation_state(&mut final_encoder, &particle_readback)?;
    world.copy_full_validation_state(&mut final_encoder, &body_readback);
    world.copy_construction_graph_evidence(&mut final_encoder, &graph_readback)?;
    if let Some(timestamps) = &timestamps {
        final_encoder.copy_buffer_to_buffer(
            &timestamps.resolve,
            0,
            &timestamps.readback,
            0,
            timestamps.total_bytes,
        );
    }
    queue.submit([final_encoder.finish()]);
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("wait for liquid benchmark final readback: {error}"))?;

    let particle_bytes = mapped_bytes(&device, &particle_readback)?;
    let body_state_bytes = mapped_bytes(&device, &body_readback)?;
    let graph_state_bytes = mapped_bytes(&device, &graph_readback)?;
    let timestamp_bytes = timestamps
        .as_ref()
        .map(|timestamps| mapped_bytes(&device, &timestamps.readback))
        .transpose()?;
    let particle_diagnostics = world.decode_particle_validation_state(&particle_bytes)?;
    let (awake_bodies, bodies_finite, bodies_valid) = validate_body_state(
        &scenario.rigid.bodies,
        world.body_capacity(),
        &body_state_bytes,
    )?;
    let graph = decode_construction_graph_evidence(&graph_state_bytes)?;
    if graph.overflowed {
        return Err("liquid benchmark final rigid graph overflowed".to_owned());
    }
    let state_hash = exact_state_hash(
        particle_layout,
        &particle_bytes,
        &scenario.rigid.bodies,
        world.body_capacity(),
        &body_state_bytes,
    )?;
    let timings = benchmark_timings(
        timestamps.as_ref(),
        timestamp_bytes.as_deref(),
        &particle_stages,
        cpu_seconds,
        host_seconds,
        queue.get_timestamp_period(),
    )?;
    let mut capacities = world.benchmark_capacities(0, 0);
    capacities.sort_by(|left, right| left.class.cmp(&right.class));
    for capacity in &capacities {
        capacity.validate().map_err(|error| error.to_string())?;
    }

    provenance.adapter = adapter_diagnostics.name.clone();
    provenance.backend = adapter_diagnostics.backend.clone();
    let reported_driver = format!(
        "{} {}",
        adapter_diagnostics.driver, adapter_diagnostics.driver_info
    )
    .trim()
    .to_owned();
    provenance.driver = if reported_driver.is_empty() {
        "not-reported".to_owned()
    } else {
        reported_driver
    };
    let dispatches_per_outer_step = 1u32
        .checked_add(world.solver_dispatches_per_step())
        .and_then(|value| value.checked_add(world.particle_dispatches_per_step()))
        .and_then(|value| value.checked_add(world.body_lifecycle_dispatches_per_step()))
        .and_then(|value| value.checked_add(world.post_fluid_stabilization_dispatches_per_step()))
        .ok_or_else(|| "liquid dispatch count overflowed".to_owned())?;
    let live_bodies = source
        .dynamic_solid_count
        .checked_add(source.fixed_solid_count)
        .ok_or_else(|| "liquid live-body count overflowed".to_owned())?;
    let artifact = LiquidScaleBenchmarkArtifactV1 {
        schema_version: SCHEMA_VERSION,
        artifact_version: LIQUID_SCALE_ARTIFACT_VERSION,
        scenario_id: source.id,
        source_definition_hash: source.scenario_hash,
        scenario_hash,
        host_kind,
        browser_version: None,
        build_profile: "release".to_owned(),
        comparator_enabled: cfg!(feature = "box2d-comparator"),
        provenance,
        config: config.clone(),
        work: LiquidScaleWorkVectorV1 {
            version: LIQUID_SCALE_ARTIFACT_VERSION,
            dynamic_bodies: source.dynamic_solid_count,
            fixed_bodies: source.fixed_solid_count,
            live_bodies,
            awake_bodies,
            body_capacity: world.body_capacity(),
            candidate_pairs: u64::from(graph.candidate_pairs),
            manifolds: graph.manifolds,
            contact_points: graph.contact_points,
            particle_capacity: particle_diagnostics.capacity,
            live_particles: particle_diagnostics.active_count,
            grid_insertions: u64::from(particle_diagnostics.grid_insertions),
            occupied_cells: particle_diagnostics.occupied_grid_cells,
            neighbor_checks: u64::from(particle_diagnostics.neighbor_checks),
            accepted_neighbors: u64::from(particle_diagnostics.accepted_neighbors),
            particle_body_contacts: u64::from(particle_diagnostics.body_contacts),
            body_contributions: u64::from(particle_diagnostics.body_contribution_edges),
            fluid_wake_roots: particle_diagnostics.fluid_wake_roots,
            dispatches_per_outer_step,
            workgroups_per_outer_step: None,
        },
        capacities,
        timings,
        correctness: LiquidScaleCorrectnessV1 {
            completed_outer_steps: config.retained_samples,
            expected_final_active_particles: config.particle_count,
            final_active_particles: particle_diagnostics.active_count,
            final_state_finite: particle_diagnostics.state_finite && bodies_finite,
            generations_and_live_flags_valid: bodies_valid
                && particle_diagnostics.active_count == particle_diagnostics.active_flags_count,
            capacity_clean: particle_diagnostics.capacity_clean,
            state_hash,
            effective_simulation_scale: None,
            slipped_wall_seconds: None,
            sustained_60_hz: false,
        },
    };
    artifact.validate().map_err(|error| error.to_string())?;
    Ok(LiquidScalePhysicsBenchmarkRun {
        artifact,
        adapter: adapter_diagnostics,
    })
}

fn encode_untimed_steps(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
    steps: u32,
    batch: &BodyCommandBatchV1,
    grab_batch: &physbox_contracts::GrabIntentBatchV1,
    construction_batch: &ConstructionIntentBatchV1,
) -> Result<(), String> {
    let ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize];
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_liquid_scale_warmup"),
    });
    world.encode_steps_with_grab_intents(
        queue,
        &mut encoder,
        steps,
        BodyStepPlan {
            batch,
            ranges: &ranges,
            grab_batch,
            grab_ranges: &ranges,
            construction_batch,
            construction_ranges: &ranges,
            particle_commands: &[],
            particle_ranges: &ranges,
            frame_timestamps: None,
        },
    )?;
    queue.submit([encoder.finish()]);
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("wait for liquid benchmark warmup: {error}"))?;
    Ok(())
}

struct TimestampResources {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    query_count: u32,
    sample_stride: u64,
    total_bytes: u64,
}

impl TimestampResources {
    fn new(device: &wgpu::Device, query_count: u32, samples: u32) -> Self {
        let query_bytes = u64::from(query_count) * size_of::<u64>() as u64;
        let sample_stride = query_bytes.div_ceil(wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
            * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
        let total_bytes = sample_stride * u64::from(samples);
        Self {
            query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("physbox5_liquid_scale_timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: query_count,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("physbox5_liquid_scale_timestamp_resolve"),
                size: total_bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: readback_buffer(
                device,
                "physbox5_liquid_scale_timestamp_readback",
                total_bytes,
            ),
            query_count,
            sample_stride,
            total_bytes,
        }
    }
}

fn benchmark_timings(
    timestamps: Option<&TimestampResources>,
    timestamp_bytes: Option<&[u8]>,
    particle_stages: &[GpuParticleTimingStage],
    cpu_seconds: Vec<f64>,
    host_seconds: Vec<f64>,
    timestamp_period_nanoseconds: f32,
) -> Result<LiquidScaleTimingsV1, String> {
    let cpu_encode_submit =
        LiquidScaleTimingSeriesV1::host(cpu_seconds).map_err(|error| error.to_string())?;
    let host_completion =
        LiquidScaleTimingSeriesV1::host(host_seconds).map_err(|error| error.to_string())?;
    let Some(timestamps) = timestamps else {
        return Ok(LiquidScaleTimingsV1 {
            percentile_method: LIQUID_SCALE_PERCENTILE_METHOD.to_owned(),
            gpu_timestamp_status: LiquidScaleGpuTimestampStatusV1::Unsupported,
            timestamp_quantization_seconds: None,
            gpu_stages: Vec::new(),
            rejected_gpu_timestamp_raw_seconds: Vec::new(),
            cpu_encode_submit,
            host_completion,
        });
    };
    let bytes = timestamp_bytes
        .ok_or_else(|| "liquid GPU timestamps were available but not mapped".to_owned())?;
    if bytes.len() != timestamps.total_bytes as usize {
        return Err("liquid GPU timestamp readback has the wrong byte count".to_owned());
    }
    let period_seconds = f64::from(timestamp_period_nanoseconds) * 1.0e-9;
    let mut raw = BTreeMap::<LiquidScaleGpuStageV1, Vec<f64>>::new();
    let mut decoded_samples = Vec::with_capacity(host_completion.sample_count as usize);
    for sample in 0..host_completion.sample_count as usize {
        let offset = sample * timestamps.sample_stride as usize;
        let values = bytemuck::try_cast_slice::<u8, u64>(
            &bytes[offset..offset + timestamps.query_count as usize * size_of::<u64>()],
        )
        .map_err(|error| format!("decode liquid GPU timestamp sample: {error}"))?;
        let frame_start = values[0];
        let rigid_end = values[1];
        let liquid_end = values[2];
        if rigid_end < frame_start || liquid_end < rigid_end {
            return Err(format!(
                "liquid GPU timestamp sample {sample} ran backwards"
            ));
        }
        push_timing(
            &mut raw,
            LiquidScaleGpuStageV1::Rigid,
            rigid_end - frame_start,
            period_seconds,
        );
        push_timing(
            &mut raw,
            LiquidScaleGpuStageV1::LiquidTotal,
            liquid_end - rigid_end,
            period_seconds,
        );
        push_timing(
            &mut raw,
            LiquidScaleGpuStageV1::PhysicsTotal,
            liquid_end - frame_start,
            period_seconds,
        );
        decoded_samples.push(values.to_vec());
    }
    let totals = raw
        .get(&LiquidScaleGpuStageV1::PhysicsTotal)
        .cloned()
        .ok_or_else(|| "liquid GPU timing did not produce physics totals".to_owned())?;
    let all_zero = totals.iter().all(|sample| *sample == 0.0);
    if all_zero {
        return Ok(LiquidScaleTimingsV1 {
            percentile_method: LIQUID_SCALE_PERCENTILE_METHOD.to_owned(),
            gpu_timestamp_status: LiquidScaleGpuTimestampStatusV1::AdvertisedUnusableZero,
            timestamp_quantization_seconds: Some(period_seconds),
            gpu_stages: Vec::new(),
            rejected_gpu_timestamp_raw_seconds: totals,
            cpu_encode_submit,
            host_completion,
        });
    }
    let mut particle_stages_usable = true;
    for values in decoded_samples {
        let liquid_end = values[2];
        let mut particle_stage_ticks = BTreeMap::<LiquidScaleGpuStageV1, u64>::new();
        for (index, stage) in particle_stages.iter().copied().enumerate() {
            let beginning = values[FRAME_QUERY_COUNT as usize + index];
            let end = if index + 1 == particle_stages.len() {
                liquid_end
            } else {
                values[FRAME_QUERY_COUNT as usize + index + 1]
            };
            if end < beginning {
                particle_stages_usable = false;
                break;
            }
            let stage = benchmark_stage(stage);
            let total = particle_stage_ticks.entry(stage).or_default();
            *total = total
                .checked_add(end - beginning)
                .ok_or_else(|| "liquid particle timestamp accumulation overflowed".to_owned())?;
        }
        if !particle_stages_usable {
            break;
        }
        for (stage, ticks) in particle_stage_ticks {
            push_timing(&mut raw, stage, ticks, period_seconds);
        }
    }
    if !particle_stages_usable {
        raw.retain(|stage, _| {
            matches!(
                stage,
                LiquidScaleGpuStageV1::Rigid
                    | LiquidScaleGpuStageV1::LiquidTotal
                    | LiquidScaleGpuStageV1::PhysicsTotal
            )
        });
    }
    let has_zero = raw.values().flatten().any(|sample| *sample == 0.0);
    let gpu_timestamp_status = if has_zero {
        LiquidScaleGpuTimestampStatusV1::QuantizedZeros
    } else {
        LiquidScaleGpuTimestampStatusV1::Valid
    };
    let gpu_stages = raw
        .into_iter()
        .map(|(stage, samples)| {
            LiquidScaleTimingSeriesV1::gpu(samples)
                .map(|series| LiquidScaleGpuStageTimingV1 { stage, series })
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LiquidScaleTimingsV1 {
        percentile_method: LIQUID_SCALE_PERCENTILE_METHOD.to_owned(),
        gpu_timestamp_status,
        timestamp_quantization_seconds: Some(period_seconds),
        gpu_stages,
        rejected_gpu_timestamp_raw_seconds: Vec::new(),
        cpu_encode_submit,
        host_completion,
    })
}

fn push_timing(
    raw: &mut BTreeMap<LiquidScaleGpuStageV1, Vec<f64>>,
    stage: LiquidScaleGpuStageV1,
    ticks: u64,
    period_seconds: f64,
) {
    raw.entry(stage)
        .or_default()
        .push(ticks as f64 * period_seconds);
}

fn benchmark_stage(stage: GpuParticleTimingStage) -> LiquidScaleGpuStageV1 {
    match stage {
        GpuParticleTimingStage::IntegrationAndBoundary => {
            LiquidScaleGpuStageV1::ParticleIntegrationAndBoundary
        }
        GpuParticleTimingStage::Grid => LiquidScaleGpuStageV1::ParticleGrid,
        GpuParticleTimingStage::DensityAndCorrection => {
            LiquidScaleGpuStageV1::ParticleDensityAndCorrection
        }
        GpuParticleTimingStage::BodyReaction => LiquidScaleGpuStageV1::ParticleBodyReaction,
        GpuParticleTimingStage::Velocity => LiquidScaleGpuStageV1::ParticleVelocity,
        GpuParticleTimingStage::Compaction => LiquidScaleGpuStageV1::ParticleCompaction,
    }
}

fn validate_body_state(
    authored: &[BodyStateV1],
    capacity: u32,
    bytes: &[u8],
) -> Result<(u32, bool, bool), String> {
    let lane_bytes = capacity as usize * size_of::<[u32; 4]>();
    if bytes.len() != lane_bytes * 3 {
        return Err("liquid body readback has the wrong byte count".to_owned());
    }
    let poses = bytemuck::try_cast_slice::<u8, [f32; 4]>(&bytes[..lane_bytes])
        .map_err(|error| format!("decode liquid body poses: {error}"))?;
    let motions = bytemuck::try_cast_slice::<u8, [f32; 4]>(&bytes[lane_bytes..lane_bytes * 2])
        .map_err(|error| format!("decode liquid body motions: {error}"))?;
    let metadata = bytemuck::try_cast_slice::<u8, [u32; 4]>(&bytes[lane_bytes * 2..])
        .map_err(|error| format!("decode liquid body metadata: {error}"))?;
    let mut awake_bodies = 0;
    let mut finite = true;
    let mut valid = true;
    for body in authored {
        let index = body.id.slot as usize;
        let pose = poses
            .get(index)
            .ok_or_else(|| "liquid authored body exceeds GPU capacity".to_owned())?;
        finite &= pose.iter().copied().all(f32::is_finite)
            && motions[index].into_iter().all(f32::is_finite);
        valid &= metadata[index][0] == body.id.generation && metadata[index][1] & 1 != 0;
        if body.inverse_mass.get() > 0.0 && metadata[index][1] & (1 << 1) == 0 {
            awake_bodies += 1;
        }
    }
    Ok((awake_bodies, finite, valid))
}

fn exact_state_hash(
    particle_layout: ParticleValidationLayout,
    particles: &[u8],
    authored_bodies: &[BodyStateV1],
    body_capacity: u32,
    bodies: &[u8],
) -> Result<String, String> {
    let particle_lane_bytes = particle_layout.capacity as usize * size_of::<[u32; 4]>();
    let positions = particle_lane(
        particles,
        particle_layout.positions_offset,
        particle_lane_bytes,
    )?;
    let velocities = particle_lane(
        particles,
        particle_layout.velocities_offset,
        particle_lane_bytes,
    )?;
    let metadata = particle_lane(
        particles,
        particle_layout.metadata_offset,
        particle_lane_bytes,
    )?;
    let densities = particle_lane(
        particles,
        particle_layout.density_offset,
        particle_lane_bytes,
    )?;
    let flag_bytes = particle_layout.capacity as usize * size_of::<u32>();
    let flags = bytemuck::try_cast_slice::<u8, u32>(
        &particles[particle_layout.active_flags_offset as usize
            ..particle_layout.active_flags_offset as usize + flag_bytes],
    )
    .map_err(|error| format!("hash liquid particle flags: {error}"))?;
    let body_lane_bytes = body_capacity as usize * size_of::<[u32; 4]>();
    if bodies.len() != body_lane_bytes * 3 {
        return Err("hash liquid body state has the wrong byte count".to_owned());
    }
    let body_poses = bytemuck::try_cast_slice::<u8, [u32; 4]>(&bodies[..body_lane_bytes])
        .map_err(|error| format!("hash liquid body poses: {error}"))?;
    let body_motions =
        bytemuck::try_cast_slice::<u8, [u32; 4]>(&bodies[body_lane_bytes..body_lane_bytes * 2])
            .map_err(|error| format!("hash liquid body motions: {error}"))?;
    let body_metadata = bytemuck::try_cast_slice::<u8, [u32; 4]>(&bodies[body_lane_bytes * 2..])
        .map_err(|error| format!("hash liquid body metadata: {error}"))?;
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

fn particle_lane(particles: &[u8], offset: u64, lane_bytes: usize) -> Result<&[[u32; 4]], String> {
    bytemuck::try_cast_slice::<u8, [u32; 4]>(
        &particles[offset as usize..offset as usize + lane_bytes],
    )
    .map_err(|error| format!("hash liquid particle lane: {error}"))
}

fn readback_buffer(device: &wgpu::Device, label: &'static str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

fn mapped_bytes(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Result<Vec<u8>, String> {
    let mapped = map_buffer(device, buffer)?;
    let bytes = mapped.to_vec();
    drop(mapped);
    buffer.unmap();
    Ok(bytes)
}
