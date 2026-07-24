use std::{collections::BTreeSet, mem::size_of};

use physbox_contracts::{
    BodyCommandBatchV1, BodyId, BodyStateV1, COLLISION_CAPACITY_COUNTER_BYTES,
    COLLISION_MANIFOLD_BYTES, CandidatePairV1, CapacityRecordV1, ConstructionIntentBatchV1,
    ConstructionStateV1, SCHEMA_VERSION, ShapeV1, Validate, Vec2, canonical_hash,
};
use physbox_launch::{FluidSolidCouplingQuality, WorldCapacityPreset};
use serde::Serialize;

use crate::body_floor_solver::decode_pass_evidence;
use crate::body_state::BODY_COMMAND_CAPACITY;
use crate::body_world::{BodyCommandStepRange, BodyStepPlan, GpuBodyWorld};
use crate::construction::{CONSTRUCTION_STATE_BYTES, construction_state_from_bytes};
use crate::coupled_scenario::{
    FluidSolidOverlapArrangement, FluidSolidOverlapSourceV1, fluid_solid_overlap_sources,
};
use crate::gpu_context::{request_adapter, request_device};
use crate::grab::empty_grab_intent_batch;
use crate::particle_world::GpuParticleDiagnosticsV4;
use crate::renderer::map_buffer;
use crate::{AdapterDiagnostics, SolverProfileConfig, create_instance};

pub const GPU_FLUID_SOLID_OVERLAP_EVIDENCE_VERSION: u32 = 3;
pub const FLUID_SOLID_SIGNIFICANT_OVERLAP: f32 = 0.005;
pub const FLUID_SOLID_OVERLAP_TRIALS_PER_MEDIUM: u32 = 2;
const FINAL_WINDOW_SAMPLES: usize = 5;
const MAX_ENCODED_STEPS: u32 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FluidSolidOverlapMedium {
    Dry,
    Wet,
}

