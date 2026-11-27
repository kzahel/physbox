use std::collections::BTreeMap;
use std::mem::size_of;
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

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
use crate::physics_render::{PhysicsRenderer, RenderOptions, RenderTarget, ViewTransform};
use crate::{
    LIQUID_SCALE_ARTIFACT_VERSION, LIQUID_SCALE_PERCENTILE_METHOD, LiquidScaleBenchmarkArtifactV1,
    LiquidScaleBenchmarkConfigV1, LiquidScaleBenchmarkSourceV1, LiquidScaleCorrectnessV1,
    LiquidScaleGpuStageTimingV1, LiquidScaleGpuStageV1, LiquidScaleGpuTimestampStatusV1,
    LiquidScaleHostKindV1, LiquidScaleTimingModeV1, LiquidScaleTimingSeriesV1,
    LiquidScaleTimingsV1, LiquidScaleWorkVectorV1, SolverProfileConfig,
    liquid_scale_benchmark_source_with_capacity,
};

const FRAME_QUERY_COUNT: u32 = 3;

pub struct LiquidScalePhysicsBenchmarkRun {
    pub artifact: LiquidScaleBenchmarkArtifactV1,
    pub adapter: AdapterDiagnostics,
}

pub struct LiquidScaleBenchmarkSession {
    device: wgpu::Device,
    queue: wgpu::Queue,
    world: GpuBodyWorld,
    particle_step_status: wgpu::Buffer,
    source: LiquidScaleBenchmarkSourceV1,
    scenario: physbox_contracts::CoupledScenarioV1,
    scenario_hash: String,
    config: LiquidScaleBenchmarkConfigV1,
    host_kind: LiquidScaleHostKindV1,
    browser_version: Option<String>,
    provenance: ProvenanceV1,
    adapter: AdapterDiagnostics,
    empty_batch: BodyCommandBatchV1,
    grab_batch: physbox_contracts::GrabIntentBatchV1,
    construction_batch: ConstructionIntentBatchV1,
    particle_stages: Vec<GpuParticleTimingStage>,
    timestamps: Option<TimestampResources>,
    renderer: Option<PhysicsRenderer>,
    _render_texture: Option<wgpu::Texture>,
    render_view: Option<wgpu::TextureView>,
    render_transform: Option<ViewTransform>,
    frame_index: u64,
    warmup_submitted: bool,
    submitted_samples: u32,
}

#[derive(Clone, Copy)]
struct PacingEvidence {
    effective_simulation_scale: f64,
    slipped_wall_seconds: f64,
}

