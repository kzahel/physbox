use std::mem::size_of;
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use physbox_contracts::{
    BodyCommandBatchV1, BodyLayoutV1, CapacityRecordV1, SCHEMA_VERSION, Validate, canonical_hash,
};
use physbox_render::{
    BodyRenderTimestamp, PhysicsRenderer, RenderOptions, RenderTarget, ViewTransform,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::body_state::{BODY_COMMAND_CAPACITY, GpuBodyResetData};
use crate::body_world::{BodyCommandStepRange, GpuBodyWorld};
use crate::gpu_context::{
    AdapterDiagnostics, create_instance, request_adapter, request_device_with_features,
};
use crate::scenario::p5_1_free_flight_scenario;

const VALIDATION_TOLERANCE: f32 = 1.0e-5;
pub const BODY_BENCHMARK_WORK_VERSION: u32 = 2;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyBenchmarkWork {
    pub version: u32,
    pub bodies_total: u32,
    pub bodies_awake: u32,
    pub rectangles: u32,
    pub integration_workgroups: u32,
    /// Retained for machine-readable compatibility with work version 1.
    pub hold_workgroups: u32,
    pub grab_constraint_workgroups: u32,
    pub dispatches: u32,
    pub draw_calls: u32,
    pub rendered_body_instances: u32,
    pub rendered_vertices: u64,
    pub render_target_pixels: u64,
    pub exact_steps_per_sample: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyTimingSeries {
    pub sample_count: u32,
    pub zero_sample_count: u32,
    pub p50_seconds: f64,
    pub p95_seconds: f64,
    pub p99_seconds: f64,
    pub raw_seconds: Vec<f64>,
}

impl BodyTimingSeries {
    pub fn new(raw_seconds: Vec<f64>) -> Result<Self, String> {
        if raw_seconds.is_empty()
            || raw_seconds
                .iter()
                .any(|sample| !sample.is_finite() || *sample <= 0.0)
        {
            return Err(
                "body benchmark timings must be finite, positive, and non-empty".to_owned(),
            );
        }
        let mut sorted = raw_seconds.clone();
        sorted.sort_by(f64::total_cmp);
        Ok(Self {
            sample_count: raw_seconds.len() as u32,
            zero_sample_count: 0,
            p50_seconds: percentile(&sorted, 0.50),
            p95_seconds: percentile(&sorted, 0.95),
            p99_seconds: percentile(&sorted, 0.99),
            raw_seconds,
        })
    }

    fn new_gpu(raw_seconds: Vec<f64>) -> Result<Self, String> {
        if raw_seconds.is_empty()
            || raw_seconds
                .iter()
                .any(|sample| !sample.is_finite() || *sample < 0.0)
        {
            return Err(
                "body GPU benchmark timings must be finite, non-negative, and non-empty".to_owned(),
            );
        }
        let zero_sample_count = raw_seconds.iter().filter(|sample| **sample == 0.0).count() as u32;
        let mut sorted = raw_seconds.clone();
        sorted.sort_by(f64::total_cmp);
        Ok(Self {
            sample_count: raw_seconds.len() as u32,
            zero_sample_count,
            p50_seconds: percentile(&sorted, 0.50),
            p95_seconds: percentile(&sorted, 0.95),
            p99_seconds: percentile(&sorted, 0.99),
            raw_seconds,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuTimestampStatus {
    Unsupported,
    Valid,
    QuantizedZeros,
    AdvertisedUnusableZero,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyFinalValidation {
    pub validated_bodies: u32,
    pub finite: bool,
    pub generations_and_live_flags_valid: bool,
    pub max_abs_error: f32,
    pub tolerance: f32,
    pub state_hash: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuBodyBenchmarkOutput {
    pub schema_version: u32,
    pub adapter: AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub seed: u64,
    pub viewport: [u32; 2],
    pub warmup_steps: u32,
    pub samples: u32,
    pub work: BodyBenchmarkWork,
    pub body_layout: BodyLayoutV1,
    pub capacities: Vec<CapacityRecordV1>,
    pub product_allocated_bytes: u64,
    pub benchmark_scratch_bytes: u64,
    pub completion_method: &'static str,
    pub reset_outside_timing: bool,
    pub pipeline_creation_outside_timing: bool,
    pub final_validation_after_timing: bool,
    pub gpu_timestamp_status: GpuTimestampStatus,
    pub gpu_total: Option<BodyTimingSeries>,
    pub rejected_gpu_timestamp_raw_seconds: Vec<f64>,
    pub host_completion: BodyTimingSeries,
    pub cpu_encode_submit: BodyTimingSeries,
    pub timestamp_period_nanoseconds: Option<f32>,
    pub timestamp_quantization_seconds: Option<f64>,
    pub final_validation: BodyFinalValidation,
}

#[derive(Clone)]
struct ExpectedBody {
    pose: [f32; 4],
    motion: [f32; 4],
    metadata: [u32; 4],
}

struct TimestampResources {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    query_count: u32,
}

pub struct GpuBodyBenchmarkSession {
    device: wgpu::Device,
    queue: wgpu::Queue,
    world: GpuBodyWorld,
    renderer: PhysicsRenderer,
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    reset_data: GpuBodyResetData,
    expected: Vec<ExpectedBody>,
    empty_batch: BodyCommandBatchV1,
    ranges: [BodyCommandStepRange; 1],
    frame_index: u64,
    timestamps: Option<TimestampResources>,
    adapter: AdapterDiagnostics,
    source_scenario_hash: String,
    seed: u64,
    viewport: [u32; 2],
    warmup_steps: u32,
    samples: u32,
    body_layout: BodyLayoutV1,
    capacities: Vec<CapacityRecordV1>,
}

impl GpuBodyBenchmarkSession {
    pub async fn new(
        body_count: u32,
        seed: u64,
        viewport: [u32; 2],
        warmup_steps: u32,
        samples: u32,
    ) -> Result<Self, String> {
        if viewport[0] == 0 || viewport[1] == 0 || warmup_steps == 0 || samples == 0 {
            return Err(
                "body benchmark requires non-zero viewport, warmup, and samples".to_owned(),
            );
        }
        let scenario =
            p5_1_free_flight_scenario(body_count, 1, seed).map_err(|error| error.to_string())?;
        let source_scenario_hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;
        let instance = create_instance();
        let adapter = request_adapter(&instance, None).await?;
        let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let timestamps_available = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let features = if timestamps_available {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) = request_device_with_features(&adapter, features).await?;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("physbox5_body_benchmark_target"),
            size: wgpu::Extent3d {
                width: viewport[0],
                height: viewport[1],
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
        let world = GpuBodyWorld::new(&device, &scenario)?;
        let renderer = PhysicsRenderer::new(&device, format, world.render_view())
            .map_err(|error| error.to_string())?;
        let reset_data = world.benchmark_reset_data(&scenario.bodies)?;
        let expected = expected_one_step(&scenario);
        let query_count = samples
            .checked_mul(2)
            .ok_or_else(|| "body benchmark query count overflow".to_owned())?;
        let timestamps = timestamps_available.then(|| timestamp_resources(&device, query_count));
        let mut capacities = world.benchmark_capacities(viewport[0], viewport[1]);
        capacities.extend(renderer.capacity_records(viewport[0], viewport[1]));
        capacities.sort_by(|left, right| left.class.cmp(&right.class));
        for capacity in &capacities {
            capacity.validate().map_err(|error| error.to_string())?;
        }
        let body_layout = world.body_layout().clone();
        Ok(Self {
            device,
            queue,
            world,
            renderer,
            _texture: texture,
            view,
            reset_data,
            expected,
            empty_batch: BodyCommandBatchV1 {
                schema_version: SCHEMA_VERSION,
                capacity_commands: BODY_COMMAND_CAPACITY,
                attempted_commands: 0,
                overflowed: false,
                commands: Vec::new(),
            },
            ranges: [BodyCommandStepRange { start: 0, count: 0 }],
            frame_index: 0,
            timestamps,
            adapter: diagnostics,
            source_scenario_hash,
            seed,
            viewport,
            warmup_steps,
            samples,
            body_layout,
            capacities,
        })
    }

    pub fn reset(&self) {
        self.world
            .reset_from_authored(&self.queue, &self.reset_data);
        self.queue.submit(std::iter::empty());
    }

    pub fn submit_step(&mut self, sample_index: Option<u32>) -> Result<(), String> {
        if sample_index.is_some_and(|sample| sample >= self.samples) {
            return Err("body benchmark sample index exceeds query capacity".to_owned());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_body_benchmark_sample"),
            });
        let beginning = sample_index.and_then(|sample| {
            self.timestamps
                .as_ref()
                .map(|timestamps| (&timestamps.query_set, sample * 2))
        });
        let ending = sample_index.and_then(|sample| {
            self.timestamps
                .as_ref()
                .map(|timestamps| (&timestamps.query_set, sample * 2 + 1))
        });
        self.world.encode_steps_with_timestamp(
            &self.queue,
            &mut encoder,
            1,
            &self.empty_batch,
            &self.ranges,
            beginning,
        )?;
        let camera = crate::Camera2d::fixed()
            .resolve(self.viewport)
            .expect("benchmark target resolves the fixed camera");
        self.renderer
            .encode(
                &self.queue,
                &mut encoder,
                RenderTarget {
                    color_view: &self.view,
                    size: self.viewport,
                },
                RenderOptions {
                    frame_index: self.frame_index,
                    view: ViewTransform {
                        center: camera.center(),
                        half_extents: camera.half_extents(),
                    },
                    body_timestamp: ending.map(|(query_set, query_index)| BodyRenderTimestamp {
                        query_set,
                        query_index,
                    }),
                    ..RenderOptions::default()
                },
            )
            .map_err(|error| error.to_string())?;
        self.queue.submit([encoder.finish()]);
        self.frame_index += 1;
        Ok(())
    }

    pub fn begin_completion_fence(&self) -> PendingCompletion {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_benchmark_completion_fence"),
            size: 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_body_benchmark_completion_encoder"),
            });
        encoder.clear_buffer(&buffer, 0, None);
        self.queue.submit([encoder.finish()]);
        PendingCompletion {
            mapped: PendingMap::new(buffer),
        }
    }

    pub fn poll(&self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Poll)
            .map(|_| ())
            .map_err(|error| format!("poll body benchmark: {error}"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_wait(&self, label: &str) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Wait)
            .map(|_| ())
            .map_err(|error| format!("poll body benchmark {label}: {error}"))
    }

    pub fn begin_final_validation(&mut self) -> PendingBodyValidation {
        let byte_count = u64::from(self.world.initial_body_count()) * 48;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_benchmark_validation"),
            size: byte_count,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_body_benchmark_validation_copy"),
            });
        self.world
            .copy_full_validation_state(&mut encoder, &readback);
        self.queue.submit([encoder.finish()]);
        PendingBodyValidation {
            mapped: PendingMap::new(readback),
            expected: std::mem::take(&mut self.expected),
        }
    }

    pub fn begin_timestamp_readback(&mut self) -> Option<PendingTimestampReadback> {
        let timestamps = self.timestamps.take()?;
        let bytes = u64::from(timestamps.query_count) * size_of::<u64>() as u64;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_body_benchmark_timestamp_copy"),
            });
        encoder.resolve_query_set(
            &timestamps.query_set,
            0..timestamps.query_count,
            &timestamps.resolve,
            0,
        );
        encoder.copy_buffer_to_buffer(&timestamps.resolve, 0, &timestamps.readback, 0, bytes);
        self.queue.submit([encoder.finish()]);
        Some(PendingTimestampReadback {
            mapped: PendingMap::new(timestamps.readback),
            query_count: timestamps.query_count,
            period_nanoseconds: self.queue.get_timestamp_period(),
        })
    }

    pub fn complete(
        self,
        gpu_seconds: Option<Vec<f64>>,
        host_completion_seconds: Vec<f64>,
        cpu_encode_submit_seconds: Vec<f64>,
        final_validation: BodyFinalValidation,
    ) -> Result<GpuBodyBenchmarkOutput, String> {
        if host_completion_seconds.len() != self.samples as usize
            || cpu_encode_submit_seconds.len() != self.samples as usize
            || gpu_seconds
                .as_ref()
                .is_some_and(|samples| samples.len() != self.samples as usize)
        {
            return Err("body benchmark timing count mismatch".to_owned());
        }
        let timestamp_period_nanoseconds = gpu_seconds
            .as_ref()
            .map(|_| self.queue.get_timestamp_period());
        let gpu_timestamp_status = classify_gpu_timestamps(gpu_seconds.as_deref());
        let product_allocated_bytes = self
            .capacities
            .iter()
            .map(|capacity| capacity.allocated_bytes)
            .sum();
        let query_bytes = if gpu_seconds.is_some() {
            u64::from(self.samples) * 2 * 8 * 2
        } else {
            0
        };
        let validation_bytes = u64::from(self.body_layout.capacity_elements) * 48;
        let workgroups = self.body_layout.capacity_elements.div_ceil(64);
        let rejected_gpu_timestamp_raw_seconds = if matches!(
            gpu_timestamp_status,
            GpuTimestampStatus::AdvertisedUnusableZero
        ) {
            gpu_seconds.clone().unwrap_or_default()
        } else {
            Vec::new()
        };
        let gpu_total = if matches!(
            gpu_timestamp_status,
            GpuTimestampStatus::Valid | GpuTimestampStatus::QuantizedZeros
        ) {
            gpu_seconds.map(BodyTimingSeries::new_gpu).transpose()?
        } else {
            None
        };
        Ok(GpuBodyBenchmarkOutput {
            schema_version: SCHEMA_VERSION,
            adapter: self.adapter,
            source_scenario_hash: self.source_scenario_hash,
            seed: self.seed,
            viewport: self.viewport,
            warmup_steps: self.warmup_steps,
            samples: self.samples,
            work: BodyBenchmarkWork {
                version: BODY_BENCHMARK_WORK_VERSION,
                bodies_total: self.body_layout.live_elements,
                bodies_awake: self.body_layout.live_elements,
                rectangles: self.body_layout.live_elements,
                integration_workgroups: workgroups,
                hold_workgroups: 0,
                grab_constraint_workgroups: 1,
                dispatches: 2,
                draw_calls: 2,
                rendered_body_instances: self.body_layout.live_elements,
                rendered_vertices: 3 + u64::from(self.body_layout.live_elements) * 6,
                render_target_pixels: u64::from(self.viewport[0]) * u64::from(self.viewport[1]),
                exact_steps_per_sample: 1,
            },
            body_layout: self.body_layout,
            capacities: self.capacities,
            product_allocated_bytes,
            benchmark_scratch_bytes: query_bytes + validation_bytes,
            completion_method: if cfg!(target_arch = "wasm32") {
                "mapped-buffer-fence"
            } else {
                "device-poll-wait"
            },
            reset_outside_timing: true,
            pipeline_creation_outside_timing: true,
            final_validation_after_timing: true,
            gpu_timestamp_status,
            gpu_total,
            rejected_gpu_timestamp_raw_seconds,
            host_completion: BodyTimingSeries::new(host_completion_seconds)?,
            cpu_encode_submit: BodyTimingSeries::new(cpu_encode_submit_seconds)?,
            timestamp_period_nanoseconds,
            timestamp_quantization_seconds: if cfg!(target_arch = "wasm32")
                && timestamp_period_nanoseconds.is_some()
            {
                Some(1.0e-4)
            } else {
                None
            },
            final_validation,
        })
    }
}

pub struct PendingBodyValidation {
    mapped: PendingMap,
    expected: Vec<ExpectedBody>,
}

pub struct PendingCompletion {
    mapped: PendingMap,
}

impl PendingCompletion {
    pub fn is_ready(&self) -> bool {
        self.mapped.is_ready()
    }

    pub fn finish(self) -> Result<(), String> {
        let bytes = self.mapped.finish()?;
        if bytes.len() != 4 {
            return Err("body completion fence size mismatch".to_owned());
        }
        Ok(())
    }
}

impl PendingBodyValidation {
    pub fn is_ready(&self) -> bool {
        self.mapped.is_ready()
    }

    pub fn finish(self) -> Result<BodyFinalValidation, String> {
        let bytes = self.mapped.finish()?;
        let body_count = self.expected.len();
        let lane_bytes = body_count * 16;
        if bytes.len() != lane_bytes * 3 {
            return Err("body validation readback size mismatch".to_owned());
        }
        let mut max_abs_error = 0.0_f32;
        for (index, expected) in self.expected.iter().enumerate() {
            let pose = read_f32x4(&bytes, index * 16);
            let motion = read_f32x4(&bytes, lane_bytes + index * 16);
            let metadata = read_u32x4(&bytes, lane_bytes * 2 + index * 16);
            if pose.iter().chain(&motion).any(|value| !value.is_finite()) {
                return Err(format!("body {index} final state is non-finite"));
            }
            for (actual, expected) in pose.iter().zip(expected.pose) {
                max_abs_error = max_abs_error.max((actual - expected).abs());
            }
            for (actual, expected) in motion.iter().zip(expected.motion) {
                max_abs_error = max_abs_error.max((actual - expected).abs());
            }
            if metadata != expected.metadata {
                return Err(format!("body {index} final metadata changed"));
            }
        }
        if max_abs_error > VALIDATION_TOLERANCE {
            return Err(format!(
                "body benchmark final-state error {max_abs_error} exceeds {VALIDATION_TOLERANCE}"
            ));
        }
        let state_hash = hex_digest(Sha256::digest(&bytes));
        Ok(BodyFinalValidation {
            validated_bodies: body_count as u32,
            finite: true,
            generations_and_live_flags_valid: true,
            max_abs_error,
            tolerance: VALIDATION_TOLERANCE,
            state_hash,
        })
    }
}

pub struct PendingTimestampReadback {
    mapped: PendingMap,
    query_count: u32,
    period_nanoseconds: f32,
}

impl PendingTimestampReadback {
    pub fn is_ready(&self) -> bool {
        self.mapped.is_ready()
    }

    pub fn finish(self) -> Result<Vec<f64>, String> {
        let bytes = self.mapped.finish()?;
        if bytes.len() != self.query_count as usize * size_of::<u64>() {
            return Err("body timestamp readback size mismatch".to_owned());
        }
        let values = bytes
            .chunks_exact(8)
            .map(|chunk| u64::from_ne_bytes(chunk.try_into().expect("u64 timestamp chunk")))
            .collect::<Vec<_>>();
        if let Some((sample, pair)) = values
            .chunks_exact(2)
            .enumerate()
            .find(|(_, pair)| pair[1] < pair[0])
        {
            return Err(format!(
                "body GPU timestamp sample {sample} ran backwards: begin={}, end={}, periodNanoseconds={}",
                pair[0], pair[1], self.period_nanoseconds
            ));
        }
        Ok(values
            .chunks_exact(2)
            .map(|pair| (pair[1] - pair[0]) as f64 * f64::from(self.period_nanoseconds) * 1.0e-9)
            .collect())
    }
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
                *callback_result.lock().expect("body readback callback lock") =
                    Some(mapped.map_err(|error| error.to_string()));
            });
        Self { buffer, result }
    }

    fn is_ready(&self) -> bool {
        self.result
            .lock()
            .expect("body readback readiness lock")
            .is_some()
    }

    fn finish(self) -> Result<Vec<u8>, String> {
        self.result
            .lock()
            .map_err(|_| "body readback result lock poisoned".to_owned())?
            .take()
            .ok_or_else(|| "body readback is not ready".to_owned())??;
        let view = self.buffer.slice(..).get_mapped_range();
        let bytes = view.to_vec();
        drop(view);
        self.buffer.unmap();
        Ok(bytes)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_body_free_flight(
    body_count: u32,
    seed: u64,
    viewport: [u32; 2],
    warmup_steps: u32,
    samples: u32,
) -> Result<GpuBodyBenchmarkOutput, String> {
    let mut session =
        GpuBodyBenchmarkSession::new(body_count, seed, viewport, warmup_steps, samples).await?;
    for _ in 0..warmup_steps {
        session.reset();
        session.poll_wait("warmup reset")?;
        session.submit_step(None)?;
        session.poll_wait("warmup")?;
    }
    let mut host_completion_seconds = Vec::with_capacity(samples as usize);
    let mut cpu_encode_submit_seconds = Vec::with_capacity(samples as usize);
    for sample in 0..samples {
        session.reset();
        session.poll_wait("sample reset")?;
        let started = Instant::now();
        session.submit_step(Some(sample))?;
        cpu_encode_submit_seconds.push(started.elapsed().as_secs_f64());
        session.poll_wait("sample")?;
        host_completion_seconds.push(started.elapsed().as_secs_f64());
    }
    let validation = session.begin_final_validation();
    session.poll_wait("final validation")?;
    let validation = validation.finish()?;
    let gpu_seconds = if let Some(timestamps) = session.begin_timestamp_readback() {
        session.poll_wait("timestamp readback")?;
        Some(timestamps.finish()?)
    } else {
        None
    };
    session.complete(
        gpu_seconds,
        host_completion_seconds,
        cpu_encode_submit_seconds,
        validation,
    )
}

fn timestamp_resources(device: &wgpu::Device, query_count: u32) -> TimestampResources {
    let bytes = u64::from(query_count) * size_of::<u64>() as u64;
    TimestampResources {
        query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("physbox5_body_benchmark_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: query_count,
        }),
        resolve: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_benchmark_timestamp_resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        readback: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_benchmark_timestamp_readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }),
        query_count,
    }
}