impl FluidSolidOverlapMedium {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dry => "dry",
            Self::Wet => "wet",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuFluidSolidPairMetricsV1 {
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub authored_separation: f32,
    pub post_admission_separation: f32,
    pub worst_separation: f32,
    pub final_separation: f32,
    pub final_window_min_separation: f32,
    pub final_window_mean_separation: f32,
    pub overlap_samples: u32,
    pub final_window_overlap_samples: u32,
    pub separation_events: u32,
    pub reentry_events: u32,
    pub separated_after_admission: bool,
    pub sustained_overlap: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuFluidSolidMotionMetricsV1 {
    pub sample_count: u32,
    pub max_linear_speed: f32,
    pub max_abs_angular_speed: f32,
    pub initial_kinetic_energy: f32,
    pub maximum_kinetic_energy: f32,
    pub final_kinetic_energy: f32,
    pub maximum_sample_energy_increase: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuRigidContactDiagnosticsV1 {
    pub contact_point_capacity: u32,
    pub active_contact_points: u32,
    pub active_manifolds: u32,
    pub minimum_separation: Option<f32>,
    pub maximum_normal_impulse: f32,
    pub state_finite: bool,
    pub coloring_overflowed: bool,
    pub parallel_uncolored_units: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuPostFluidCandidateAuditSampleV1 {
    pub recovery_step: u32,
    pub retained_contact_pairs: u32,
    pub complete_contact_pairs: u32,
    pub missed_pairs: Vec<CandidatePairV1>,
    pub retained_only_pairs: Vec<CandidatePairV1>,
    pub retained_overflowed: bool,
    pub complete_overflowed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuPostFluidCandidateAuditV1 {
    pub sample_count: u32,
    pub samples_with_misses: u32,
    pub missed_pair_observations: u32,
    pub missed_unique_pairs: Vec<CandidatePairV1>,
    pub retained_only_pair_observations: u32,
    pub retained_only_unique_pairs: Vec<CandidatePairV1>,
    pub capacity_clean: bool,
    pub comparison_consistent: bool,
    pub samples: Vec<GpuPostFluidCandidateAuditSampleV1>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuFluidSolidOverlapRowV1 {
    pub source_id: String,
    pub source_hash: String,
    pub trial_index: u32,
    pub arrangement: FluidSolidOverlapArrangement,
    pub particle_phase_index: u32,
    pub particle_phase_offset: Vec2,
    pub medium: FluidSolidOverlapMedium,
    pub coupling_quality: &'static str,
    pub settle_steps: u32,
    pub recovery_steps: u32,
    pub sample_stride: u32,
    pub solver_dispatches_per_step: u32,
    pub solver_allocated_bytes: u64,
    pub post_fluid_stabilization_dispatches_per_step: u32,
    pub post_fluid_stabilization_allocated_bytes: u64,
    pub particle_dispatches_per_step: u32,
    pub body_lifecycle_dispatches_per_step: u32,
    pub construction: ConstructionStateV1,
    pub rigid_contacts: GpuRigidContactDiagnosticsV1,
    pub post_fluid_candidate_audit: Option<GpuPostFluidCandidateAuditV1>,
    pub pairs: Vec<GpuFluidSolidPairMetricsV1>,
    pub motion: GpuFluidSolidMotionMetricsV1,
    pub particle_diagnostics: Option<GpuParticleDiagnosticsV4>,
    pub capacities: Vec<CapacityRecordV1>,
    pub body_state_finite: bool,
    pub solver_capacity_overflowed: bool,
    pub capacity_clean: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuFluidSolidOverlapEvidenceV1 {
    pub version: u32,
    pub solver_profile: &'static str,
    pub physics: physbox_launch::PhysicsConfigurationV3,
    pub coupling_quality: &'static str,
    pub adapter: AdapterDiagnostics,
    pub source_count: u32,
    pub trials_per_medium: u32,
    pub row_count: u32,
    pub capacity_clean_rows: u32,
    pub wet_sustained_overlap_rows: u32,
    pub wet_reentry_rows: u32,
    pub wet_only_issue_rows: u32,
    pub wet_only_issue_sources: u32,
    pub candidate_audit_sample_count: u32,
    pub candidate_audit_samples_with_misses: u32,
    pub candidate_audit_missed_pair_observations: u32,
    pub candidate_audit_missed_unique_pairs: Vec<CandidatePairV1>,
    pub candidate_audit_capacity_clean: bool,
    pub candidate_audit_comparison_consistent: bool,
    pub rows: Vec<GpuFluidSolidOverlapRowV1>,
    pub blocking_readback_for_advancement: bool,
    pub trajectory_sampling: &'static str,
}

pub async fn run_gpu_fluid_solid_overlap_standard_evidence(
    profile: SolverProfileConfig,
) -> Result<GpuFluidSolidOverlapEvidenceV1, String> {
    run_gpu_fluid_solid_overlap_evidence(profile, FluidSolidCouplingQuality::Standard).await
}

pub async fn run_gpu_fluid_solid_overlap_evidence(
    profile: SolverProfileConfig,
    coupling_quality: FluidSolidCouplingQuality,
) -> Result<GpuFluidSolidOverlapEvidenceV1, String> {
    let sources = fluid_solid_overlap_sources().map_err(|error| error.to_string())?;
    let instance = create_instance();
    let adapter = request_adapter(&instance, None).await?;
    let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
    let (device, queue) = request_device(&adapter).await?;
    let mut rows =
        Vec::with_capacity(sources.len() * 2 * FLUID_SOLID_OVERLAP_TRIALS_PER_MEDIUM as usize);
    for source in &sources {
        for medium in [FluidSolidOverlapMedium::Dry, FluidSolidOverlapMedium::Wet] {
            for trial_index in 0..FLUID_SOLID_OVERLAP_TRIALS_PER_MEDIUM {
                rows.push(run_row(
                    &device,
                    &queue,
                    source,
                    &profile,
                    coupling_quality,
                    medium,
                    trial_index,
                )?);
            }
        }
    }
    let capacity_clean_rows = rows.iter().filter(|row| row.capacity_clean).count() as u32;
    let wet_sustained_overlap_rows = rows
        .iter()
        .filter(|row| {
            row.medium == FluidSolidOverlapMedium::Wet
                && row.pairs.iter().any(|pair| pair.sustained_overlap)
        })
        .count() as u32;
    let wet_reentry_rows = rows
        .iter()
        .filter(|row| {
            row.medium == FluidSolidOverlapMedium::Wet
                && row.pairs.iter().any(|pair| pair.reentry_events != 0)
        })
        .count() as u32;
    let wet_only_issue_rows = rows
        .iter()
        .filter(|row| wet_only_issue_row(&rows, row))
        .count() as u32;
    let wet_only_issue_sources = sources
        .iter()
        .filter(|source| {
            rows.iter()
                .any(|row| row.source_id == source.id && wet_only_issue_row(&rows, row))
        })
        .count() as u32;
    let candidate_audits = rows
        .iter()
        .filter_map(|row| row.post_fluid_candidate_audit.as_ref())
        .collect::<Vec<_>>();
    let candidate_audit_sample_count = candidate_audits
        .iter()
        .map(|audit| audit.sample_count)
        .sum();
    let candidate_audit_samples_with_misses = candidate_audits
        .iter()
        .map(|audit| audit.samples_with_misses)
        .sum();
    let candidate_audit_missed_pair_observations = candidate_audits
        .iter()
        .map(|audit| audit.missed_pair_observations)
        .sum();
    let candidate_audit_missed_unique_pairs = candidate_audits
        .iter()
        .flat_map(|audit| audit.missed_unique_pairs.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let candidate_audit_capacity_clean = candidate_audits.iter().all(|audit| audit.capacity_clean);
    let candidate_audit_comparison_consistent = candidate_audits
        .iter()
        .all(|audit| audit.comparison_consistent);
    Ok(GpuFluidSolidOverlapEvidenceV1 {
        version: GPU_FLUID_SOLID_OVERLAP_EVIDENCE_VERSION,
        solver_profile: profile.name(),
        physics: profile.physics(),
        coupling_quality: coupling_quality.name(),
        adapter: diagnostics,
        source_count: sources.len() as u32,
        trials_per_medium: FLUID_SOLID_OVERLAP_TRIALS_PER_MEDIUM,
        row_count: rows.len() as u32,
        capacity_clean_rows,
        wet_sustained_overlap_rows,
        wet_reentry_rows,
        wet_only_issue_rows,
        wet_only_issue_sources,
        candidate_audit_sample_count,
        candidate_audit_samples_with_misses,
        candidate_audit_missed_pair_observations,
        candidate_audit_missed_unique_pairs,
        candidate_audit_capacity_clean,
        candidate_audit_comparison_consistent,
        rows,
        blocking_readback_for_advancement: false,
        trajectory_sampling: "bounded headless validation readback after admission and every four recovery steps",
    })
}

fn run_row(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: &FluidSolidOverlapSourceV1,
    profile: &SolverProfileConfig,
    coupling_quality: FluidSolidCouplingQuality,
    medium: FluidSolidOverlapMedium,
    trial_index: u32,
) -> Result<GpuFluidSolidOverlapRowV1, String> {
    source.validate().map_err(|error| error.to_string())?;
    let source_hash = canonical_hash(source).map_err(|error| error.to_string())?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let world = match medium {
        FluidSolidOverlapMedium::Dry => {
            let mut scenario = source.dry.clone();
            profile.apply_to_scenario(&mut scenario)?;
            GpuBodyWorld::new_generated_with_solver_profile(
                device,
                format,
                &scenario,
                WorldCapacityPreset::Slots64,
                profile,
            )?
        }
        FluidSolidOverlapMedium::Wet => {
            let mut scenario = source.wet.clone();
            profile.apply_to_scenario(&mut scenario.rigid)?;
            GpuBodyWorld::new_coupled_with_solver_profile(
                device,
                format,
                &scenario,
                WorldCapacityPreset::Slots64,
                profile,
                coupling_quality,
            )?
        }
    };

    encode_empty_steps(device, queue, &world, source.settle_steps)?;
    encode_construction_step(device, queue, &world, &source.construction)?;
    let construction = read_construction_state(device, queue, &world)?;
    require_construction_success(source, &construction)?;

    let audit_retained_candidates = medium == FluidSolidOverlapMedium::Wet
        && coupling_quality == FluidSolidCouplingQuality::Stabilized;
    let mut candidate_audit_samples = Vec::new();
    if audit_retained_candidates {
        candidate_audit_samples.push(read_post_fluid_candidate_audit(device, queue, &world, 0)?);
    }
    let mut samples = vec![(
        0,
        read_body_states(device, queue, &world, &source.constructed_bodies)?,
    )];
    let mut completed = 0;
    while completed < source.recovery_steps {
        let steps = source.sample_stride.min(source.recovery_steps - completed);
        encode_empty_steps(device, queue, &world, steps)?;
        completed += steps;
        if audit_retained_candidates {
            candidate_audit_samples.push(read_post_fluid_candidate_audit(
                device, queue, &world, completed,
            )?);
        }
        samples.push((
            completed,
            read_body_states(device, queue, &world, &source.constructed_bodies)?,
        ));
    }

    let pairs = pair_metrics(source, &samples)?;
    let motion = motion_metrics(&samples)?;
    let rigid_contacts = read_rigid_contact_diagnostics(device, queue, &world)?;
    let post_fluid_candidate_audit = audit_retained_candidates
        .then(|| summarize_post_fluid_candidate_audit(candidate_audit_samples));
    let particle_diagnostics = if medium == FluidSolidOverlapMedium::Wet {
        Some(read_particle_diagnostics(device, queue, &world)?)
    } else {
        None
    };
    let capacities = world.benchmark_capacities(320, 200);
    for capacity in &capacities {
        capacity.validate().map_err(|error| error.to_string())?;
    }
    let body_state_finite = samples.iter().all(|(_, states)| {
        states.iter().all(|body| {
            body.position.x.is_finite()
                && body.position.y.is_finite()
                && body.angle.get().is_finite()
                && body.linear_velocity.x.is_finite()
                && body.linear_velocity.y.is_finite()
                && body.angular_velocity.get().is_finite()
        })
    });
    let solver_capacity_overflowed = world.solver_capacity_overflowed();
    let capacity_clean = body_state_finite
        && rigid_contacts.state_finite
        && !rigid_contacts.coloring_overflowed
        && rigid_contacts.parallel_uncolored_units == 0
        && !solver_capacity_overflowed
        && capacities.iter().all(|capacity| !capacity.overflowed)
        && post_fluid_candidate_audit
            .as_ref()
            .is_none_or(|audit| audit.capacity_clean && audit.comparison_consistent)
        && particle_diagnostics.as_ref().is_none_or(|diagnostics| {
            diagnostics.state_finite
                && diagnostics.capacity_clean
                && diagnostics.overflow_flags == 0
                && diagnostics.grid_overflow == 0
                && diagnostics.body_contribution_overflow == 0
                && diagnostics.command_drops == 0
        });
    Ok(GpuFluidSolidOverlapRowV1 {
        source_id: source.id.clone(),
        source_hash,
        trial_index,
        arrangement: source.arrangement,
        particle_phase_index: source.particle_phase_index,
        particle_phase_offset: source.particle_phase_offset,
        medium,
        coupling_quality: coupling_quality.name(),
        settle_steps: source.settle_steps,
        recovery_steps: source.recovery_steps,
        sample_stride: source.sample_stride,
        solver_dispatches_per_step: world.solver_dispatches_per_step(),
        solver_allocated_bytes: world.solver_capacity_bytes(),
        post_fluid_stabilization_dispatches_per_step: world
            .post_fluid_stabilization_dispatches_per_step(),
        post_fluid_stabilization_allocated_bytes: world.post_fluid_stabilization_allocated_bytes(),
        particle_dispatches_per_step: world.particle_dispatches_per_step(),
        body_lifecycle_dispatches_per_step: world.body_lifecycle_dispatches_per_step(),
        construction,
        rigid_contacts,
        post_fluid_candidate_audit,
        pairs,
        motion,
        particle_diagnostics,
        capacities,
        body_state_finite,
        solver_capacity_overflowed,
        capacity_clean,
    })
}

fn encode_empty_steps(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
    steps: u32,
) -> Result<(), String> {
    let construction = empty_construction();
    let mut remaining = steps;
    while remaining != 0 {
        let encoded = remaining.min(MAX_ENCODED_STEPS);
        let ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; encoded as usize];
        encode_steps(device, queue, world, encoded, &construction, &ranges)?;
        remaining -= encoded;
    }
    Ok(())
}

fn encode_construction_step(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
    construction: &ConstructionIntentBatchV1,
) -> Result<(), String> {
    encode_steps(
        device,
        queue,
        world,
        1,
        construction,
        &[BodyCommandStepRange {
            start: 0,
            count: construction.intents.len() as u32,
        }],
    )
}

fn encode_steps(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
    steps: u32,
    construction: &ConstructionIntentBatchV1,
    construction_ranges: &[BodyCommandStepRange],
) -> Result<(), String> {
    let commands = BodyCommandBatchV1 {
        schema_version: SCHEMA_VERSION,
        capacity_commands: BODY_COMMAND_CAPACITY,
        attempted_commands: 0,
        overflowed: false,
        commands: Vec::new(),
    };
    let grab = empty_grab_intent_batch();
    let ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize];
    if construction_ranges.len() != steps as usize {
        return Err("fluid/solid construction ranges must match admission steps".to_owned());
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_fluid_solid_overlap_steps"),
    });
    world.encode_steps_with_grab_intents(
        queue,
        &mut encoder,
        steps,
        BodyStepPlan {
            batch: &commands,
            ranges: &ranges,
            grab_batch: &grab,
            grab_ranges: &ranges,
            construction_batch: construction,
            construction_ranges,
            particle_commands: &[],
            particle_ranges: &ranges,
            frame_timestamps: None,
        },
    )?;
    queue.submit([encoder.finish()]);
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("wait for fluid/solid overlap steps: {error}"))?;
    Ok(())
}

fn empty_construction() -> ConstructionIntentBatchV1 {
    ConstructionIntentBatchV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: physbox_contracts::CONSTRUCTION_INTENT_CONTRACT_VERSION,
        capacity_intents: crate::construction::CONSTRUCTION_INTENT_CAPACITY,
        attempted_intents: 0,
        overflowed: false,
        intents: Vec::new(),
    }
}

fn read_construction_state(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
) -> Result<ConstructionStateV1, String> {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_fluid_solid_overlap_construction_readback"),
        size: u64::from(CONSTRUCTION_STATE_BYTES),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_fluid_solid_overlap_construction_copy"),
    });
    world.copy_construction_state(&mut encoder, &buffer);
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let state = construction_state_from_bytes(&mapped)?;
    drop(mapped);
    buffer.unmap();
    Ok(state)
}

fn require_construction_success(
    source: &FluidSolidOverlapSourceV1,
    state: &ConstructionStateV1,
) -> Result<(), String> {
    state.validate().map_err(|error| error.to_string())?;
    let intended = source.construction.intents.len() as u64;
    if state.attempted_intents != intended
        || state.succeeded_intents != intended
        || state.live_bodies != 3 + intended as u32
        || state.capacity_failures != 0
        || state.deep_overlap_failures != 0
        || state.generation_failures != 0
    {
        return Err(format!(
            "{} {} construction did not admit every permissive overlap: {state:?}",
            source.id, intended
        ));
    }
    Ok(())
}

fn read_body_states(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
    templates: &[BodyStateV1],
) -> Result<Vec<BodyStateV1>, String> {
    let capacity = world.body_capacity();
    let size = u64::from(capacity) * size_of::<[u32; 4]>() as u64 * 3;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_fluid_solid_overlap_body_readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_fluid_solid_overlap_body_copy"),
    });
    world.copy_full_validation_state(&mut encoder, &buffer);
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let states = decode_body_states(templates, capacity, &mapped)?;
    drop(mapped);
    buffer.unmap();
    Ok(states)
}