impl LiquidScaleBenchmarkSession {
    pub async fn new(
        config: LiquidScaleBenchmarkConfigV1,
        host_kind: LiquidScaleHostKindV1,
        browser_version: Option<String>,
        provenance: ProvenanceV1,
    ) -> Result<Self, String> {
        config.validate().map_err(|error| error.to_string())?;
        if cfg!(feature = "box2d-comparator") {
            return Err("liquid benchmark rejects a comparator-enabled build".to_owned());
        }
        if cfg!(debug_assertions) {
            return Err("liquid benchmark requires a release build".to_owned());
        }
        let expected_host = if cfg!(target_arch = "wasm32") {
            LiquidScaleHostKindV1::BrowserWebGpu
        } else if cfg!(target_os = "android") {
            LiquidScaleHostKindV1::AndroidNative
        } else {
            LiquidScaleHostKindV1::NativeDesktop
        };
        if host_kind != expected_host {
            return Err(format!(
                "liquid benchmark host kind {host_kind:?} does not match {expected_host:?}"
            ));
        }
        match host_kind {
            LiquidScaleHostKindV1::BrowserWebGpu
                if browser_version.as_deref().is_none_or(str::is_empty) =>
            {
                return Err("browser liquid benchmark requires a browser version".to_owned());
            }
            LiquidScaleHostKindV1::NativeDesktop | LiquidScaleHostKindV1::AndroidNative
                if browser_version.is_some() =>
            {
                return Err("native liquid benchmark cannot carry a browser version".to_owned());
            }
            _ => {}
        }

        let source = liquid_scale_benchmark_source_with_capacity(
            config.solid_tier,
            config.particle_count,
            config.resolved_particle_capacity(),
        )
        .map_err(|error| error.to_string())?;
        let total_steps = config
            .warmup_steps
            .checked_add(config.retained_samples)
            .ok_or_else(|| "liquid benchmark step count overflowed".to_owned())?;
        if config.timing_mode == LiquidScaleTimingModeV1::PhysicsOnlyExplicitSteps
            && total_steps > source.scenario.requested_steps
        {
            return Err(format!(
                "liquid benchmark requests {total_steps} steps from source window {}",
                source.scenario.requested_steps
            ));
        }
        let mut scenario = source.scenario.clone();
        if config.timing_mode == LiquidScaleTimingModeV1::FullFrameInteractive {
            scenario.requested_steps = total_steps;
            scenario.rigid.requested_steps = total_steps;
        }
        let profile = SolverProfileConfig::default();
        profile.apply_to_scenario(&mut scenario.rigid)?;
        scenario.validate().map_err(|error| error.to_string())?;
        let scenario_hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;

        let instance = create_instance();
        let adapter = request_adapter(&instance, None).await?;
        let adapter_diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let timestamps_available = config.timing_mode
            == LiquidScaleTimingModeV1::PhysicsOnlyExplicitSteps
            && adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
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
            || world.particle_capacity() != config.resolved_particle_capacity()
        {
            return Err(
                "liquid benchmark world did not create the exact particle population".to_owned(),
            );
        }
        let (renderer, render_texture, render_view, render_transform) =
            if config.timing_mode == LiquidScaleTimingModeV1::FullFrameInteractive {
                let extent = config
                    .render_extent
                    .expect("validated full-frame liquid extent exists");
                let format = wgpu::TextureFormat::Rgba8UnormSrgb;
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("physbox5_liquid_scale_render_target"),
                    size: wgpu::Extent3d {
                        width: extent[0],
                        height: extent[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                let renderer = PhysicsRenderer::new(&device, format, world.engine_resources())
                    .map_err(|error| error.to_string())?;
                let transform = liquid_render_transform(&scenario, extent)?;
                (Some(renderer), Some(texture), Some(view), Some(transform))
            } else {
                (None, None, None, None)
            };
        let particle_stages = world.particle_timestamp_stages_per_step();
        if particle_stages.len() as u32 != world.particle_dispatches_per_step() {
            return Err(
                "liquid particle timestamp layout does not match dispatch count".to_owned(),
            );
        }
        let query_count = FRAME_QUERY_COUNT
            .checked_add(particle_stages.len() as u32)
            .ok_or_else(|| "liquid timestamp query count overflowed".to_owned())?;
        let timestamps = timestamps_available
            .then(|| TimestampResources::new(&device, query_count, config.retained_samples));

        let status_bytes = u64::from(total_steps) * size_of::<[u32; 2]>() as u64;
        let particle_layout = world
            .particle_validation_layout()
            .ok_or_else(|| "liquid benchmark world has no particle validation layout".to_owned())?;
        if particle_layout.total_bytes + status_bytes > device.limits().max_buffer_size {
            return Err("liquid benchmark step history exceeds the device buffer limit".to_owned());
        }
        // Write on the GPU each step and read once with the final validation snapshot.
        let particle_step_status = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_liquid_scale_step_status"),
            size: status_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        Ok(Self {
            particle_step_status,
            device,
            queue,
            world,
            source,
            scenario,
            scenario_hash,
            config,
            host_kind,
            browser_version,
            provenance,
            adapter: adapter_diagnostics,
            empty_batch: BodyCommandBatchV1 {
                schema_version: SCHEMA_VERSION,
                capacity_commands: BODY_COMMAND_CAPACITY,
                attempted_commands: 0,
                overflowed: false,
                commands: Vec::new(),
            },
            grab_batch: empty_grab_intent_batch(),
            construction_batch: empty_construction_intent_batch(),
            particle_stages,
            timestamps,
            renderer,
            _render_texture: render_texture,
            render_view,
            render_transform,
            frame_index: 0,
            warmup_submitted: false,
            submitted_samples: 0,
        })
    }

    pub fn submit_warmup(&mut self) -> Result<(), String> {
        if self.warmup_submitted || self.submitted_samples != 0 {
            return Err("liquid benchmark warmup may be submitted exactly once".to_owned());
        }
        let ranges = [BodyCommandStepRange { start: 0, count: 0 }];
        for step_index in 0..self.config.warmup_steps {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("physbox5_liquid_scale_warmup"),
                });
            self.world.encode_steps_with_grab_intents(
                &self.queue,
                &mut encoder,
                1,
                BodyStepPlan {
                    batch: &self.empty_batch,
                    ranges: &ranges,
                    grab_batch: &self.grab_batch,
                    grab_ranges: &ranges,
                    construction_batch: &self.construction_batch,
                    construction_ranges: &ranges,
                    particle_commands: &[],
                    particle_ranges: &ranges,
                    frame_timestamps: None,
                },
            )?;
            self.encode_render(&mut encoder)?;
            self.world.copy_particle_step_status(
                &mut encoder,
                &self.particle_step_status,
                step_index,
            )?;
            self.queue.submit([encoder.finish()]);
        }
        self.warmup_submitted = true;
        Ok(())
    }

