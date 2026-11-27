#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod demos;
#[cfg(target_arch = "wasm32")]
mod water_surface;

#[cfg(target_arch = "wasm32")]
use std::mem::size_of;
#[cfg(target_arch = "wasm32")]
use std::sync::{Arc, Mutex};

#[cfg(target_arch = "wasm32")]
use bytemuck::{Pod, Zeroable};
#[cfg(all(test, not(target_arch = "wasm32")))]
use physbox_sim::WorldCapacity;
#[cfg(target_arch = "wasm32")]
use physbox_sim::{
    BodyDef, CommandResultRequest, GpuBodyObservation, GpuParticleObservation,
    InteractiveClockAction, InteractiveClockInput, ParticleCountSnapshot,
    ParticleCountSnapshotRequest, ParticleEmissionDef, ParticleRegionDef, ParticleSystemDef,
    PointConstraintDef, PointConstraintId, PointQueryDef, ShapeDef, ShapeGeometry, SimulationClock,
    StepDef, World, WorldCapacity, WorldCommandKind, WorldCommandStatus, WorldDef,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;
#[cfg(target_arch = "wasm32")]
use wgpu::util::DeviceExt;

#[cfg(any(target_arch = "wasm32", test))]
const PLAYGROUND: &str = "playground";
#[cfg(any(target_arch = "wasm32", test))]
const STACK_AND_SLEEP: &str = "stack-and-sleep";
#[cfg(any(target_arch = "wasm32", test))]
const WATER_AND_RIGID: &str = "water-and-rigid";
#[cfg(any(target_arch = "wasm32", test))]
const RIGID_STRESS_1K: &str = "rigid-stress-1k";
#[cfg(any(target_arch = "wasm32", test))]
const RIGID_STRESS_2K: &str = "rigid-stress-2k";
#[cfg(any(target_arch = "wasm32", test))]
const RIGID_STRESS_5K: &str = "rigid-stress-5k";
#[cfg(any(target_arch = "wasm32", test))]
const RIGID_STRESS_10K: &str = "rigid-stress-10k";
#[cfg(any(target_arch = "wasm32", test))]
const WATER_STRESS_4K: &str = "water-stress-4k";
#[cfg(any(target_arch = "wasm32", test))]
const WATER_STRESS_8K: &str = "water-stress-8k";
#[cfg(any(target_arch = "wasm32", test))]
const WATER_STRESS_16K: &str = "water-stress-16k";
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SceneName {
    Playground,
    StackAndSleep,
    WaterAndRigid,
    DominoRun,
    MarblePegboard,
    WaterSurge,
    SeesawLaunch,
    Breakwater,
    BounceTrio,
    PullABrick,
    TowerDrop13k,
    TowerDrop32k,
    RigidStress1k,
    RigidStress2k,
    RigidStress5k,
    RigidStress10k,
    RigidRain1k,
    RigidAvalanche2k,
    WaterStress4k,
    WaterStress8k,
    WaterStress16k,
}

#[cfg(any(target_arch = "wasm32", test))]
impl SceneName {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            PLAYGROUND => Ok(Self::Playground),
            STACK_AND_SLEEP => Ok(Self::StackAndSleep),
            "tower-drop-13k" => Ok(Self::TowerDrop13k),
            "tower-drop-32k" => Ok(Self::TowerDrop32k),
            WATER_AND_RIGID => Ok(Self::WaterAndRigid),
            "domino-run" => Ok(Self::DominoRun),
            "marble-pegboard" => Ok(Self::MarblePegboard),
            "water-surge" => Ok(Self::WaterSurge),
            "seesaw-launch" => Ok(Self::SeesawLaunch),
            "breakwater" => Ok(Self::Breakwater),
            "bounce-trio" => Ok(Self::BounceTrio),
            "pull-a-brick" => Ok(Self::PullABrick),
            RIGID_STRESS_1K => Ok(Self::RigidStress1k),
            RIGID_STRESS_2K => Ok(Self::RigidStress2k),
            RIGID_STRESS_5K => Ok(Self::RigidStress5k),
            RIGID_STRESS_10K => Ok(Self::RigidStress10k),
            "rigid-rain-1k" => Ok(Self::RigidRain1k),
            "rigid-avalanche-2k" => Ok(Self::RigidAvalanche2k),
            WATER_STRESS_4K => Ok(Self::WaterStress4k),
            WATER_STRESS_8K => Ok(Self::WaterStress8k),
            WATER_STRESS_16K => Ok(Self::WaterStress16k),
            _ => Err(format!("unknown showcase scene {value:?}")),
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::Playground => PLAYGROUND,
            Self::StackAndSleep => STACK_AND_SLEEP,
            Self::TowerDrop13k => "tower-drop-13k",
            Self::TowerDrop32k => "tower-drop-32k",
            Self::WaterAndRigid => WATER_AND_RIGID,
            Self::DominoRun => "domino-run",
            Self::MarblePegboard => "marble-pegboard",
            Self::WaterSurge => "water-surge",
            Self::SeesawLaunch => "seesaw-launch",
            Self::Breakwater => "breakwater",
            Self::BounceTrio => "bounce-trio",
            Self::PullABrick => "pull-a-brick",
            Self::RigidStress1k => RIGID_STRESS_1K,
            Self::RigidStress2k => RIGID_STRESS_2K,
            Self::RigidStress5k => RIGID_STRESS_5K,
            Self::RigidStress10k => RIGID_STRESS_10K,
            Self::RigidRain1k => "rigid-rain-1k",
            Self::RigidAvalanche2k => "rigid-avalanche-2k",
            Self::WaterStress4k => WATER_STRESS_4K,
            Self::WaterStress8k => WATER_STRESS_8K,
            Self::WaterStress16k => WATER_STRESS_16K,
        }
    }

    const fn tower_drop(self) -> bool {
        matches!(self, Self::TowerDrop13k | Self::TowerDrop32k)
    }

    const fn authors_water(self) -> bool {
        matches!(
            self,
            Self::TowerDrop13k
                | Self::TowerDrop32k
                | Self::WaterAndRigid
                | Self::WaterSurge
                | Self::Breakwater
                | Self::WaterStress4k
                | Self::WaterStress8k
                | Self::WaterStress16k
        )
    }

    const fn rigid_stress_body_count(self) -> Option<u32> {
        match self {
            Self::RigidStress1k => Some(1_000),
            Self::RigidStress2k => Some(2_000),
            Self::RigidStress5k => Some(5_000),
            Self::RigidStress10k => Some(10_000),
            _ => None,
        }
    }

    const fn water_stress_capacity(self) -> Option<u32> {
        match self {
            Self::WaterStress4k => Some(4_096),
            Self::WaterStress8k => Some(8_192),
            Self::WaterStress16k => Some(16_384),
            _ => None,
        }
    }

    #[cfg(target_arch = "wasm32")]
    const fn rigid_drop_body_count(self) -> Option<u32> {
        match self {
            Self::RigidRain1k => Some(1_000),
            Self::RigidAvalanche2k => Some(2_000),
            _ => None,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShowcaseConfig {
    body_capacity: WorldCapacity,
    particle_capacity: Option<u32>,
    physics_hz: u32,
}

#[cfg(any(target_arch = "wasm32", test))]
impl ShowcaseConfig {
    const DEFAULT: Self = Self {
        body_capacity: WorldCapacity::Slots256,
        particle_capacity: Some(4_096),
        physics_hz: 60,
    };

    const fn for_scene(name: SceneName) -> Self {
        let body_capacity = match name {
            SceneName::TowerDrop13k
            | SceneName::TowerDrop32k
            | SceneName::DominoRun
            | SceneName::WaterSurge
            | SceneName::SeesawLaunch
            | SceneName::Breakwater
            | SceneName::BounceTrio
            | SceneName::PullABrick => WorldCapacity::Slots64,
            SceneName::RigidStress1k | SceneName::RigidRain1k => WorldCapacity::Slots1000,
            SceneName::RigidStress2k | SceneName::RigidAvalanche2k => WorldCapacity::Slots2000,
            SceneName::RigidStress5k => WorldCapacity::Slots5000,
            SceneName::RigidStress10k => WorldCapacity::Slots10000,
            _ => Self::DEFAULT.body_capacity,
        };
        Self {
            body_capacity,
            particle_capacity: if matches!(name, SceneName::TowerDrop13k) {
                Some(16_384)
            } else if matches!(name, SceneName::TowerDrop32k) {
                Some(32_768)
            } else if let Some(capacity) = name.water_stress_capacity() {
                Some(capacity)
            } else if name.authors_water() {
                Some(4_096)
            } else {
                None
            },
            physics_hz: Self::DEFAULT.physics_hz,
        }
    }

    fn parse(body_capacity: u32, particle_capacity: u32, physics_hz: u32) -> Result<Self, String> {
        let body_capacity = match body_capacity {
            64 => WorldCapacity::Slots64,
            256 => WorldCapacity::Slots256,
            1_000 => WorldCapacity::Slots1000,
            2_000 => WorldCapacity::Slots2000,
            5_000 => WorldCapacity::Slots5000,
            10_000 => WorldCapacity::Slots10000,
            _ => {
                return Err(format!(
                    "showcase body capacity must be 64, 256, 1000, 2000, 5000, or 10000; got {body_capacity}"
                ));
            }
        };
        let particle_capacity = match particle_capacity {
            0 => None,
            4_096 | 8_192 | 16_384 | 32_768 => Some(particle_capacity),
            _ => {
                return Err(format!(
                    "showcase water capacity must be None, 4096, 8192, 16384, or 32768; got {particle_capacity}"
                ));
            }
        };
        if !matches!(physics_hz, 30 | 60) {
            return Err(format!(
                "showcase physics rate must be 30 or 60 Hz; got {physics_hz}"
            ));
        }
        Ok(Self {
            body_capacity,
            particle_capacity,
            physics_hz,
        })
    }
}

#[cfg(target_arch = "wasm32")]
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShapeVisual {
    /// Rectangle half-extents in `xy`, circle radius repeated in `zw`.
    geometry: [f32; 4],
    color: [f32; 4],
    /// `[body generation, shape kind (1 circle, 2 rectangle), slot, reserved]`.
    identity: [u32; 4],
}

#[cfg(target_arch = "wasm32")]
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RenderParams {
    /// `[center_x, center_y, half_width, half_height]`.
    camera: [f32; 4],
    /// `[pixel_width, pixel_height, reserved, reserved]`.
    viewport: [f32; 4],
    /// `[particle render radius, reserved, reserved, reserved]`.
    material: [f32; 4],
}

#[cfg(target_arch = "wasm32")]
const SHOWCASE_TIMESTAMP_COUNT: u32 = 3;
#[cfg(target_arch = "wasm32")]
const SHOWCASE_TIMESTAMP_SLOT_COUNT: u32 = 3;
#[cfg(target_arch = "wasm32")]
const SHOWCASE_TIMESTAMP_BYTES: u64 = SHOWCASE_TIMESTAMP_COUNT as u64 * size_of::<u64>() as u64;
#[cfg(target_arch = "wasm32")]
const SHOWCASE_TIMESTAMP_SAMPLE_INTERVAL_STEPS: u64 = 30;

#[cfg(target_arch = "wasm32")]
struct ShowcaseTimestampSlot {
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    completion: Arc<Mutex<Option<Result<(), String>>>>,
    pending_sample: Option<(u64, u64)>,
}

#[cfg(target_arch = "wasm32")]
struct ShowcaseGpuTiming {
    query_set: Option<wgpu::QuerySet>,
    slots: Vec<ShowcaseTimestampSlot>,
    timestamp_period_nanoseconds: f64,
    epoch: u64,
    latest: Option<ShowcaseGpuTimingSample>,
    error: Option<String>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
struct ShowcaseGpuTimingSample {
    epoch: u64,
    outer_step: u64,
    physics_ms: f64,
    render_ms: f64,
    frame_ms: f64,
}

#[cfg(target_arch = "wasm32")]
impl ShowcaseGpuTiming {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, supported: bool) -> Self {
        let query_set = supported.then(|| {
            device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("physbox_showcase_timestamp_queries"),
                ty: wgpu::QueryType::Timestamp,
                count: SHOWCASE_TIMESTAMP_COUNT * SHOWCASE_TIMESTAMP_SLOT_COUNT,
            })
        });
        let slots = if supported {
            (0..SHOWCASE_TIMESTAMP_SLOT_COUNT)
                .map(|_| ShowcaseTimestampSlot {
                    resolve: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("physbox_showcase_timestamp_resolve"),
                        size: SHOWCASE_TIMESTAMP_BYTES,
                        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    }),
                    readback: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("physbox_showcase_timestamp_readback"),
                        size: SHOWCASE_TIMESTAMP_BYTES,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    completion: Arc::new(Mutex::new(None)),
                    pending_sample: None,
                })
                .collect()
        } else {
            Vec::new()
        };
        Self {
            query_set,
            slots,
            timestamp_period_nanoseconds: f64::from(queue.get_timestamp_period()),
            epoch: 0,
            latest: None,
            error: None,
        }
    }

    fn supported(&self) -> bool {
        self.query_set.is_some()
    }

    fn begin_scene(&mut self) {
        self.epoch = self.epoch.saturating_add(1);
        self.latest = None;
    }

    fn latest(&self) -> Option<ShowcaseGpuTimingSample> {
        self.latest.filter(|sample| sample.epoch == self.epoch)
    }

    fn candidate_slot(&self, next_outer_step: u64) -> Option<usize> {
        if self.error.is_some()
            || !next_outer_step.is_multiple_of(SHOWCASE_TIMESTAMP_SAMPLE_INTERVAL_STEPS)
        {
            return None;
        }
        self.query_set.as_ref()?;
        self.slots
            .iter()
            .position(|slot| slot.pending_sample.is_none())
    }

    fn write_marker(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        slot: usize,
        offset: u32,
        label: &'static str,
    ) {
        let query_set = self
            .query_set
            .as_ref()
            .expect("timing marker requires the supported query set");
        let _pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(
                    slot as u32 * SHOWCASE_TIMESTAMP_COUNT + offset,
                ),
                end_of_pass_write_index: None,
            }),
        });
    }

    fn encode_resolve(&mut self, encoder: &mut wgpu::CommandEncoder, slot: usize, outer_step: u64) {
        let query_set = self
            .query_set
            .as_ref()
            .expect("timing resolve requires the supported query set");
        let query_base = slot as u32 * SHOWCASE_TIMESTAMP_COUNT;
        let resources = &mut self.slots[slot];
        encoder.resolve_query_set(
            query_set,
            query_base..query_base + SHOWCASE_TIMESTAMP_COUNT,
            &resources.resolve,
            0,
        );
        encoder.copy_buffer_to_buffer(
            &resources.resolve,
            0,
            &resources.readback,
            0,
            SHOWCASE_TIMESTAMP_BYTES,
        );
        resources.pending_sample = Some((self.epoch, outer_step));
    }

    fn start_mapping(&mut self, slot: usize) {
        let resources = &mut self.slots[slot];
        let callback = Arc::clone(&resources.completion);
        resources
            .readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                *callback
                    .lock()
                    .expect("showcase timestamp completion lock is not poisoned") =
                    Some(result.map_err(|error| error.to_string()));
            });
    }

    fn resolve_ready(&mut self, device: &wgpu::Device) {
        if self.slots.iter().any(|slot| slot.pending_sample.is_some()) {
            let _ = device.poll(wgpu::PollType::Poll);
        }
        for slot in &mut self.slots {
            let Some(result) = slot
                .completion
                .lock()
                .expect("showcase timestamp completion lock is not poisoned")
                .take()
            else {
                continue;
            };
            let (epoch, outer_step) = slot
                .pending_sample
                .take()
                .expect("mapped timestamp slot retains its outer step");
            if let Err(error) = result {
                self.error = Some(format!("timestamp mapping failed: {error}"));
                return;
            }
            let mapped = slot.readback.slice(..).get_mapped_range();
            let values = mapped
                .chunks_exact(size_of::<u64>())
                .map(|chunk| {
                    u64::from_ne_bytes(chunk.try_into().expect("timestamp chunk is eight bytes"))
                })
                .collect::<Vec<_>>();
            drop(mapped);
            slot.readback.unmap();
            if values.len() != SHOWCASE_TIMESTAMP_COUNT as usize
                || values.windows(2).any(|pair| pair[1] < pair[0])
            {
                self.error = Some("GPU timestamps were incomplete or out of order".to_owned());
                return;
            }
            let milliseconds = |start: usize, end: usize| {
                (values[end] - values[start]) as f64 * self.timestamp_period_nanoseconds * 1.0e-6
            };
            self.latest = Some(ShowcaseGpuTimingSample {
                epoch,
                outer_step,
                physics_ms: milliseconds(0, 1),
                render_ms: milliseconds(1, 2),
                frame_ms: milliseconds(0, 2),
            });
        }
    }
}