fn decode_body_states(
    templates: &[BodyStateV1],
    capacity: u32,
    bytes: &[u8],
) -> Result<Vec<BodyStateV1>, String> {
    let lane_bytes = capacity as usize * size_of::<[u32; 4]>();
    if bytes.len() != lane_bytes * 3 {
        return Err("fluid/solid overlap body readback has the wrong byte count".to_owned());
    }
    let poses = bytemuck::try_cast_slice::<u8, [f32; 4]>(&bytes[..lane_bytes])
        .map_err(|error| format!("decode fluid/solid overlap poses: {error}"))?;
    let motions = bytemuck::try_cast_slice::<u8, [f32; 4]>(&bytes[lane_bytes..lane_bytes * 2])
        .map_err(|error| format!("decode fluid/solid overlap motions: {error}"))?;
    let metadata = bytemuck::try_cast_slice::<u8, [u32; 4]>(&bytes[lane_bytes * 2..])
        .map_err(|error| format!("decode fluid/solid overlap metadata: {error}"))?;
    templates
        .iter()
        .map(|template| {
            let slot = template.id.slot as usize;
            let pose = *poses
                .get(slot)
                .ok_or_else(|| "constructed body slot exceeds GPU capacity".to_owned())?;
            let motion = motions[slot];
            let identity = metadata[slot];
            if identity[0] != template.id.generation || identity[1] & 1 == 0 {
                return Err(format!(
                    "constructed body {}:{} is not live",
                    template.id.slot, template.id.generation
                ));
            }
            if !pose.into_iter().all(f32::is_finite) || !motion.into_iter().all(f32::is_finite) {
                return Err("fluid/solid overlap body sample is non-finite".to_owned());
            }
            let mut state = template.clone();
            state.position = Vec2::new(pose[0], pose[1]);
            state.angle =
                physbox_contracts::FiniteF32::new(pose[2]).map_err(|error| error.to_string())?;
            state.linear_velocity = Vec2::new(motion[0], motion[1]);
            state.angular_velocity =
                physbox_contracts::FiniteF32::new(motion[2]).map_err(|error| error.to_string())?;
            state.sleeping = identity[1] & (1 << 1) != 0;
            state.validate().map_err(|error| error.to_string())?;
            Ok(state)
        })
        .collect()
}