    pub fn submit_sample(&mut self) -> Result<(), String> {
        if !self.warmup_submitted {
            return Err("liquid benchmark sample requires completed warmup submission".to_owned());
        }
        if self.submitted_samples >= self.config.retained_samples {
            return Err("liquid benchmark sample exceeds retained count".to_owned());
        }
        let sample = self.submitted_samples;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_liquid_scale_sample"),
            });
        let ranges = [BodyCommandStepRange { start: 0, count: 0 }];
        self.world.encode_steps_with_grab_intents(
            &self.queue,
            &mut encoder,
            1,
            BodyStepPlan {
                batch: &self.empty_batch,
                ranges: &ranges,
                grab_batch: &self.grab_batch,
                grab_ranges: &ranges,
                construction_batch: &self.construction_batch,
                construction_ranges: &ranges,
                particle_commands: &[],
                particle_ranges: &ranges,
                frame_timestamps: self.timestamps.as_ref().map(|timestamps| {
                    GpuFrameTimestampPlan {
                        query_set: &timestamps.query_set,
                        query_base: 0,
                        particle_query_base: Some(FRAME_QUERY_COUNT),
                    }
                }),
            },
        )?;
        self.encode_render(&mut encoder)?;
        if let Some(timestamps) = &self.timestamps {
            encoder.resolve_query_set(
                &timestamps.query_set,
                0..timestamps.query_count,
                &timestamps.resolve,
                u64::from(sample) * timestamps.sample_stride,
            );
        }
        self.world.copy_particle_step_status(
            &mut encoder,
            &self.particle_step_status,
            self.config.warmup_steps + sample,
        )?;
        self.queue.submit([encoder.finish()]);
        self.submitted_samples += 1;
        Ok(())
    }

    fn encode_render(&mut self, encoder: &mut wgpu::CommandEncoder) -> Result<(), String> {
        let Some(renderer) = &self.renderer else {
            return Ok(());
        };
        let extent = self
            .config
            .render_extent
            .expect("full-frame liquid renderer has an extent");
        renderer
            .encode(
                &self.queue,
                encoder,
                RenderTarget {
                    color_view: self
                        .render_view
                        .as_ref()
                        .expect("full-frame liquid renderer has a target view"),
                    size: extent,
                },
                RenderOptions {
                    frame_index: self.frame_index,
                    view: self
                        .render_transform
                        .expect("full-frame liquid renderer has a view transform"),
                    current_outer_step: self.frame_index.min(u64::from(u32::MAX)) as u32,
                    ..RenderOptions::default()
                },
            )
            .map_err(|error| error.to_string())?;
        self.frame_index += 1;
        Ok(())
    }

    pub fn begin_completion_fence(&self) -> PendingLiquidScaleCompletion {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_liquid_scale_completion_fence"),
            size: 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_liquid_scale_completion_encoder"),
            });
        encoder.clear_buffer(&buffer, 0, None);
        self.queue.submit([encoder.finish()]);
        PendingLiquidScaleCompletion {
            mapped: PendingMap::new(buffer),
        }
    }

    pub fn begin_final_readback(&mut self) -> Result<PendingLiquidScaleReadback, String> {
        if self.submitted_samples != self.config.retained_samples {
            return Err(format!(
                "liquid benchmark final readback requires {} samples, found {}",
                self.config.retained_samples, self.submitted_samples
            ));
        }
        let particle_layout = self
            .world
            .particle_validation_layout()
            .ok_or_else(|| "liquid benchmark world has no particle validation layout".to_owned())?;
        let particle_readback = readback_buffer(
            &self.device,
            "physbox5_liquid_scale_particle_readback",
            particle_layout.total_bytes + self.particle_step_status.size(),
        );
        let body_bytes = u64::from(self.world.body_capacity()) * size_of::<[u32; 4]>() as u64 * 3;
        let body_readback = readback_buffer(
            &self.device,
            "physbox5_liquid_scale_body_readback",
            body_bytes,
        );
        let graph_bytes = u64::from(COLLISION_CAPACITY_COUNTER_BYTES) * 4;
        let graph_readback = readback_buffer(
            &self.device,
            "physbox5_liquid_scale_graph_readback",
            graph_bytes,
        );
        let timestamps = self.timestamps.take();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_liquid_scale_final_readback"),
            });
        self.world
            .copy_particle_validation_state(&mut encoder, &particle_readback)?;
        encoder.copy_buffer_to_buffer(
            &self.particle_step_status,
            0,
            &particle_readback,
            particle_layout.total_bytes,
            self.particle_step_status.size(),
        );
        self.world
            .copy_full_validation_state(&mut encoder, &body_readback);
        self.world
            .copy_construction_graph_evidence(&mut encoder, &graph_readback)?;
        if let Some(timestamps) = &timestamps {
            encoder.copy_buffer_to_buffer(
                &timestamps.resolve,
                0,
                &timestamps.readback,
                0,
                timestamps.total_bytes,
            );
        }
        self.queue.submit([encoder.finish()]);
        let timestamp_layout = timestamps.as_ref().map(TimestampResources::layout);
        Ok(PendingLiquidScaleReadback {
            particle: PendingMap::new(particle_readback),
            body: PendingMap::new(body_readback),
            graph: PendingMap::new(graph_readback),
            timestamps: timestamps.map(|timestamps| PendingMap::new(timestamps.readback)),
            particle_layout,
            timestamp_layout,
        })
    }

    pub fn poll(&self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Poll)
            .map(|_| ())
            .map_err(|error| format!("poll liquid benchmark: {error}"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_wait(&self, label: &str) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Wait)
            .map(|_| ())
            .map_err(|error| format!("wait for liquid benchmark {label}: {error}"))
    }

    pub fn complete(
        self,
        readback: LiquidScaleFinalReadback,
        cpu_seconds: Vec<f64>,
        host_seconds: Vec<f64>,
    ) -> Result<LiquidScalePhysicsBenchmarkRun, String> {
        if self.config.timing_mode != LiquidScaleTimingModeV1::PhysicsOnlyExplicitSteps {
            return Err("full-frame liquid benchmark requires pacing evidence".to_owned());
        }
        self.complete_with_pacing(readback, cpu_seconds, host_seconds, None)
    }

    pub fn complete_full_frame(
        self,
        readback: LiquidScaleFinalReadback,
        cpu_seconds: Vec<f64>,
        host_seconds: Vec<f64>,
        elapsed_seconds: f64,
    ) -> Result<LiquidScalePhysicsBenchmarkRun, String> {
        if self.config.timing_mode != LiquidScaleTimingModeV1::FullFrameInteractive {
            return Err("explicit-step liquid benchmark cannot carry pacing evidence".to_owned());
        }
        let requested_duration = self
            .config
            .interactive_duration_seconds
            .expect("validated full-frame liquid duration exists");
        if !elapsed_seconds.is_finite() || elapsed_seconds < requested_duration {
            return Err(format!(
                "full-frame liquid benchmark elapsed {elapsed_seconds:.6}s before its requested {requested_duration:.6}s duration"
            ));
        }
        let simulated_seconds = f64::from(self.config.retained_samples)
            * f64::from(self.config.outer_steps_per_sample)
            * self.config.outer_dt_seconds;
        let effective_simulation_scale = (simulated_seconds / elapsed_seconds).min(1.0);
        let slipped_wall_seconds = (elapsed_seconds - simulated_seconds).max(0.0);
        self.complete_with_pacing(
            readback,
            cpu_seconds,
            host_seconds,
            Some(PacingEvidence {
                effective_simulation_scale,
                slipped_wall_seconds,
            }),
        )
    }

    fn complete_with_pacing(
        self,
        readback: LiquidScaleFinalReadback,
        cpu_seconds: Vec<f64>,
        host_seconds: Vec<f64>,
        pacing: Option<PacingEvidence>,
    ) -> Result<LiquidScalePhysicsBenchmarkRun, String> {
        if cpu_seconds.len() != self.config.retained_samples as usize
            || host_seconds.len() != self.config.retained_samples as usize
        {
            return Err("liquid benchmark timing count mismatch".to_owned());
        }
        let (particle_bytes, step_bytes) = readback
            .particle_bytes
            .split_at(readback.particle_layout.total_bytes as usize);
        let particle_diagnostics = self
            .world
            .decode_particle_validation_state(particle_bytes)?;
        let (awake_bodies, bodies_finite, bodies_valid) = validate_body_state(
            &self.scenario.rigid.bodies,
            self.world.body_capacity(),
            &readback.body_bytes,
        )?;
        let graph = decode_construction_graph_evidence(&readback.graph_bytes)?;
        if graph.overflowed {
            return Err("liquid benchmark final rigid graph overflowed".to_owned());
        }
        let state_hash = exact_state_hash(
            readback.particle_layout,
            particle_bytes,
            &self.scenario.rigid.bodies,
            self.world.body_capacity(),
            &readback.body_bytes,
        )?;
        let timings = benchmark_timings(
            readback.timestamp_layout.as_ref(),
            readback.timestamp_bytes.as_deref(),
            &self.particle_stages,
            cpu_seconds,
            host_seconds,
            self.queue.get_timestamp_period(),
        )?;
        let extent = self.config.render_extent.unwrap_or([0, 0]);
        let mut capacities = self.world.benchmark_capacities(extent[0], extent[1]);
        if let Some(renderer) = &self.renderer {
            capacities.extend(renderer.capacity_records(extent[0], extent[1]));
        }
        capacities.sort_by(|left, right| left.class.cmp(&right.class));
        for capacity in &capacities {
            capacity.validate().map_err(|error| error.to_string())?;
        }

        let mut provenance = self.provenance;
        provenance.adapter = reported_or(&self.adapter.name, "not-reported");
        provenance.backend = reported_or(&self.adapter.backend, "not-reported");
        let reported_driver = format!("{} {}", self.adapter.driver, self.adapter.driver_info)
            .trim()
            .to_owned();
        provenance.driver = if reported_driver.is_empty() {
            "not-reported".to_owned()
        } else {
            reported_driver
        };
        let dispatches_per_outer_step = 1u32
            .checked_add(self.world.solver_dispatches_per_step())
            .and_then(|value| value.checked_add(self.world.particle_dispatches_per_step()))
            .and_then(|value| value.checked_add(self.world.body_lifecycle_dispatches_per_step()))
            .and_then(|value| {
                value.checked_add(self.world.post_fluid_stabilization_dispatches_per_step())
            })
            .ok_or_else(|| "liquid dispatch count overflowed".to_owned())?;
        let live_bodies = self
            .source
            .dynamic_solid_count
            .checked_add(self.source.fixed_solid_count)
            .ok_or_else(|| "liquid live-body count overflowed".to_owned())?;
        let sustained_60_hz = self.config.timing_mode
            == LiquidScaleTimingModeV1::FullFrameInteractive
            && self.config.interactive_duration_seconds.unwrap_or(0.0) >= 30.0
            && pacing.is_some_and(|evidence| evidence.effective_simulation_scale >= 0.95)
            && timings.host_completion.p95_seconds <= self.config.outer_dt_seconds;
        let artifact = LiquidScaleBenchmarkArtifactV1 {
            schema_version: SCHEMA_VERSION,
            artifact_version: LIQUID_SCALE_ARTIFACT_VERSION,
            scenario_id: self.source.id,
            source_definition_hash: self.source.scenario_hash,
            scenario_hash: self.scenario_hash,
            host_kind: self.host_kind,
            browser_version: self.browser_version,
            build_profile: "release".to_owned(),
            comparator_enabled: cfg!(feature = "box2d-comparator"),
            provenance,
            config: self.config.clone(),
            work: LiquidScaleWorkVectorV1 {
                version: LIQUID_SCALE_ARTIFACT_VERSION,
                dynamic_bodies: self.source.dynamic_solid_count,
                fixed_bodies: self.source.fixed_solid_count,
                live_bodies,
                awake_bodies,
                body_capacity: self.world.body_capacity(),
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
                completed_outer_steps: self.config.retained_samples,
                expected_final_active_particles: self.config.particle_count,
                final_active_particles: particle_diagnostics.active_count,
                final_state_finite: particle_diagnostics.state_finite && bodies_finite,
                generations_and_live_flags_valid: bodies_valid
                    && particle_diagnostics.active_count == particle_diagnostics.active_flags_count,
                capacity_clean: particle_diagnostics.capacity_clean,
                state_hash,
                effective_simulation_scale: pacing
                    .map(|evidence| evidence.effective_simulation_scale),
                slipped_wall_seconds: pacing.map(|evidence| evidence.slipped_wall_seconds),
                sustained_60_hz,
            },
        };
        let history: &[[u32; 2]] = bytemuck::try_cast_slice(step_bytes)
            .map_err(|error| format!("decode liquid benchmark step status: {error}"))?;
        let step_failure = first_particle_step_failure(history, self.config.particle_count);
        artifact.validate().map_err(|error| {
            format!(
                "{error}; work={:?}; finalActiveParticles={}; expected={}; finite={}; generationsAndLiveFlagsValid={}; capacityClean={}; overflowFlags={}; gridOverflow={}; commandDrops={}; escapedParticles={}; killedParticles={}; bodyContributionOverflow={}; firstParticleStepFailure={}",
                artifact.work,
                artifact.correctness.final_active_particles,
                artifact.correctness.expected_final_active_particles,
                artifact.correctness.final_state_finite,
                artifact.correctness.generations_and_live_flags_valid,
                artifact.correctness.capacity_clean,
                particle_diagnostics.overflow_flags,
                particle_diagnostics.grid_overflow,
                particle_diagnostics.command_drops,
                particle_diagnostics.escaped_particles,
                particle_diagnostics.killed_particles,
                particle_diagnostics.body_contribution_overflow,
                step_failure.as_deref().unwrap_or("none"),
            )
        })?;
        if let Some(error) = step_failure {
            return Err(error);
        }
        Ok(LiquidScalePhysicsBenchmarkRun {
            artifact,
            adapter: self.adapter,
        })
    }
}