fn expected_one_step(scenario: &physbox_contracts::ScenarioV1) -> Vec<ExpectedBody> {
    let dt = scenario.settings.fixed_dt.get();
    scenario
        .bodies
        .iter()
        .map(|body| {
            let mut velocity = body.linear_velocity;
            velocity.x += scenario.settings.gravity.x * dt;
            velocity.y += scenario.settings.gravity.y * dt;
            let mut flags = 1_u32 | (1_u32 << 8);
            if body.sleeping {
                flags |= 1_u32 << 1;
            }
            if body.inverse_mass.get() == 0.0 {
                flags |= 1_u32 << 2;
            }
            ExpectedBody {
                pose: [
                    body.position.x + velocity.x * dt,
                    body.position.y + velocity.y * dt,
                    body.angle.get() + body.angular_velocity.get() * dt,
                    0.0,
                ],
                motion: [velocity.x, velocity.y, body.angular_velocity.get(), 0.0],
                metadata: [
                    body.id.generation,
                    flags,
                    body.material.friction.get().to_bits(),
                    body.material.restitution.get().to_bits(),
                ],
            }
        })
        .collect()
}

fn read_f32x4(bytes: &[u8], offset: usize) -> [f32; 4] {
    std::array::from_fn(|index| {
        f32::from_ne_bytes(
            bytes[offset + index * 4..offset + index * 4 + 4]
                .try_into()
                .expect("f32 body-state chunk"),
        )
    })
}