fn read_particle_diagnostics(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
) -> Result<GpuParticleDiagnosticsV4, String> {
    let layout = world
        .particle_validation_layout()
        .ok_or_else(|| "wet fluid/solid overlap row has no particle layout".to_owned())?;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_fluid_solid_overlap_particle_readback"),
        size: layout.total_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_fluid_solid_overlap_particle_copy"),
    });
    world.copy_particle_validation_state(&mut encoder, &buffer)?;
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let diagnostics = world.decode_particle_validation_state(&mapped)?;
    drop(mapped);
    buffer.unmap();
    Ok(diagnostics)
}

fn read_rigid_contact_diagnostics(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
) -> Result<GpuRigidContactDiagnosticsV1, String> {
    let layout = world.floor_pass_evidence_layout()?;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_fluid_solid_overlap_contact_readback"),
        size: layout.total_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_fluid_solid_overlap_contact_copy"),
    });
    world.copy_floor_pass_evidence(&mut encoder, &buffer)?;
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let evidence = decode_pass_evidence(&mapped, layout)?;
    drop(mapped);
    buffer.unmap();
    let active = evidence
        .contacts
        .iter()
        .filter(|contact| contact.valid)
        .collect::<Vec<_>>();
    let state_finite = active.iter().all(|contact| {
        [
            contact.normal.x,
            contact.normal.y,
            contact.tangent.x,
            contact.tangent.y,
            contact.anchor_a.x,
            contact.anchor_a.y,
            contact.anchor_b.x,
            contact.anchor_b.y,
            contact.initial_normal_velocity,
            contact.normal_effective_mass,
            contact.tangent_effective_mass,
            contact.separation,
            contact.normal_impulse,
            contact.tangent_impulse,
            contact.total_normal_impulse,
        ]
        .into_iter()
        .all(f32::is_finite)
    });
    let minimum_separation = active
        .iter()
        .map(|contact| contact.separation)
        .reduce(f32::min);
    let maximum_normal_impulse = active
        .iter()
        .map(|contact| contact.normal_impulse.abs())
        .fold(0.0, f32::max);
    let (coloring_overflowed, parallel_uncolored_units) =
        evidence.coloring.as_ref().map_or((false, 0), |coloring| {
            (coloring.overflowed, coloring.parallel_uncolored_units)
        });
    Ok(GpuRigidContactDiagnosticsV1 {
        contact_point_capacity: world.contact_point_count(),
        active_contact_points: active.len() as u32,
        active_manifolds: evidence.manifold_blocks.len() as u32,
        minimum_separation,
        maximum_normal_impulse,
        state_finite,
        coloring_overflowed,
        parallel_uncolored_units,
    })
}