fn first_particle_step_failure(history: &[[u32; 2]], expected: u32) -> Option<String> {
    let (index, [active, flags]) = history
        .iter()
        .enumerate()
        .find(|(_, status)| **status != [expected, 0])?;
    Some(format!(
        "outer step {} (including warmup): activeParticles={active}, expected={expected}, overflowFlags={flags}",
        index + 1
    ))
}

pub struct PendingLiquidScaleCompletion {
    mapped: PendingMap,
}

impl PendingLiquidScaleCompletion {
    pub fn is_ready(&self) -> bool {
        self.mapped.is_ready()
    }

    pub fn finish(self) -> Result<(), String> {
        let bytes = self.mapped.finish()?;
        if bytes.len() != 4 {
            return Err("liquid completion fence size mismatch".to_owned());
        }
        Ok(())
    }
}

pub struct PendingLiquidScaleReadback {
    particle: PendingMap,
    body: PendingMap,
    graph: PendingMap,
    timestamps: Option<PendingMap>,
    particle_layout: ParticleValidationLayout,
    timestamp_layout: Option<TimestampLayout>,
}

impl PendingLiquidScaleReadback {
    pub fn is_ready(&self) -> bool {
        self.particle.is_ready()
            && self.body.is_ready()
            && self.graph.is_ready()
            && self.timestamps.as_ref().is_none_or(PendingMap::is_ready)
    }