#[cfg(target_arch = "wasm32")]
struct Scene {
    name: SceneName,
    config: ShowcaseConfig,
    world: World,
    observation: GpuBodyObservation,
    particle_observation: Option<GpuParticleObservation>,
    renderer: BodyRenderer,
    clock: SimulationClock,
    home_camera: [f32; 4],
    camera: [f32; 4],
    initial_particle_count: u32,
    particle_counts: Option<ParticleCountSnapshot>,
    pending_particle_counts: Option<ParticleCountSnapshotRequest>,
    frame_count: u64,
    last_steps: u32,
    effective_simulation_scale: f64,
    last_timestamp_ms: Option<f64>,
    paused: bool,
    pending_box_target: Option<[f32; 2]>,
    pending_water_target: Option<[f32; 2]>,
    tool_notice: Option<String>,
    pointer_down: bool,
    pointer_target: [f32; 2],
    pointer_dirty: bool,
    begin_pending: bool,
    release_pending: bool,
    constraint: Option<PointConstraintId>,
    commands_queued: bool,
    pending_results: Option<CommandResultRequest>,
}

#[cfg(target_arch = "wasm32")]
impl Scene {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        name: SceneName,
        config: ShowcaseConfig,
    ) -> Result<Self, String> {
        let (mut world, visuals, camera, initial_particle_count) =
            build_scene(device, name, config)?;
        let fixed_dt = world.definition().quality.physics.fixed_dt_seconds();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox_showcase_scene_bootstrap"),
        });
        world
            .step(device, queue, &mut encoder, StepDef::fixed(fixed_dt))
            .map_err(|error| error.to_string())?;
        let observation = world
            .create_gpu_body_observation(device)
            .map_err(|error| error.to_string())?;
        observation.encode(&mut encoder);
        let particle_observation = if config.particle_capacity.is_some() {
            let observation = world
                .create_gpu_particle_observation(device)
                .map_err(|error| error.to_string())?;
            observation.encode(&mut encoder);
            Some(observation)
        } else {
            None
        };
        queue.submit([encoder.finish()]);
        let renderer = BodyRenderer::new(
            device,
            format,
            &observation,
            particle_observation.as_ref(),
            &visuals,
            name.tower_drop() || matches!(name, SceneName::WaterSurge | SceneName::Breakwater),
        );
        Ok(Self {
            name,
            config,
            world,
            observation,
            particle_observation,
            renderer,
            clock: SimulationClock::new(config.physics_hz),
            home_camera: camera,
            camera,
            initial_particle_count,
            particle_counts: None,
            pending_particle_counts: None,
            frame_count: 0,
            last_steps: 1,
            effective_simulation_scale: 0.0,
            last_timestamp_ms: None,
            paused: false,
            pending_box_target: None,
            pending_water_target: None,
            tool_notice: None,
            pointer_down: false,
            pointer_target: [0.0; 2],
            pointer_dirty: false,
            begin_pending: false,
            release_pending: false,
            constraint: None,
            commands_queued: false,
            pending_results: None,
        })
    }

    fn pointer(
        &mut self,
        phase: u8,
        normalized_x: f32,
        normalized_y: f32,
        viewport_size: [u32; 2],
    ) {
        self.pointer_target = self.world_point(normalized_x, normalized_y, viewport_size);
        match phase {
            0 => {
                self.pointer_down = true;
                self.pointer_dirty = true;
                self.begin_pending = true;
                self.release_pending = false;
            }
            1 if self.pointer_down => self.pointer_dirty = true,
            2 | 3 => {
                self.pointer_down = false;
                self.pointer_dirty = false;
                self.begin_pending = false;
                self.release_pending = self.constraint.is_some();
            }
            _ => {}
        }
    }

    fn request_box(&mut self, normalized_x: f32, normalized_y: f32, viewport_size: [u32; 2]) {
        self.pending_box_target = Some(self.world_point(normalized_x, normalized_y, viewport_size));
    }

    fn request_water(&mut self, normalized_x: f32, normalized_y: f32, viewport_size: [u32; 2]) {
        if self.config.particle_capacity.is_some() {
            self.pending_water_target =
                Some(self.world_point(normalized_x, normalized_y, viewport_size));
        } else {
            self.tool_notice = Some("Set Max water and press Reset to enable Water".to_owned());
        }
    }

    fn pan(&mut self, normalized_dx: f32, normalized_dy: f32, viewport_size: [u32; 2]) {
        let camera = fitted_camera(self.camera, viewport_size);
        self.camera[0] -= normalized_dx * 2.0 * camera[2];
        self.camera[1] += normalized_dy * 2.0 * camera[3];
    }

    fn zoom(&mut self, normalized_x: f32, normalized_y: f32, factor: f32, viewport_size: [u32; 2]) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let before = self.world_point(normalized_x, normalized_y, viewport_size);
        let minimum_half_height = 0.35;
        let maximum_half_height = (self.home_camera[3] * 8.0).max(minimum_half_height);
        let requested_half_height =
            (self.camera[3] * factor).clamp(minimum_half_height, maximum_half_height);
        let applied_factor = requested_half_height / self.camera[3];
        self.camera[2] *= applied_factor;
        self.camera[3] = requested_half_height;
        let after = self.world_point(normalized_x, normalized_y, viewport_size);
        self.camera[0] += before[0] - after[0];
        self.camera[1] += before[1] - after[1];
    }

    fn world_point(
        &self,
        normalized_x: f32,
        normalized_y: f32,
        viewport_size: [u32; 2],
    ) -> [f32; 2] {
        let camera = fitted_camera(self.camera, viewport_size);
        [
            camera[0] + (normalized_x.clamp(0.0, 1.0) * 2.0 - 1.0) * camera[2],
            camera[1] + (1.0 - normalized_y.clamp(0.0, 1.0) * 2.0) * camera[3],
        ]
    }

    fn reconcile_results(&mut self, device: &wgpu::Device) -> Result<(), String> {
        if self.pending_results.is_some() || self.pending_particle_counts.is_some() {
            let _ = device.poll(wgpu::PollType::Poll);
        }
        if let Some(request) = self.pending_results.as_mut()
            && let Some(results) = self.world.try_take_command_results(request)
        {
            let results = results.map_err(|error| error.to_string())?;
            for result in results.results {
                if result.kind == WorldCommandKind::CreatePointConstraint
                    && result.status != WorldCommandStatus::Succeeded
                {
                    self.constraint = None;
                    self.release_pending = false;
                }
                if result.kind == WorldCommandKind::DestroyPointConstraint
                    && result.status == WorldCommandStatus::Succeeded
                {
                    self.constraint = None;
                    self.release_pending = false;
                }
            }
            self.pending_results = None;
            self.commands_queued = false;
        }
        if let Some(request) = self.pending_particle_counts.as_mut()
            && let Some(counts) = request.try_take()
        {
            self.particle_counts = Some(counts.map_err(|error| error.to_string())?);
            self.pending_particle_counts = None;
        }
        Ok(())
    }

    fn queue_pointer_action(&mut self) -> Result<(), String> {
        if self.pending_results.is_some() || self.commands_queued {
            return Ok(());
        }
        if self.release_pending {
            if let Some(constraint) = self.constraint {
                self.world
                    .queue_destroy_point_constraint(constraint)
                    .map_err(|error| error.to_string())?;
                self.commands_queued = true;
            } else {
                self.release_pending = false;
            }
            return Ok(());
        }
        if self.begin_pending {
            let selection = self
                .world
                .queue_point_query(PointQueryDef::at(self.pointer_target))
                .map_err(|error| error.to_string())?;
            let constraint = self
                .world
                .queue_create_point_constraint(
                    selection.selection,
                    PointConstraintDef::at_target(self.pointer_target),
                )
                .map_err(|error| error.to_string())?;
            self.constraint = Some(constraint.constraint);
            self.begin_pending = false;
            self.pointer_dirty = false;
            self.commands_queued = true;
        } else if self.pointer_dirty
            && let Some(constraint) = self.constraint
        {
            self.world
                .queue_set_point_constraint_target(constraint, self.pointer_target)
                .map_err(|error| error.to_string())?;
            self.pointer_dirty = false;
            self.commands_queued = true;
        }
        Ok(())
    }

    fn queue_placement_actions(&mut self, queue: &wgpu::Queue, steps: u32) {
        if steps != 1 {
            return;
        }
        if let Some(target) = self.pending_box_target.take() {
            let result = self
                .world
                .create_body(BodyDef::dynamic().with_position(target))
                .and_then(|body| {
                    let shape = ShapeDef::standard_runtime_rectangle();
                    self.world.create_shape(body, shape).map(|_| (body, shape))
                });
            match result {
                Ok((body, shape)) => {
                    self.renderer.write_visual(
                        queue,
                        body.slot(),
                        shape_visual(body, shape, palette(body.slot() as usize)),
                    );
                    self.tool_notice = None;
                }
                Err(error) => self.tool_notice = Some(error.to_string()),
            }
        }
        if let Some(target) = self.pending_water_target.take() {
            match self
                .world
                .queue_particle_emission(ParticleEmissionDef::water_circle(target, 0.4))
            {
                Ok(_) => self.tool_notice = None,
                Err(error) => self.tool_notice = Some(error.to_string()),
            }
        }
    }

    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        gpu_timing: &mut ShowcaseGpuTiming,
        target: &wgpu::TextureView,
        size: [u32; 2],
        timestamp_ms: f64,
    ) -> Result<(), String> {
        gpu_timing.resolve_ready(device);
        self.reconcile_results(device)?;
        self.queue_pointer_action()?;
        let elapsed_seconds = self
            .last_timestamp_ms
            .map_or(1.0 / 60.0, |last| (timestamp_ms - last).max(0.0) / 1000.0);
        self.last_timestamp_ms = Some(timestamp_ms);
        let (steps, pacing) = self.clock.admit(InteractiveClockInput {
            elapsed_seconds,
            requested_time_scale: if self.paused { 0.0 } else { 1.0 },
            action: InteractiveClockAction::Run,
        });
        self.last_steps = steps;
        self.effective_simulation_scale = pacing.effective_simulation_scale;
        self.queue_placement_actions(queue, steps);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox_showcase_frame"),
        });
        let next_outer_step = self.world.counters().outer_steps.saturating_add(1);
        let timing_slot = (steps == 1)
            .then(|| gpu_timing.candidate_slot(next_outer_step))
            .flatten();
        if let Some(slot) = timing_slot {
            gpu_timing.write_marker(
                &mut encoder,
                slot,
                0,
                "physbox_showcase_timestamp_frame_start",
            );
        }
        let encoded_commands = steps == 1 && self.commands_queued && self.pending_results.is_none();
        if steps == 1 {
            self.world
                .step(
                    device,
                    queue,
                    &mut encoder,
                    StepDef::fixed(self.clock.fixed_step_seconds() as f32),
                )
                .map_err(|error| error.to_string())?;
        }
        if let Some(slot) = timing_slot {
            gpu_timing.write_marker(
                &mut encoder,
                slot,
                1,
                "physbox_showcase_timestamp_physics_end",
            );
        }
        self.observation.encode(&mut encoder);
        if let Some(observation) = &self.particle_observation {
            observation.encode(&mut encoder);
        }
        let mut result_request = if encoded_commands {
            Some(
                self.world
                    .request_command_results(device, &mut encoder)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        let mut particle_count_request = if self.particle_observation.is_some()
            && self.pending_particle_counts.is_none()
            && self.frame_count.is_multiple_of(15)
        {
            Some(
                self.world
                    .request_particle_count_snapshot(device, &mut encoder)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        self.renderer.encode(
            device,
            queue,
            &mut encoder,
            target,
            size,
            fitted_camera(self.camera, size),
        );
        if let Some(slot) = timing_slot {
            gpu_timing.write_marker(
                &mut encoder,
                slot,
                2,
                "physbox_showcase_timestamp_frame_end",
            );
            gpu_timing.encode_resolve(&mut encoder, slot, next_outer_step);
        }
        queue.submit([encoder.finish()]);
        if let Some(request) = result_request.as_mut() {
            request.begin_map().map_err(|error| error.to_string())?;
        }
        if result_request.is_some() {
            self.pending_results = result_request;
        }
        if let Some(request) = particle_count_request.as_mut() {
            request.begin_map().map_err(|error| error.to_string())?;
        }
        if particle_count_request.is_some() {
            self.pending_particle_counts = particle_count_request;
        }
        if let Some(slot) = timing_slot {
            gpu_timing.start_mapping(slot);
        }
        self.frame_count = self.frame_count.saturating_add(1);
        Ok(())
    }

    fn live_particle_count(&self) -> u32 {
        self.particle_counts
            .map_or(self.initial_particle_count, |counts| counts.live_particles)
    }

    fn dropped_particle_count(&self) -> u32 {
        self.particle_counts
            .map_or(0, |counts| counts.dropped_particles)
    }
}

#[cfg(target_arch = "wasm32")]
struct BodyRenderer {
    params: wgpu::Buffer,
    shape_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    capacity: u32,
    sleep_visualization: bool,
    particles: Option<ParticleRenderer>,
}

#[cfg(target_arch = "wasm32")]
struct ParticleRenderer {
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    capacity: u32,
    radius: f32,
    surface: Option<water_surface::WaterSurface>,
}

#[cfg(target_arch = "wasm32")]
impl BodyRenderer {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        observation: &GpuBodyObservation,
        particle_observation: Option<&GpuParticleObservation>,
        visuals: &[ShapeVisual],
        water_surface: bool,
    ) -> Self {
        let shape_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox_showcase_shape_visuals"),
            contents: bytemuck::cast_slice(visuals),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox_showcase_render_params"),
            size: size_of::<RenderParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox_showcase_render_layout"),
            entries: &[
                storage_entry(0),
                storage_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox_showcase_render_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: observation.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: shape_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox_showcase_body_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_render.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox_showcase_render_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("physbox_showcase_body_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("body_vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("body_fragment"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let particles = particle_observation.map(|observation| {
            ParticleRenderer::new(device, format, observation, &params, water_surface)
        });
        Self {
            params,
            shape_buffer,
            bind_group,
            pipeline,
            capacity: observation.capacity(),
            sleep_visualization: true,
            particles,
        }
    }

    fn set_sleep_visualization(&mut self, enabled: bool) {
        self.sleep_visualization = enabled;
    }

    const fn sleep_visualization(&self) -> bool {
        self.sleep_visualization
    }

    fn write_visual(&self, queue: &wgpu::Queue, slot: u32, visual: ShapeVisual) {
        queue.write_buffer(
            &self.shape_buffer,
            u64::from(slot) * size_of::<ShapeVisual>() as u64,
            bytemuck::bytes_of(&visual),
        );
    }

    fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: [u32; 2],
        camera: [f32; 4],
    ) {
        queue.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&RenderParams {
                camera,
                viewport: [size[0] as f32, size[1] as f32, 0.0, 0.0],
                material: [
                    self.particles
                        .as_ref()
                        .map_or(0.0, |particles| particles.radius),
                    if self.sleep_visualization { 1.0 } else { 0.0 },
                    0.0,
                    0.0,
                ],
            }),
        );
        if let Some(particles) = &mut self.particles
            && let Some(surface) = &mut particles.surface
        {
            surface.accumulate(
                device,
                encoder,
                size,
                &particles.pipeline,
                &particles.bind_group,
                particles.capacity,
            );
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox_showcase_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.018,
                        g: 0.035,
                        b: 0.067,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if let Some(particles) = &self.particles {
            if let Some(surface) = &particles.surface {
                surface.draw(&mut pass);
            } else {
                pass.set_pipeline(&particles.pipeline);
                pass.set_bind_group(0, &particles.bind_group, &[]);
                pass.draw(0..6, 0..particles.capacity);
            }
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..6, 0..self.capacity);
    }
}

#[cfg(target_arch = "wasm32")]
impl ParticleRenderer {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        observation: &GpuParticleObservation,
        params: &wgpu::Buffer,
        use_surface: bool,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox_showcase_particle_render_layout"),
            entries: &[
                storage_entry(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox_showcase_particle_render_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: observation.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox_showcase_particle_shader"),
            source: wgpu::ShaderSource::Wgsl(
                (if use_surface {
                    include_str!("water_thickness.wgsl")
                } else {
                    include_str!("particle_render.wgsl")
                })
                .into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox_showcase_particle_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("physbox_showcase_particle_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("particle_vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("particle_fragment"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: if use_surface {
                        wgpu::TextureFormat::Rgba8Unorm
                    } else {
                        format
                    },
                    blend: Some(if use_surface {
                        wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent::REPLACE,
                        }
                    } else {
                        wgpu::BlendState::ALPHA_BLENDING
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self {
            bind_group,
            pipeline,
            capacity: observation.capacity(),
            surface: use_surface.then(|| water_surface::WaterSurface::new(device, format)),
            radius: observation.particle_radius() * 1.15,
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct ShowcaseApp {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    gpu_timing: ShowcaseGpuTiming,
    scene: Scene,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl ShowcaseApp {
    pub fn render(&mut self, timestamp_ms: f64) -> Result<(), JsValue> {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                self.surface.get_current_texture().map_err(|error| {
                    js_error(format!("failed to recover showcase surface: {error}"))
                })?
            }
            Err(error) => {
                return Err(js_error(format!(
                    "failed to acquire showcase surface: {error}"
                )));
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("physbox_showcase_surface_view"),
            format: Some(self.view_format),
            ..Default::default()
        });
        self.scene
            .render(
                &self.device,
                &self.queue,
                &mut self.gpu_timing,
                &view,
                [self.surface_config.width, self.surface_config.height],
                timestamp_ms,
            )
            .map_err(js_error)?;
        frame.present();
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
    }

    #[wasm_bindgen(js_name = selectScene)]
    pub fn select_scene(&mut self, name: &str) -> Result<(), JsValue> {
        let name = SceneName::parse(name).map_err(js_error)?;
        self.scene = Scene::new(
            &self.device,
            &self.queue,
            self.view_format,
            name,
            self.scene.config,
        )
        .map_err(js_error)?;
        self.gpu_timing.begin_scene();
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), JsValue> {
        self.scene = Scene::new(
            &self.device,
            &self.queue,
            self.view_format,
            self.scene.name,
            self.scene.config,
        )
        .map_err(js_error)?;
        self.gpu_timing.begin_scene();
        Ok(())
    }

    #[wasm_bindgen(js_name = configureAndReset)]
    pub fn configure_and_reset(
        &mut self,
        scene_name: &str,
        body_capacity: u32,
        particle_capacity: u32,
        physics_hz: u32,
    ) -> Result<(), JsValue> {
        let name = SceneName::parse(scene_name).map_err(js_error)?;
        let config = ShowcaseConfig::parse(body_capacity, particle_capacity, physics_hz)
            .map_err(js_error)?;
        let replacement = Scene::new(&self.device, &self.queue, self.view_format, name, config)
            .map_err(js_error)?;
        self.scene = replacement;
        self.gpu_timing.begin_scene();
        Ok(())
    }

    #[wasm_bindgen(js_name = setPaused)]
    pub fn set_paused(&mut self, paused: bool) {
        self.scene.paused = paused;
    }

    #[wasm_bindgen(js_name = pointerEvent)]
    pub fn pointer_event(&mut self, phase: u8, x: f32, y: f32) {
        self.scene.pointer(
            phase,
            x,
            y,
            [self.surface_config.width, self.surface_config.height],
        );
    }

    #[wasm_bindgen(js_name = placeBox)]
    pub fn place_box(&mut self, x: f32, y: f32) {
        self.scene.request_box(
            x,
            y,
            [self.surface_config.width, self.surface_config.height],
        );
    }

    #[wasm_bindgen(js_name = emitWater)]
    pub fn emit_water(&mut self, x: f32, y: f32) {
        self.scene.request_water(
            x,
            y,
            [self.surface_config.width, self.surface_config.height],
        );
    }

    #[wasm_bindgen(js_name = panView)]
    pub fn pan_view(&mut self, dx: f32, dy: f32) {
        self.scene.pan(
            dx,
            dy,
            [self.surface_config.width, self.surface_config.height],
        );
    }

    #[wasm_bindgen(js_name = zoomView)]
    pub fn zoom_view(&mut self, x: f32, y: f32, factor: f32) {
        self.scene.zoom(
            x,
            y,
            factor,
            [self.surface_config.width, self.surface_config.height],
        );
    }

    #[wasm_bindgen(js_name = setSleepVisualization)]
    pub fn set_sleep_visualization(&mut self, enabled: bool) {
        self.scene.renderer.set_sleep_visualization(enabled);
    }

    #[wasm_bindgen(getter)]
    pub fn scene(&self) -> String {
        self.scene.name.slug().to_owned()
    }

    #[wasm_bindgen(getter, js_name = frameCount)]
    pub fn frame_count(&self) -> u64 {
        self.scene.frame_count
    }

    #[wasm_bindgen(getter, js_name = fixedSteps)]
    pub fn fixed_steps(&self) -> u64 {
        self.scene.world.counters().outer_steps
    }

    #[wasm_bindgen(getter, js_name = fixedStepsThisFrame)]
    pub fn fixed_steps_this_frame(&self) -> u32 {
        self.scene.last_steps
    }

    #[wasm_bindgen(getter, js_name = liveBodies)]
    pub fn live_bodies(&self) -> u32 {
        self.scene.world.counters().live_bodies
    }

    #[wasm_bindgen(getter, js_name = bodyCapacity)]
    pub fn body_capacity(&self) -> u32 {
        self.scene.config.body_capacity.slots()
    }

    #[wasm_bindgen(getter, js_name = particleCount)]
    pub fn particle_count(&self) -> u32 {
        self.scene.live_particle_count()
    }

    #[wasm_bindgen(getter, js_name = particleCapacity)]
    pub fn particle_capacity(&self) -> u32 {
        self.scene.config.particle_capacity.unwrap_or(0)
    }

    #[wasm_bindgen(getter, js_name = droppedParticles)]
    pub fn dropped_particles(&self) -> u32 {
        self.scene.dropped_particle_count()
    }

    #[wasm_bindgen(getter, js_name = physicsHz)]
    pub fn physics_hz(&self) -> u32 {
        self.scene.config.physics_hz
    }

    #[wasm_bindgen(getter, js_name = admittedPhysicsHz)]
    pub fn admitted_physics_hz(&self) -> f64 {
        self.scene.effective_simulation_scale * f64::from(self.scene.config.physics_hz)
    }

    #[wasm_bindgen(getter, js_name = effectiveSimulationScale)]
    pub fn effective_simulation_scale(&self) -> f64 {
        self.scene.effective_simulation_scale
    }

    #[wasm_bindgen(getter, js_name = gpuTimingAvailable)]
    pub fn gpu_timing_available(&self) -> bool {
        self.gpu_timing.supported() && self.gpu_timing.error.is_none()
    }

    #[wasm_bindgen(getter, js_name = gpuTimingStep)]
    pub fn gpu_timing_step(&self) -> Option<u64> {
        self.gpu_timing.latest().map(|sample| sample.outer_step)
    }

    #[wasm_bindgen(getter, js_name = gpuPhysicsMs)]
    pub fn gpu_physics_ms(&self) -> Option<f64> {
        self.gpu_timing.latest().map(|sample| sample.physics_ms)
    }

    #[wasm_bindgen(getter, js_name = gpuRenderMs)]
    pub fn gpu_render_ms(&self) -> Option<f64> {
        self.gpu_timing.latest().map(|sample| sample.render_ms)
    }

    #[wasm_bindgen(getter, js_name = gpuFrameMs)]
    pub fn gpu_frame_ms(&self) -> Option<f64> {
        self.gpu_timing.latest().map(|sample| sample.frame_ms)
    }

    #[wasm_bindgen(getter, js_name = gpuTimingError)]
    pub fn gpu_timing_error(&self) -> Option<String> {
        self.gpu_timing.error.clone()
    }

    #[wasm_bindgen(getter, js_name = waterAvailable)]
    pub fn water_available(&self) -> bool {
        self.scene.config.particle_capacity.is_some()
    }

    #[wasm_bindgen(getter, js_name = sleepVisualization)]
    pub fn sleep_visualization(&self) -> bool {
        self.scene.renderer.sleep_visualization()
    }

    #[wasm_bindgen(getter, js_name = cameraCenterX)]
    pub fn camera_center_x(&self) -> f32 {
        self.scene.camera[0]
    }

    #[wasm_bindgen(getter, js_name = cameraCenterY)]
    pub fn camera_center_y(&self) -> f32 {
        self.scene.camera[1]
    }

    #[wasm_bindgen(getter, js_name = cameraHalfHeight)]
    pub fn camera_half_height(&self) -> f32 {
        self.scene.camera[3]
    }

    #[wasm_bindgen(getter, js_name = toolNotice)]
    pub fn tool_notice(&self) -> Option<String> {
        self.scene.tool_notice.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn dragging(&self) -> bool {
        self.scene.constraint.is_some()
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(canvas: HtmlCanvasElement, scene_name: String) -> Result<ShowcaseApp, JsValue> {
    console_error_panic_hook::set_once();
    let scene_name = SceneName::parse(&scene_name).map_err(js_error)?;
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..Default::default()
    });
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|error| js_error(format!("failed to create showcase surface: {error}")))?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .map_err(|error| js_error(format!("failed to find a WebGPU adapter: {error}")))?;
    let timestamp_queries_supported = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let required_features = if timestamp_queries_supported {
        wgpu::Features::TIMESTAMP_QUERY
    } else {
        wgpu::Features::empty()
    };
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("physbox_showcase_device"),
            required_features,
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .map_err(|error| js_error(format!("failed to create the WebGPU device: {error}")))?;
    let capabilities = surface.get_capabilities(&adapter);
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| capabilities.formats.first().copied())
        .ok_or_else(|| js_error("WebGPU surface reported no texture formats"))?;
    let view_format = format.add_srgb_suffix();
    let width = canvas.width().max(1);
    let height = canvas.height().max(1);
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: if capabilities
            .present_modes
            .contains(&wgpu::PresentMode::Fifo)
        {
            wgpu::PresentMode::Fifo
        } else {
            capabilities.present_modes[0]
        },
        alpha_mode: capabilities.alpha_modes[0],
        view_formats: if view_format == format {
            Vec::new()
        } else {
            vec![view_format]
        },
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_config);
    let gpu_timing = ShowcaseGpuTiming::new(&device, &queue, timestamp_queries_supported);
    let scene = Scene::new(
        &device,
        &queue,
        view_format,
        scene_name,
        ShowcaseConfig::for_scene(scene_name),
    )
    .map_err(js_error)?;
    Ok(ShowcaseApp {
        _instance: instance,
        surface,
        device,
        queue,
        surface_config,
        view_format,
        gpu_timing,
        scene,
    })
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct RigidStressLayout {
    target_bodies: u32,
    stack_count: u32,
    dynamic_bodies: u32,
    columns: u32,
    rows: u32,
    lower: [f32; 2],
    upper: [f32; 2],
}

#[cfg(any(target_arch = "wasm32", test))]
fn rigid_stress_layout(target_bodies: u32) -> RigidStressLayout {
    const BOUNDARY_BODIES: u32 = 3;
    const MAX_DYNAMIC_BODIES_PER_STACK: u32 = 5;
    let field_bodies = target_bodies.saturating_sub(BOUNDARY_BODIES);
    let stack_count = field_bodies
        .div_ceil(MAX_DYNAMIC_BODIES_PER_STACK + 1)
        .max(1);
    let dynamic_bodies = field_bodies.saturating_sub(stack_count);
    let columns = ((stack_count as f32 * 1.7).sqrt().ceil() as u32).max(1);
    let rows = stack_count.div_ceil(columns).max(1);
    // Keep the clearance and box thickness in world units as the field grows.
    let half_width = columns as f32 * 1.8;
    let lower = [-half_width, -8.6];
    let upper = [half_width, lower[1] + rows as f32 * 3.6];
    RigidStressLayout {
        target_bodies,
        stack_count,
        dynamic_bodies,
        columns,
        rows,
        lower,
        upper,
    }
}

#[cfg(target_arch = "wasm32")]
const DROP_RAMP_COUNT: u32 = 5;
#[cfg(target_arch = "wasm32")]
const DROP_COLUMNS: u32 = 60;

#[cfg(target_arch = "wasm32")]
fn rigid_drop_top(name: SceneName, body_count: u32) -> f32 {
    let rows = (body_count - 3 - DROP_RAMP_COUNT).div_ceil(DROP_COLUMNS);
    let stagger = if name == SceneName::RigidAvalanche2k {
        8.0
    } else {
        2.2
    };
    0.6 + (rows - 1) as f32 * 0.72 + stagger + 1.0
}

#[cfg(target_arch = "wasm32")]
fn add_rigid_drop(
    world: &mut World,
    visuals: &mut [ShapeVisual],
    name: SceneName,
    body_count: u32,
) -> Result<(), String> {
    for index in 0..DROP_RAMP_COUNT {
        let alternate = index % 2 == 0;
        add_body(
            world,
            visuals,
            BodyDef::fixed()
                .with_position([
                    (index as f32 - 2.0) * 10.0,
                    if alternate { -4.0 } else { -5.5 },
                ])
                .with_angle(if alternate { -0.32 } else { 0.32 }),
            ShapeDef::rectangle([2.8, 0.22]).with_friction(0.48),
            [0.27, 0.36, 0.47, 1.0],
        )?;
    }
    let avalanche = name == SceneName::RigidAvalanche2k;
    for index in 0..(body_count - 3 - DROP_RAMP_COUNT) {
        let column = index % DROP_COLUMNS;
        let row = index / DROP_COLUMNS;
        let stagger = if avalanche {
            8.0 * (1.0 - column as f32 / (DROP_COLUMNS - 1) as f32)
        } else {
            (column % 12) as f32 * 0.2
        };
        let x = (column as f32 - (DROP_COLUMNS - 1) as f32 * 0.5) * 0.9 + (row % 2) as f32 * 0.22;
        let y = 0.6 + row as f32 * 0.72 + stagger;
        let shape = if (row + column).is_multiple_of(2) {
            ShapeDef::circle(0.27)
        } else {
            ShapeDef::rectangle([0.29, 0.22])
        };
        add_body(
            world,
            visuals,
            BodyDef::dynamic()
                .with_position([x, y])
                .with_angle(((column + row * 3) % 7) as f32 * 0.09 - 0.27)
                .with_linear_velocity([if avalanche { 0.7 } else { 0.0 }, -2.0]),
            shape
                .with_density(0.8)
                .with_friction(0.42)
                .with_restitution(0.18),
            palette((row / 2 + column / 6) as usize),
        )?;
    }
    debug_assert_eq!(world.counters().live_bodies, body_count);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn build_scene(
    device: &wgpu::Device,
    name: SceneName,
    config: ShowcaseConfig,
) -> Result<(World, Vec<ShapeVisual>, [f32; 4], u32), String> {
    let rigid_stress_count = name.rigid_stress_body_count();
    let rigid_drop_count = name.rigid_drop_body_count();
    if let Some(count) = rigid_stress_count.or(rigid_drop_count)
        && count > config.body_capacity.slots()
    {
        return Err(format!(
            "{} requires Max bodies of at least {count}",
            name.slug()
        ));
    }
    let definition = WorldDef {
        capacity: config.body_capacity,
        quality: physbox_sim::QualityDef {
            physics: physbox_sim::PhysicsConfigurationV3 {
                fixed_step_hz: config.physics_hz,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let mut world = World::new(device, definition).map_err(|error| error.to_string())?;
    let mut visuals = vec![ShapeVisual::zeroed(); config.body_capacity.slots() as usize];
    let rigid_stress_layout = rigid_stress_count.map(rigid_stress_layout);
    let drop_top = rigid_drop_count.map(|count| rigid_drop_top(name, count));
    let water_stress_region = if name.water_stress_capacity().is_some() {
        config.particle_capacity.map(water_stress_region)
    } else {
        None
    };
    let camera = if name.tower_drop() {
        [0.0, 3.2, 14.2, 12.4]
    } else if name == SceneName::DominoRun {
        [0.0, -0.85, 11.6, 3.8]
    } else if name == SceneName::Breakwater {
        [0.0, 0.25, 14.85, 5.1]
    } else if let Some(layout) = rigid_stress_layout {
        [
            0.0,
            (layout.lower[1] + layout.upper[1]) * 0.5,
            layout.upper[0] + 1.2,
            (layout.upper[1] - layout.lower[1]) * 0.5 + 1.2,
        ]
    } else if let Some(top) = drop_top {
        [0.0, (top - 9.6) * 0.5, 30.0, (top + 9.6) * 0.5]
    } else {
        water_stress_region.map_or([0.0, 0.0, 8.0, 5.0], |(lower, upper, _)| {
            let left = lower[0] - 0.8;
            let right = upper[0] + 0.8;
            let bottom = lower[1] - 1.0;
            let top = upper[1] + 1.0;
            [
                (left + right) * 0.5,
                (bottom + top) * 0.5,
                (right - left) * 0.5,
                (top - bottom) * 0.5,
            ]
        })
    };
    let (floor_position, floor_half_extents, left_position, wall_half_extents) =
        if name.tower_drop() {
            ([0.0, -8.3], [13.3, 0.3], [-13.1, 2.7], [0.2, 11.0])
        } else if name == SceneName::DominoRun {
            ([0.0, -4.45], [11.2, 0.35], [-11.0, -0.85], [0.2, 3.4])
        } else if name == SceneName::WaterSurge {
            ([0.0, -4.45], [7.85, 0.35], [-7.35, 0.35], [0.4, 4.55])
        } else if name == SceneName::Breakwater {
            ([0.0, -4.45], [14.6, 0.35], [-14.25, 0.32], [0.32, 4.60])
        } else if let Some(top) = drop_top {
            (
                [0.0, -8.65],
                [29.0, 0.35],
                [-28.8, (top - 8.65) * 0.5],
                [0.2, (top + 8.65) * 0.5],
            )
        } else if let Some(layout) = rigid_stress_layout {
            let floor_y = layout.lower[1] - 0.3;
            let wall_top = layout.upper[1] + 0.4;
            (
                [0.0, floor_y],
                [(layout.upper[0] - layout.lower[0]) * 0.5 + 0.45, 0.3],
                [layout.lower[0] - 0.25, (floor_y + wall_top) * 0.5],
                [0.2, (wall_top - floor_y) * 0.5],
            )
        } else {
            water_stress_region.map_or(
                ([0.0, -4.45], [7.5, 0.35], [-7.15, 0.0], [0.2, 4.2]),
                |(lower, upper, _)| {
                    let half_width = (upper[0] - lower[0]) * 0.5 + 0.45;
                    let floor_y = lower[1] - 0.3;
                    let wall_top = upper[1] + 0.6;
                    (
                        [0.0, floor_y],
                        [half_width, 0.3],
                        [-half_width + 0.2, (floor_y + wall_top) * 0.5],
                        [0.2, (wall_top - floor_y) * 0.5],
                    )
                },
            )
        };
    add_body(
        &mut world,
        &mut visuals,
        BodyDef::fixed().with_position(floor_position),
        ShapeDef::rectangle(floor_half_extents)
            .with_density(0.0)
            .with_friction(0.7)
            .with_restitution(0.0),
        [0.17, 0.23, 0.32, 1.0],
    )?;
    add_body(
        &mut world,
        &mut visuals,
        BodyDef::fixed().with_position(left_position),
        ShapeDef::rectangle(wall_half_extents)
            .with_density(0.0)
            .with_friction(0.7)
            .with_restitution(0.0),
        [0.12, 0.18, 0.27, 1.0],
    )?;
    add_body(
        &mut world,
        &mut visuals,
        BodyDef::fixed().with_position([-left_position[0], left_position[1]]),
        ShapeDef::rectangle(wall_half_extents)
            .with_density(0.0)
            .with_friction(0.7)
            .with_restitution(0.0),
        [0.12, 0.18, 0.27, 1.0],
    )?;
    let mut particle_definition = config.particle_capacity.map(ParticleSystemDef::water);
    let initial_particle_count = match name {
        SceneName::DominoRun => {
            demos::add_domino_run(&mut world, &mut visuals)?;
            0
        }
        SceneName::MarblePegboard => {
            demos::add_marble_pegboard(&mut world, &mut visuals)?;
            0
        }
        SceneName::WaterSurge => {
            demos::add_water_surge(&mut world, &mut visuals, &mut particle_definition)?
        }
        SceneName::SeesawLaunch => {
            demos::add_seesaw_launch(&mut world, &mut visuals)?;
            0
        }
        SceneName::Breakwater => {
            demos::add_breakwater(&mut world, &mut visuals, &mut particle_definition)?
        }
        SceneName::BounceTrio => {
            demos::add_bounce_trio(&mut world, &mut visuals)?;
            0
        }
        SceneName::PullABrick => {
            demos::add_brick_tower(&mut world, &mut visuals)?;
            0
        }
        SceneName::Playground => {
            for row in 0..3 {
                for column in 0..6 {
                    let x = -4.5 + column as f32 * 1.75;
                    let y = -2.9 + row as f32 * 1.45;
                    let color = palette((row * 6 + column) as usize);
                    if (row + column) % 2 == 0 {
                        add_body(
                            &mut world,
                            &mut visuals,
                            BodyDef::dynamic().with_position([x, y]),
                            ShapeDef::circle(0.48)
                                .with_density(0.8)
                                .with_friction(0.45)
                                .with_restitution(0.08),
                            color,
                        )?;
                    } else {
                        add_body(
                            &mut world,
                            &mut visuals,
                            BodyDef::dynamic()
                                .with_position([x, y])
                                .with_angle((column as f32 - row as f32) * 0.08),
                            ShapeDef::rectangle([0.52, 0.42])
                                .with_density(0.8)
                                .with_friction(0.55)
                                .with_restitution(0.03),
                            color,
                        )?;
                    }
                }
            }
            0
        }
        SceneName::StackAndSleep => {
            for index in 0..8 {
                add_body(
                    &mut world,
                    &mut visuals,
                    BodyDef::dynamic().with_position([0.0, -2.75 + index as f32 * 0.86]),
                    ShapeDef::rectangle([0.58, 0.4])
                        .with_density(0.9)
                        .with_friction(0.62)
                        .with_restitution(0.0),
                    palette(index),
                )?;
            }
            for (index, x) in [-4.0, -2.7, 2.7, 4.0].into_iter().enumerate() {
                add_body(
                    &mut world,
                    &mut visuals,
                    BodyDef::dynamic().with_position([x, -2.75 + 0.2 * (index % 2) as f32]),
                    ShapeDef::circle(0.42)
                        .with_density(0.8)
                        .with_friction(0.5)
                        .with_restitution(0.05),
                    palette(index + 8),
                )?;
            }
            0
        }
        SceneName::TowerDrop13k | SceneName::TowerDrop32k => {
            // Offset each course so the tower opens up as it hits the water.
            for row in 0..5 {
                for column in 0..2 {
                    let x = -5.1 + 0.45 * row as f32 + 2.65 * column as f32;
                    let y = 8.2 + row as f32 * 1.42;
                    add_body(
                        &mut world,
                        &mut visuals,
                        BodyDef::dynamic()
                            .with_position([x, y])
                            .with_linear_velocity([1.0 + 0.6 * row as f32, -6.0])
                            .with_angular_velocity(-0.18 - 0.08 * row as f32),
                        ShapeDef::rectangle([1.28, 0.67])
                            .with_density(1.3)
                            .with_friction(0.48)
                            .with_restitution(0.08),
                        palette((row * 2 + column) as usize),
                    )?;
                }
            }
            if let Some(mut particles) = particle_definition.take() {
                particles = particles.with_profile(physbox_sim::WaterProfile::HighCount);
                let capacity = particles.capacity();
                let spacing = match capacity {
                    32_768 => 0.1,
                    16_384 => 0.1575,
                    8_192 => 0.205,
                    _ => 0.29,
                };
                particles = particles.with_region(ParticleRegionDef::rectangle(
                    [-12.75, -7.99],
                    [12.76, 4.82],
                    spacing,
                ));
                let count = particles
                    .initial_particle_count()
                    .map_err(|e| e.to_string())?;
                particle_definition = Some(particles);
                count
            } else {
                0
            }
        }
        SceneName::WaterAndRigid => {
            add_body(
                &mut world,
                &mut visuals,
                BodyDef::fixed()
                    .with_position([2.0, -2.0])
                    .with_angle(-0.18),
                ShapeDef::rectangle([2.1, 0.16])
                    .with_density(0.0)
                    .with_friction(0.65)
                    .with_restitution(0.0),
                [0.14, 0.22, 0.34, 1.0],
            )?;
            for (index, (position, geometry)) in [
                (
                    [-4.2, 2.2],
                    ShapeGeometry::Circle(physbox_sim::Circle { radius: 0.5 }),
                ),
                (
                    [-2.8, 2.7],
                    ShapeGeometry::Rectangle(physbox_sim::Rectangle {
                        half_extents: [0.58, 0.46],
                    }),
                ),
                (
                    [0.2, 1.3],
                    ShapeGeometry::Rectangle(physbox_sim::Rectangle {
                        half_extents: [0.72, 0.38],
                    }),
                ),
                (
                    [3.2, 2.8],
                    ShapeGeometry::Circle(physbox_sim::Circle { radius: 0.62 }),
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let shape = match geometry {
                    ShapeGeometry::Circle(circle) => ShapeDef::circle(circle.radius),
                    ShapeGeometry::Rectangle(rectangle) => {
                        ShapeDef::rectangle(rectangle.half_extents)
                    }
                }
                .with_density(0.9)
                .with_friction(0.52)
                .with_restitution(0.02);
                add_body(
                    &mut world,
                    &mut visuals,
                    BodyDef::dynamic()
                        .with_position(position)
                        .with_angle((index as f32 - 1.5) * 0.09),
                    shape,
                    palette(index + 2),
                )?;
            }
            if let Some(particles) = particle_definition.take() {
                let particles = particles.with_region(ParticleRegionDef::rectangle(
                    [-6.5, -4.0],
                    [-1.2, 1.0],
                    0.1,
                ));
                let count = particles
                    .initial_particle_count()
                    .map_err(|error| error.to_string())?;
                particle_definition = Some(particles);
                count
            } else {
                0
            }
        }
        SceneName::RigidStress1k
        | SceneName::RigidStress2k
        | SceneName::RigidStress5k
        | SceneName::RigidStress10k => {
            let layout = rigid_stress_layout.expect("rigid stress layout was computed above");
            let cell_width = (layout.upper[0] - layout.lower[0]) / layout.columns as f32;
            let cell_height = (layout.upper[1] - layout.lower[1]) / layout.rows as f32;
            let platform_half_extents = [cell_width * 0.38, cell_height * 0.028];
            let dynamic_half_extents = [cell_width * 0.28, cell_height * 0.048];
            let base_stack_height = layout.dynamic_bodies / layout.stack_count;
            let taller_stack_count = layout.dynamic_bodies % layout.stack_count;
            let mut authored_dynamics = 0u32;
            for stack_index in 0..layout.stack_count {
                let column = stack_index % layout.columns;
                let row = stack_index / layout.columns;
                let cell_lower = [
                    layout.lower[0] + column as f32 * cell_width,
                    layout.lower[1] + row as f32 * cell_height,
                ];
                let platform_position = [
                    cell_lower[0] + cell_width * 0.5,
                    cell_lower[1] + cell_height * 0.08,
                ];
                add_body(
                    &mut world,
                    &mut visuals,
                    BodyDef::fixed().with_position(platform_position),
                    ShapeDef::rectangle(platform_half_extents)
                        .with_density(0.0)
                        .with_friction(0.65)
                        .with_restitution(0.0),
                    [0.15, 0.21, 0.29, 1.0],
                )?;
                let bodies_in_stack =
                    base_stack_height + u32::from(stack_index < taller_stack_count);
                let platform_top = platform_position[1] + platform_half_extents[1];
                let drop_clearance = cell_height * (0.12 + 0.045 * ((column + row) % 7) as f32);
                for level in 0..bodies_in_stack {
                    let position = [
                        platform_position[0],
                        platform_top
                            + drop_clearance
                            + dynamic_half_extents[1]
                            + level as f32 * dynamic_half_extents[1] * 2.0,
                    ];
                    add_body(
                        &mut world,
                        &mut visuals,
                        BodyDef::dynamic().with_position(position),
                        ShapeDef::rectangle(dynamic_half_extents)
                            .with_density(0.8)
                            .with_friction(0.65)
                            .with_restitution(0.0),
                        palette(((column + 2 * row) % 6 + level) as usize),
                    )?;
                    authored_dynamics += 1;
                }
            }
            debug_assert_eq!(authored_dynamics, layout.dynamic_bodies);
            debug_assert_eq!(world.counters().live_bodies, layout.target_bodies);
            0
        }
        SceneName::RigidRain1k | SceneName::RigidAvalanche2k => {
            add_rigid_drop(
                &mut world,
                &mut visuals,
                name,
                rigid_drop_count.expect("drop scenes have a body count"),
            )?;
            0
        }
        SceneName::WaterStress4k | SceneName::WaterStress8k | SceneName::WaterStress16k => {
            let (lower, upper, expected_count) =
                water_stress_region.unwrap_or(([-6.5, -4.0], [6.5, 1.0], 0));
            let water_width = upper[0] - lower[0];
            for index in 0..8 {
                let x = -water_width * 0.35 + index as f32 * water_width * 0.1;
                let y = lower[1] + 1.2 + (index % 3) as f32 * 0.75;
                let shape = if index % 2 == 0 {
                    ShapeDef::circle(0.34)
                } else {
                    ShapeDef::rectangle([0.42, 0.28])
                }
                .with_density(0.75)
                .with_friction(0.5)
                .with_restitution(0.0);
                add_body(
                    &mut world,
                    &mut visuals,
                    BodyDef::dynamic()
                        .with_position([x, y])
                        .with_angle((index as f32 - 3.5) * 0.04),
                    shape,
                    palette(index as usize + 2),
                )?;
            }
            if let Some(particles) = particle_definition.take() {
                let particles =
                    particles.with_region(ParticleRegionDef::rectangle(lower, upper, 0.1));
                let count = particles
                    .initial_particle_count()
                    .map_err(|error| error.to_string())?;
                if count != expected_count {
                    return Err(format!(
                        "water stress authored {count} particles, expected {expected_count}"
                    ));
                }
                particle_definition = Some(particles);
                count
            } else {
                0
            }
        }
    };
    if let Some(particles) = particle_definition {
        world
            .set_particle_system(particles)
            .map_err(|error| error.to_string())?;
    }
    Ok((world, visuals, camera, initial_particle_count))
}

#[cfg(any(target_arch = "wasm32", test))]
fn water_stress_region(capacity: u32) -> ([f32; 2], [f32; 2], u32) {
    let target = capacity * 4 / 5;
    let columns = ((target as f32 * 1.2).sqrt().ceil() as u32).max(1);
    let rows = (target / columns).max(1);
    let count = columns * rows;
    let width = columns as f32 * 0.1;
    let height = rows as f32 * 0.1;
    ([-width * 0.5, -4.0], [width * 0.5, -4.0 + height], count)
}

#[cfg(any(target_arch = "wasm32", test))]
fn fitted_camera(mut camera: [f32; 4], viewport_size: [u32; 2]) -> [f32; 4] {
    let viewport_aspect = viewport_size[0].max(1) as f32 / viewport_size[1].max(1) as f32;
    let authored_aspect = camera[2] / camera[3];
    if viewport_aspect > authored_aspect {
        camera[2] = camera[3] * viewport_aspect;
    } else {
        camera[3] = camera[2] / viewport_aspect;
    }
    camera
}

#[cfg(target_arch = "wasm32")]
fn add_body(
    world: &mut World,
    visuals: &mut [ShapeVisual],
    body: BodyDef,
    shape: ShapeDef,
    color: [f32; 4],
) -> Result<(), String> {
    let id = world.create_body(body).map_err(|error| error.to_string())?;
    world
        .create_shape(id, shape)
        .map_err(|error| error.to_string())?;
    visuals[id.slot() as usize] = shape_visual(id, shape, color);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn shape_visual(id: physbox_sim::BodyId, shape: ShapeDef, color: [f32; 4]) -> ShapeVisual {
    let geometry = match shape.geometry {
        ShapeGeometry::Circle(circle) => {
            [circle.radius, circle.radius, circle.radius, circle.radius]
        }
        ShapeGeometry::Rectangle(rectangle) => [
            rectangle.half_extents[0],
            rectangle.half_extents[1],
            0.0,
            0.0,
        ],
    };
    let kind = match shape.geometry {
        ShapeGeometry::Circle(_) => 1,
        ShapeGeometry::Rectangle(_) => 2,
    };
    ShapeVisual {
        geometry,
        color,
        identity: [id.generation(), kind, id.slot(), 0],
    }
}

#[cfg(target_arch = "wasm32")]
const fn palette(index: usize) -> [f32; 4] {
    const COLORS: [[f32; 4]; 6] = [
        [0.21, 0.72, 0.91, 1.0],
        [0.43, 0.84, 0.61, 1.0],
        [0.98, 0.68, 0.26, 1.0],
        [0.89, 0.38, 0.51, 1.0],
        [0.61, 0.50, 0.94, 1.0],
        [0.18, 0.78, 0.74, 1.0],
    ];
    COLORS[index % COLORS.len()]
}

#[cfg(target_arch = "wasm32")]
fn js_error(message: impl Into<String>) -> JsValue {
    JsValue::from_str(&message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_scene_slugs_are_stable_and_strict() {
        assert_eq!(SceneName::parse(PLAYGROUND).unwrap().slug(), PLAYGROUND);
        assert_eq!(
            SceneName::parse(STACK_AND_SLEEP).unwrap().slug(),
            STACK_AND_SLEEP
        );
        assert_eq!(
            SceneName::parse(WATER_AND_RIGID).unwrap().slug(),
            WATER_AND_RIGID
        );
        assert_eq!(
            SceneName::parse(RIGID_STRESS_1K)
                .unwrap()
                .rigid_stress_body_count(),
            Some(1_000)
        );
        assert_eq!(
            SceneName::parse(RIGID_STRESS_2K).unwrap().slug(),
            RIGID_STRESS_2K
        );
        assert_eq!(
            SceneName::parse(RIGID_STRESS_10K)
                .unwrap()
                .rigid_stress_body_count(),
            Some(10_000)
        );
        assert_eq!(
            SceneName::parse(WATER_STRESS_16K)
                .unwrap()
                .water_stress_capacity(),
            Some(16_384)
        );
        for (slug, capacity) in [("tower-drop-13k", 16_384), ("tower-drop-32k", 32_768)] {
            let scene = SceneName::parse(slug).unwrap();
            assert_eq!(scene.slug(), slug);
            assert!(scene.tower_drop());
            let config = ShowcaseConfig::for_scene(scene);
            assert_eq!(config.body_capacity.slots(), 64);
            assert_eq!(config.particle_capacity, Some(capacity));
        }
        assert!(!SceneName::RigidStress10k.authors_water());
        assert!(SceneName::WaterStress16k.authors_water());
        assert!(SceneName::parse("water-stress-32k").is_err());
        assert!(SceneName::parse("liquid-sandbox").is_err());
    }

    #[test]
    fn showcase_configuration_and_water_stress_presets_are_bounded() {
        assert_eq!(ShowcaseConfig::DEFAULT.body_capacity.slots(), 256);
        assert_eq!(ShowcaseConfig::DEFAULT.particle_capacity, Some(4_096));
        assert_eq!(ShowcaseConfig::DEFAULT.physics_hz, 60);
        assert_eq!(
            ShowcaseConfig::for_scene(SceneName::Playground).particle_capacity,
            None
        );
        assert_eq!(
            ShowcaseConfig::for_scene(SceneName::WaterAndRigid),
            ShowcaseConfig::DEFAULT
        );
        assert_eq!(
            ShowcaseConfig::for_scene(SceneName::RigidStress1k)
                .body_capacity
                .slots(),
            1_000
        );
        assert_eq!(
            ShowcaseConfig::for_scene(SceneName::RigidStress10k)
                .body_capacity
                .slots(),
            10_000
        );
        assert_eq!(
            ShowcaseConfig::for_scene(SceneName::WaterStress16k).particle_capacity,
            Some(16_384)
        );
        assert_eq!(
            ShowcaseConfig::parse(10_000, 32_768, 30).unwrap(),
            ShowcaseConfig {
                body_capacity: WorldCapacity::Slots10000,
                particle_capacity: Some(32_768),
                physics_hz: 30,
            }
        );
        assert_eq!(
            ShowcaseConfig::parse(256, 0, 60).unwrap().particle_capacity,
            None
        );
        assert!(ShowcaseConfig::parse(128, 8_192, 60).is_err());
        assert!(ShowcaseConfig::parse(256, 1_024, 60).is_err());
        assert!(ShowcaseConfig::parse(256, 8_192, 120).is_err());
        assert!(ShowcaseConfig::parse(256, 8_192, 90).is_err());
        assert!(ShowcaseConfig::parse(20_000, 8_192, 30).is_err());
        for capacity in [4_096, 8_192, 16_384, 32_768] {
            let (lower, upper, count) = water_stress_region(capacity);
            assert!(count <= capacity * 4 / 5);
            assert!(count >= capacity * 79 / 100);
            assert!(lower[0] >= -16.0 && lower[1] >= -10.0);
            assert!(upper[0] <= 16.0 && upper[1] <= 24.0);
        }
    }

    #[test]
    fn rigid_stress_variants_are_exact_bounded_and_spatially_inspectable() {
        for target in [1_000, 2_000, 5_000, 10_000] {
            let layout = rigid_stress_layout(target);
            assert_eq!(layout.target_bodies, target);
            assert_eq!(3 + layout.stack_count + layout.dynamic_bodies, target);
            assert!(layout.dynamic_bodies >= target * 4 / 5);
            assert!(layout.dynamic_bodies <= layout.stack_count * 5);
            assert!(layout.columns * layout.rows >= layout.stack_count);
            assert!(layout.upper[0] - layout.lower[0] >= 3.59 * layout.columns as f32);
            assert!(layout.upper[1] - layout.lower[1] >= 3.59 * layout.rows as f32);
        }
    }

    #[test]
    fn camera_aspect_fit_preserves_world_units_and_authored_frame() {
        let authored = [0.0, 0.0, 8.0, 5.0];
        assert_eq!(fitted_camera(authored, [1_600, 1_000]), authored);

        for size in [[1_600, 900], [900, 1_600], [1, 1], [2_560, 1_600]] {
            let fitted = fitted_camera(authored, size);
            assert!(fitted[2] >= authored[2]);
            assert!(fitted[3] >= authored[3]);
            let pixels_per_world_x = size[0] as f32 / (2.0 * fitted[2]);
            let pixels_per_world_y = size[1] as f32 / (2.0 * fitted[3]);
            assert!((pixels_per_world_x - pixels_per_world_y).abs() < 0.000_1);
        }
    }
}