struct DecodedPostFluidContactSet {
    pairs: BTreeSet<CandidatePairV1>,
    overflowed: bool,
}

fn read_post_fluid_candidate_audit(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    world: &GpuBodyWorld,
    recovery_step: u32,
) -> Result<GpuPostFluidCandidateAuditSampleV1, String> {
    let bytes = world.post_fluid_candidate_audit_bytes()?;
    let readback = |label| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        })
    };
    let retained = readback("physbox5_post_fluid_retained_contact_audit");
    let complete = readback("physbox5_post_fluid_complete_contact_audit");
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_post_fluid_candidate_audit"),
    });
    world.encode_post_fluid_candidate_audit(&mut encoder, &retained, &complete)?;
    queue.submit([encoder.finish()]);
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("wait for post-fluid candidate audit: {error}"))?;

    let retained_mapped = map_buffer(device, &retained)?;
    let retained_set = decode_post_fluid_contact_set(&retained_mapped)?;
    drop(retained_mapped);
    retained.unmap();
    let complete_mapped = map_buffer(device, &complete)?;
    let complete_set = decode_post_fluid_contact_set(&complete_mapped)?;
    drop(complete_mapped);
    complete.unmap();

    let missed_pairs = complete_set
        .pairs
        .difference(&retained_set.pairs)
        .copied()
        .collect();
    let retained_only_pairs = retained_set
        .pairs
        .difference(&complete_set.pairs)
        .copied()
        .collect();
    Ok(GpuPostFluidCandidateAuditSampleV1 {
        recovery_step,
        retained_contact_pairs: retained_set.pairs.len() as u32,
        complete_contact_pairs: complete_set.pairs.len() as u32,
        missed_pairs,
        retained_only_pairs,
        retained_overflowed: retained_set.overflowed,
        complete_overflowed: complete_set.overflowed,
    })
}

fn decode_post_fluid_contact_set(bytes: &[u8]) -> Result<DecodedPostFluidContactSet, String> {
    let capacity_bytes = COLLISION_CAPACITY_COUNTER_BYTES as usize;
    let manifold_bytes = COLLISION_MANIFOLD_BYTES as usize;
    if bytes.len() < capacity_bytes || (bytes.len() - capacity_bytes) % manifold_bytes != 0 {
        return Err("post-fluid candidate audit has an invalid byte count".to_owned());
    }
    let capacity = bytemuck::pod_read_unaligned::<[u32; 4]>(&bytes[..capacity_bytes]);
    let manifold_capacity = (bytes.len() - capacity_bytes) / manifold_bytes;
    let manifold_count = capacity[2] as usize;
    if manifold_count > manifold_capacity {
        return Err(format!(
            "post-fluid candidate audit reports {manifold_count} manifolds above capacity {manifold_capacity}"
        ));
    }
    let mut pairs = BTreeSet::new();
    for index in 0..manifold_count {
        let start = capacity_bytes + index * manifold_bytes;
        let words =
            bytemuck::pod_read_unaligned::<[u32; 12]>(&bytes[start..start + manifold_bytes]);
        let pair = CandidatePairV1 {
            body_a: BodyId {
                slot: words[0],
                generation: words[1],
            },
            body_b: BodyId {
                slot: words[2],
                generation: words[3],
            },
        };
        if pair.body_a >= pair.body_b {
            return Err("post-fluid candidate audit emitted a noncanonical pair".to_owned());
        }
        if !pairs.insert(pair) {
            return Err("post-fluid candidate audit emitted a duplicate pair".to_owned());
        }
    }
    Ok(DecodedPostFluidContactSet {
        pairs,
        overflowed: capacity[3] != 0,
    })
}