fn read_u32x4(bytes: &[u8], offset: usize) -> [u32; 4] {
    std::array::from_fn(|index| {
        u32::from_ne_bytes(
            bytes[offset + index * 4..offset + index * 4 + 4]
                .try_into()
                .expect("u32 body-state chunk"),
        )
    })
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    let rank = (percentile * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank - 1]
}

fn classify_gpu_timestamps(samples: Option<&[f64]>) -> GpuTimestampStatus {
    match samples {
        None => GpuTimestampStatus::Unsupported,
        Some(samples) if samples.iter().all(|sample| *sample == 0.0) => {
            GpuTimestampStatus::AdvertisedUnusableZero
        }
        Some(samples) if samples.contains(&0.0) => GpuTimestampStatus::QuantizedZeros,
        Some(_) => GpuTimestampStatus::Valid,
    }
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_summary_retains_raw_nearest_rank_samples() {
        let timing = BodyTimingSeries::new(vec![0.004, 0.001, 0.003, 0.002]).unwrap();
        assert_eq!(timing.p50_seconds, 0.002);
        assert_eq!(timing.p95_seconds, 0.004);
        assert_eq!(timing.raw_seconds, vec![0.004, 0.001, 0.003, 0.002]);
    }

    #[test]
    fn gpu_timing_classifies_quantized_and_unusable_zero_samples() {
        assert_eq!(
            classify_gpu_timestamps(Some(&[0.0, 0.0])),
            GpuTimestampStatus::AdvertisedUnusableZero
        );
        assert_eq!(
            classify_gpu_timestamps(Some(&[0.0, 0.001])),
            GpuTimestampStatus::QuantizedZeros
        );
        let timing = BodyTimingSeries::new_gpu(vec![0.0, 0.001]).unwrap();
        assert_eq!(timing.zero_sample_count, 1);
        assert_eq!(timing.p50_seconds, 0.0);
    }

    #[test]
    fn body_pipeline_performance_smoke_validates_state_and_reports_timing() {
        let result =
            pollster::block_on(benchmark_gpu_body_free_flight(32, 0x5eed, [320, 200], 1, 3))
                .unwrap();
        assert_eq!(result.work.bodies_total, 32);
        assert_eq!(result.work.version, BODY_BENCHMARK_WORK_VERSION);
        assert_eq!(result.work.hold_workgroups, 0);
        assert_eq!(result.work.grab_constraint_workgroups, 1);
        assert_eq!(result.work.dispatches, 2);
        assert_eq!(result.work.draw_calls, 2);
        assert_eq!(result.work.rendered_vertices, 3 + 32 * 6);
        assert_eq!(result.final_validation.validated_bodies, 32);
        assert!(result.final_validation.finite);
        assert!(result.final_validation.max_abs_error <= VALIDATION_TOLERANCE);
        assert_eq!(result.body_layout.bytes_per_body, 64);
        assert!(
            result
                .capacities
                .iter()
                .all(|capacity| !capacity.overflowed)
        );
        println!(
            "PHYSBOX5_PERF_SMOKE scope=body-free-flight bodies=32 host_p50_ms={:.3} host_p95_ms={:.3} cpu_p50_ms={:.3} gpu_status={:?} replay=validated capacity=clean",
            result.host_completion.p50_seconds * 1_000.0,
            result.host_completion.p95_seconds * 1_000.0,
            result.cpu_encode_submit.p50_seconds * 1_000.0,
            result.gpu_timestamp_status,
        );
    }
}