    pub fn finish(self) -> Result<LiquidScaleFinalReadback, String> {
        Ok(LiquidScaleFinalReadback {
            particle_bytes: self.particle.finish()?,
            body_bytes: self.body.finish()?,
            graph_bytes: self.graph.finish()?,
            timestamp_bytes: self.timestamps.map(PendingMap::finish).transpose()?,
            particle_layout: self.particle_layout,
            timestamp_layout: self.timestamp_layout,
        })
    }
}

pub struct LiquidScaleFinalReadback {
    particle_bytes: Vec<u8>,
    body_bytes: Vec<u8>,
    graph_bytes: Vec<u8>,
    timestamp_bytes: Option<Vec<u8>>,
    particle_layout: ParticleValidationLayout,
    timestamp_layout: Option<TimestampLayout>,
}

struct PendingMap {
    buffer: wgpu::Buffer,
    result: Arc<Mutex<Option<Result<(), String>>>>,
}

impl PendingMap {
    fn new(buffer: wgpu::Buffer) -> Self {
        let result = Arc::new(Mutex::new(None));
        let callback_result = Arc::clone(&result);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |mapped| {
                *callback_result
                    .lock()
                    .expect("liquid readback callback lock") =
                    Some(mapped.map_err(|error| error.to_string()));
            });
        Self { buffer, result }
    }

    fn is_ready(&self) -> bool {
        self.result
            .lock()
            .expect("liquid readback readiness lock")
            .is_some()
    }

    fn finish(self) -> Result<Vec<u8>, String> {
        self.result
            .lock()
            .map_err(|_| "liquid readback result lock poisoned".to_owned())?
            .take()
            .ok_or_else(|| "liquid readback is not ready".to_owned())??;
        let view = self.buffer.slice(..).get_mapped_range();
        let bytes = view.to_vec();
        drop(view);
        self.buffer.unmap();
        Ok(bytes)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_liquid_scale_physics_benchmark(
    config: LiquidScaleBenchmarkConfigV1,
    host_kind: LiquidScaleHostKindV1,
    provenance: ProvenanceV1,
) -> Result<LiquidScalePhysicsBenchmarkRun, String> {
    let mut session =
        LiquidScaleBenchmarkSession::new(config.clone(), host_kind, None, provenance).await?;
    session.submit_warmup()?;
    session.poll_wait("warmup")?;
    let mut cpu_seconds = Vec::with_capacity(config.retained_samples as usize);
    let mut host_seconds = Vec::with_capacity(config.retained_samples as usize);
    for sample in 0..config.retained_samples {
        let started = Instant::now();
        session.submit_sample()?;
        cpu_seconds.push(started.elapsed().as_secs_f64());
        session.poll_wait(&format!("sample {sample}"))?;
        host_seconds.push(started.elapsed().as_secs_f64());
    }
    let pending = session.begin_final_readback()?;
    session.poll_wait("final readback")?;
    session.complete(pending.finish()?, cpu_seconds, host_seconds)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_liquid_scale_full_frame_benchmark(
    config: LiquidScaleBenchmarkConfigV1,
    host_kind: LiquidScaleHostKindV1,
    provenance: ProvenanceV1,
) -> Result<LiquidScalePhysicsBenchmarkRun, String> {
    if config.timing_mode != LiquidScaleTimingModeV1::FullFrameInteractive {
        return Err("full-frame liquid runner requires full-frame timing mode".to_owned());
    }
    let requested_duration = config
        .interactive_duration_seconds
        .expect("validated full-frame liquid duration exists");
    let mut session =
        LiquidScaleBenchmarkSession::new(config.clone(), host_kind, None, provenance).await?;
    session.submit_warmup()?;
    session.poll_wait("warmup")?;

    let paced_started = Instant::now();
    let mut cpu_seconds = Vec::with_capacity(config.retained_samples as usize);
    let mut host_seconds = Vec::with_capacity(config.retained_samples as usize);
    for sample in 0..config.retained_samples {
        sleep_until(
            paced_started + Duration::from_secs_f64(f64::from(sample) * config.outer_dt_seconds),
        );
        let started = Instant::now();
        session.submit_sample()?;
        cpu_seconds.push(started.elapsed().as_secs_f64());
        session.poll_wait(&format!("full-frame sample {sample}"))?;
        host_seconds.push(started.elapsed().as_secs_f64());
    }
    sleep_until(paced_started + Duration::from_secs_f64(requested_duration));
    let elapsed_seconds = paced_started.elapsed().as_secs_f64();
    let pending = session.begin_final_readback()?;
    session.poll_wait("full-frame final readback")?;
    session.complete_full_frame(
        pending.finish()?,
        cpu_seconds,
        host_seconds,
        elapsed_seconds,
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn sleep_until(deadline: Instant) {
    if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        std::thread::sleep(remaining);
    }
}

struct TimestampResources {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    query_count: u32,
    sample_stride: u64,
    total_bytes: u64,
}

#[derive(Clone, Copy)]
struct TimestampLayout {
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

    fn layout(&self) -> TimestampLayout {
        TimestampLayout {
            query_count: self.query_count,
            sample_stride: self.sample_stride,
            total_bytes: self.total_bytes,
        }
    }
}

fn benchmark_timings(
    timestamps: Option<&TimestampLayout>,
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
            let rejected_delta = if rigid_end < frame_start {
                rigid_end as i128 - frame_start as i128
            } else {
                liquid_end as i128 - rigid_end as i128
            };
            return Ok(LiquidScaleTimingsV1 {
                percentile_method: LIQUID_SCALE_PERCENTILE_METHOD.to_owned(),
                gpu_timestamp_status: LiquidScaleGpuTimestampStatusV1::AdvertisedUnusableOrder,
                timestamp_quantization_seconds: Some(period_seconds),
                gpu_stages: Vec::new(),
                rejected_gpu_timestamp_raw_seconds: vec![rejected_delta as f64 * period_seconds],
                cpu_encode_submit,
                host_completion,
            });
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

fn liquid_render_transform(
    scenario: &physbox_contracts::CoupledScenarioV1,
    extent: [u32; 2],
) -> Result<ViewTransform, String> {
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
    let aspect = extent[0] as f32 / extent[1] as f32;
    let half_height =
        (0.5 * (upper_y - lower_y) + 1.0).max(0.5 * (upper_x - lower_x) / aspect + 1.0);
    let frame = crate::Camera2d::try_new_at_home(
        [0.5 * (lower_x + upper_x), 0.5 * (lower_y + upper_y)],
        half_height,
    )
    .map_err(|error| error.to_string())?
    .resolve(extent)
    .map_err(|error| error.to_string())?;
    Ok(ViewTransform {
        center: frame.center(),
        half_extents: frame.half_extents(),
    })
}

fn reported_or(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod particle_history_tests {
    use super::first_particle_step_failure;

    #[test]
    fn early_overflow_rejects_a_clean_endpoint() {
        assert!(first_particle_step_failure(&[[0, 0], [0, 0]], 0).is_none());
        let error = first_particle_step_failure(&[[8, 0], [8, 16], [8, 0]], 8).unwrap();
        assert!(error.contains("outer step 2"));
        assert!(error.contains("overflowFlags=16"));
        let error = first_particle_step_failure(&[[8, 0], [7, 0]], 8).unwrap();
        assert!(error.contains("activeParticles=7"));
    }
}