fn summarize_post_fluid_candidate_audit(
    samples: Vec<GpuPostFluidCandidateAuditSampleV1>,
) -> GpuPostFluidCandidateAuditV1 {
    let samples_with_misses = samples
        .iter()
        .filter(|sample| !sample.missed_pairs.is_empty())
        .count() as u32;
    let missed_pair_observations = samples
        .iter()
        .map(|sample| sample.missed_pairs.len() as u32)
        .sum();
    let missed_unique_pairs = samples
        .iter()
        .flat_map(|sample| sample.missed_pairs.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let retained_only_pair_observations = samples
        .iter()
        .map(|sample| sample.retained_only_pairs.len() as u32)
        .sum();
    let retained_only_unique_pairs = samples
        .iter()
        .flat_map(|sample| sample.retained_only_pairs.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let capacity_clean = samples
        .iter()
        .all(|sample| !sample.retained_overflowed && !sample.complete_overflowed);
    let comparison_consistent = retained_only_unique_pairs.is_empty();
    GpuPostFluidCandidateAuditV1 {
        sample_count: samples.len() as u32,
        samples_with_misses,
        missed_pair_observations,
        missed_unique_pairs,
        retained_only_pair_observations,
        retained_only_unique_pairs,
        capacity_clean,
        comparison_consistent,
        samples,
    }
}

fn pair_metrics(
    source: &FluidSolidOverlapSourceV1,
    samples: &[(u32, Vec<BodyStateV1>)],
) -> Result<Vec<GpuFluidSolidPairMetricsV1>, String> {
    if samples.len() < FINAL_WINDOW_SAMPLES {
        return Err("fluid/solid overlap evidence has too few trajectory samples".to_owned());
    }
    source
        .monitored_pairs
        .iter()
        .map(|pair| {
            let authored_left = body_by_id(&source.constructed_bodies, pair[0])?;
            let authored_right = body_by_id(&source.constructed_bodies, pair[1])?;
            let authored_separation = signed_shape_separation(authored_left, authored_right);
            let separations = samples
                .iter()
                .map(|(_, states)| {
                    Ok(signed_shape_separation(
                        body_by_id(states, pair[0])?,
                        body_by_id(states, pair[1])?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if separations.iter().any(|value| !value.is_finite()) {
                return Err("fluid/solid overlap separation is non-finite".to_owned());
            }
            let mut separation_events = 0;
            let mut reentry_events = 0;
            let mut previous_overlap = significant_overlap(separations[0]);
            let mut separated_after_admission = !previous_overlap;
            for separation in separations.iter().skip(1).copied() {
                let overlap = significant_overlap(separation);
                if previous_overlap && !overlap {
                    separation_events += 1;
                    separated_after_admission = true;
                } else if !previous_overlap && overlap {
                    reentry_events += 1;
                }
                previous_overlap = overlap;
            }
            let final_window = &separations[separations.len() - FINAL_WINDOW_SAMPLES..];
            let final_window_overlap_samples = final_window
                .iter()
                .filter(|value| significant_overlap(**value))
                .count() as u32;
            Ok(GpuFluidSolidPairMetricsV1 {
                body_a: pair[0],
                body_b: pair[1],
                authored_separation,
                post_admission_separation: separations[0],
                worst_separation: separations.iter().copied().fold(f32::INFINITY, f32::min),
                final_separation: *separations.last().unwrap(),
                final_window_min_separation: final_window
                    .iter()
                    .copied()
                    .fold(f32::INFINITY, f32::min),
                final_window_mean_separation: final_window.iter().sum::<f32>()
                    / final_window.len() as f32,
                overlap_samples: separations
                    .iter()
                    .filter(|value| significant_overlap(**value))
                    .count() as u32,
                final_window_overlap_samples,
                separation_events,
                reentry_events,
                separated_after_admission,
                sustained_overlap: final_window_overlap_samples == final_window.len() as u32,
            })
        })
        .collect()
}

fn motion_metrics(
    samples: &[(u32, Vec<BodyStateV1>)],
) -> Result<GpuFluidSolidMotionMetricsV1, String> {
    let energies = samples
        .iter()
        .map(|(_, states)| kinetic_energy(states))
        .collect::<Result<Vec<_>, String>>()?;
    let mut max_linear_speed = 0.0f32;
    let mut max_abs_angular_speed = 0.0f32;
    for (_, states) in samples {
        for body in states {
            max_linear_speed = max_linear_speed.max(length(body.linear_velocity));
            max_abs_angular_speed = max_abs_angular_speed.max(body.angular_velocity.get().abs());
        }
    }
    Ok(GpuFluidSolidMotionMetricsV1 {
        sample_count: samples.len() as u32,
        max_linear_speed,
        max_abs_angular_speed,
        initial_kinetic_energy: energies[0],
        maximum_kinetic_energy: energies.iter().copied().fold(0.0, f32::max),
        final_kinetic_energy: *energies.last().unwrap(),
        maximum_sample_energy_increase: energies
            .windows(2)
            .map(|values| (values[1] - values[0]).max(0.0))
            .fold(0.0, f32::max),
    })
}

fn kinetic_energy(states: &[BodyStateV1]) -> Result<f32, String> {
    states.iter().try_fold(0.0, |sum, body| {
        let inverse_mass = body.inverse_mass.get();
        let inverse_inertia = body.inverse_inertia.get();
        if inverse_mass <= 0.0 || inverse_inertia <= 0.0 {
            return Err("fluid/solid overlap dynamic body has nonpositive inverse mass".to_owned());
        }
        let linear = body.linear_velocity;
        let angular = body.angular_velocity.get();
        let energy = 0.5 / inverse_mass * (linear.x * linear.x + linear.y * linear.y)
            + 0.5 / inverse_inertia * angular * angular;
        if !energy.is_finite() {
            return Err("fluid/solid overlap kinetic energy is non-finite".to_owned());
        }
        Ok(sum + energy)
    })
}

fn body_by_id(states: &[BodyStateV1], id: BodyId) -> Result<&BodyStateV1, String> {
    states
        .binary_search_by_key(&id, |body| body.id)
        .map(|index| &states[index])
        .map_err(|_| {
            format!(
                "missing fluid/solid overlap body {}:{}",
                id.slot, id.generation
            )
        })
}

fn signed_shape_separation(left: &BodyStateV1, right: &BodyStateV1) -> f32 {
    match (left.shape, right.shape) {
        (
            ShapeV1::Circle {
                radius: left_radius,
            },
            ShapeV1::Circle {
                radius: right_radius,
            },
        ) => {
            length(subtract(right.position, left.position)) - left_radius.get() - right_radius.get()
        }
        (ShapeV1::Circle { radius }, ShapeV1::Rectangle { half_extents }) => {
            circle_rectangle_separation(left.position, radius.get(), right, half_extents)
        }
        (ShapeV1::Rectangle { half_extents }, ShapeV1::Circle { radius }) => {
            circle_rectangle_separation(right.position, radius.get(), left, half_extents)
        }
        (
            ShapeV1::Rectangle {
                half_extents: left_half,
            },
            ShapeV1::Rectangle {
                half_extents: right_half,
            },
        ) => rectangle_rectangle_separation(left, left_half, right, right_half),
    }
}

fn circle_rectangle_separation(
    circle_position: Vec2,
    circle_radius: f32,
    rectangle: &BodyStateV1,
    half_extents: Vec2,
) -> f32 {
    let local = inverse_rotate(
        subtract(circle_position, rectangle.position),
        rectangle.angle.get(),
    );
    let q = Vec2::new(
        local.x.abs() - half_extents.x,
        local.y.abs() - half_extents.y,
    );
    let outside = length(Vec2::new(q.x.max(0.0), q.y.max(0.0)));
    let inside = q.x.max(q.y).min(0.0);
    outside + inside - circle_radius
}

fn rectangle_rectangle_separation(
    left: &BodyStateV1,
    left_half: Vec2,
    right: &BodyStateV1,
    right_half: Vec2,
) -> f32 {
    let left_axes = axes(left.angle.get());
    let right_axes = axes(right.angle.get());
    let center_delta = subtract(right.position, left.position);
    left_axes
        .into_iter()
        .chain(right_axes)
        .map(|axis| {
            dot(center_delta, axis).abs()
                - projected_radius(left_half, left_axes, axis)
                - projected_radius(right_half, right_axes, axis)
        })
        .fold(f32::NEG_INFINITY, f32::max)
}

fn axes(angle: f32) -> [Vec2; 2] {
    let cosine = angle.cos();
    let sine = angle.sin();
    [Vec2::new(cosine, sine), Vec2::new(-sine, cosine)]
}

fn projected_radius(half_extents: Vec2, axes: [Vec2; 2], axis: Vec2) -> f32 {
    half_extents.x * dot(axes[0], axis).abs() + half_extents.y * dot(axes[1], axis).abs()
}

fn inverse_rotate(value: Vec2, angle: f32) -> Vec2 {
    let cosine = angle.cos();
    let sine = angle.sin();
    Vec2::new(
        cosine * value.x + sine * value.y,
        -sine * value.x + cosine * value.y,
    )
}

fn significant_overlap(separation: f32) -> bool {
    separation < -FLUID_SOLID_SIGNIFICANT_OVERLAP
}

fn wet_only_issue_row(rows: &[GpuFluidSolidOverlapRowV1], wet: &GpuFluidSolidOverlapRowV1) -> bool {
    if wet.medium != FluidSolidOverlapMedium::Wet {
        return false;
    }
    wet.pairs.iter().any(|wet_pair| {
        let wet_issue = wet_pair.sustained_overlap || wet_pair.reentry_events != 0;
        let dry_issue = rows
            .iter()
            .filter(|row| {
                row.source_id == wet.source_id && row.medium == FluidSolidOverlapMedium::Dry
            })
            .flat_map(|row| &row.pairs)
            .filter(|dry_pair| {
                dry_pair.body_a == wet_pair.body_a && dry_pair.body_b == wet_pair.body_b
            })
            .any(|dry_pair| dry_pair.sustained_overlap || dry_pair.reentry_events != 0);
        wet_issue && !dry_issue
    })
}

fn subtract(left: Vec2, right: Vec2) -> Vec2 {
    Vec2::new(left.x - right.x, left.y - right.y)
}

fn dot(left: Vec2, right: Vec2) -> f32 {
    left.x * right.x + left.y * right.y
}

fn length(value: Vec2) -> f32 {
    (value.x * value.x + value.y * value.y).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use physbox_contracts::{FiniteF32, MaterialV1};

    #[test]
    fn signed_separation_handles_all_shape_pairs_and_rotation() {
        let circle_left = test_body(
            0,
            ShapeV1::Circle {
                radius: FiniteF32::new(0.25).unwrap(),
            },
            Vec2::new(-0.17, 0.0),
            0.0,
        );
        let circle_right = test_body(
            1,
            ShapeV1::Circle {
                radius: FiniteF32::new(0.25).unwrap(),
            },
            Vec2::new(0.17, 0.0),
            0.0,
        );
        assert_close(signed_shape_separation(&circle_left, &circle_right), -0.16);

        let rectangle_left = test_body(
            2,
            ShapeV1::Rectangle {
                half_extents: Vec2::new(0.3, 0.2),
            },
            Vec2::new(-0.21, 0.0),
            0.0,
        );
        let rectangle_right = test_body(
            3,
            ShapeV1::Rectangle {
                half_extents: Vec2::new(0.3, 0.2),
            },
            Vec2::new(0.21, 0.0),
            0.0,
        );
        assert_close(
            signed_shape_separation(&rectangle_left, &rectangle_right),
            -0.18,
        );
        assert_close(
            signed_shape_separation(&circle_left, &rectangle_right),
            -0.17,
        );

        let rotated = test_body(
            4,
            ShapeV1::Rectangle {
                half_extents: Vec2::new(0.3, 0.2),
            },
            Vec2::new(0.0, 0.0),
            std::f32::consts::FRAC_PI_2,
        );
        let separated_circle = test_body(
            5,
            ShapeV1::Circle {
                radius: FiniteF32::new(0.25).unwrap(),
            },
            Vec2::new(0.0, 0.5),
            0.0,
        );
        assert_close(signed_shape_separation(&rotated, &separated_circle), -0.05);
    }

    #[test]
    fn pair_metrics_count_separation_reentry_and_sustained_overlap() {
        let mut source =
            crate::fluid_solid_overlap_source(FluidSolidOverlapArrangement::CirclePair, 0).unwrap();
        let authored = source.constructed_bodies.clone();
        let mut separated = authored.clone();
        separated[1].position.x += 0.4;
        let mut reentered = authored.clone();
        reentered[1].position.x += 0.1;
        let samples = vec![
            (0, authored),
            (4, separated),
            (8, reentered.clone()),
            (12, reentered.clone()),
            (16, reentered.clone()),
            (20, reentered.clone()),
            (24, reentered),
        ];
        source.recovery_steps = 24;
        let metrics = pair_metrics(&source, &samples).unwrap();
        assert_eq!(metrics[0].separation_events, 1);
        assert_eq!(metrics[0].reentry_events, 1);
        assert!(metrics[0].sustained_overlap);
    }

    #[test]
    fn post_fluid_candidate_audit_reports_complete_search_misses() {
        let retained = audit_sample_bytes(&[(3, 1, 4, 1)]);
        let complete = audit_sample_bytes(&[(3, 1, 4, 1), (4, 1, 5, 2)]);
        let retained = decode_post_fluid_contact_set(&retained).unwrap();
        let complete = decode_post_fluid_contact_set(&complete).unwrap();
        let missed = complete
            .pairs
            .difference(&retained.pairs)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            missed,
            vec![CandidatePairV1 {
                body_a: BodyId {
                    slot: 4,
                    generation: 1,
                },
                body_b: BodyId {
                    slot: 5,
                    generation: 2,
                },
            }]
        );
        assert!(!retained.overflowed);
        assert!(!complete.overflowed);
    }

    fn audit_sample_bytes(pairs: &[(u32, u32, u32, u32)]) -> Vec<u8> {
        let capacity_bytes = COLLISION_CAPACITY_COUNTER_BYTES as usize;
        let manifold_bytes = COLLISION_MANIFOLD_BYTES as usize;
        let mut bytes = vec![0; capacity_bytes + pairs.len().max(2) * manifold_bytes];
        let capacity = [
            pairs.len() as u32,
            pairs.len() as u32,
            pairs.len() as u32,
            0,
        ];
        bytes[..capacity_bytes].copy_from_slice(bytemuck::bytes_of(&capacity));
        for (index, pair) in pairs.iter().copied().enumerate() {
            let mut words = [0_u32; 12];
            words[..4].copy_from_slice(&[pair.0, pair.1, pair.2, pair.3]);
            let start = capacity_bytes + index * manifold_bytes;
            bytes[start..start + manifold_bytes].copy_from_slice(bytemuck::bytes_of(&words));
        }
        bytes
    }

    fn test_body(slot: u32, shape: ShapeV1, position: Vec2, angle: f32) -> BodyStateV1 {
        BodyStateV1 {
            id: BodyId {
                slot,
                generation: 1,
            },
            shape,
            position,
            angle: FiniteF32::new(angle).unwrap(),
            linear_velocity: Vec2::ZERO,
            angular_velocity: FiniteF32::new(0.0).unwrap(),
            inverse_mass: FiniteF32::new(1.0).unwrap(),
            inverse_inertia: FiniteF32::new(1.0).unwrap(),
            material: MaterialV1 {
                friction: FiniteF32::new(0.5).unwrap(),
                restitution: FiniteF32::new(0.0).unwrap(),
            },
            sleeping: false,
        }
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1.0e-5,
            "{actual} != {expected}"
        );
    }
}
