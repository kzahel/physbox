use bytemuck::{Pod, Zeroable};
use physbox_box2d_comparator::{BOX2D_REVISION, BodySnapshot, Box2dWorld, GrabTuning};
use physbox_contracts::{
    ANALYTIC_SHAPE_FLAG_CIRCLE, ANALYTIC_SHAPE_FLAG_RECTANGLE, BodyCommandV1, BodyId, BodyStateV1,
    ConstructionIntentV1, FiniteF32, GrabIntentV1, MaterialV1, NeutralActionV1,
    SLEEP_STATE_LAYOUT_VERSION, ScenarioV1, ShapeV1, Vec2,
};
use wgpu::util::DeviceExt;

use crate::{
    BodySlotRegistry, CameraFrame2d,
    construction::{
        CONSTRUCTION_CIRCLE_RADIUS, CONSTRUCTION_FRICTION, CONSTRUCTION_RECTANGLE_HALF_EXTENTS,
        CONSTRUCTION_RESTITUTION,
    },
    grab::{GRAB_DAMPING_RATE, GRAB_MAX_ACCELERATION, GRAB_SPRING_ACCELERATION},
    renderer::RenderFrameTarget,
};

const BODY_FLAG_LIVE: u32 = 1 << 0;
const BODY_FLAG_FIXED: u32 = 1 << 2;
const SLEEP_FLAG_AWAKE: u32 = 1 << 0;
const OBSERVED_WAKE_TRANSITION: u32 = 1 << 6;
const ACTIVE_BODY_HEADER_WORDS: usize = 20;

pub(crate) struct ComparisonShadow {
    world: Box2dWorld,
    renderer: ComparisonRenderer,
    actions: Vec<NeutralActionV1>,
    fixed_dt: f32,
    substeps: u32,
    completed_steps: u64,
    live_bodies: u32,
    slots: BodySlotRegistry,
    active_grab: Option<BodyId>,
    failed: Option<String>,
}

impl ComparisonShadow {
    pub(crate) fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &ScenarioV1,
        capacity: u32,
        sleeping_enabled: bool,
    ) -> Result<Self, String> {
        let world = Box2dWorld::new(scenario, capacity, sleeping_enabled)?;
        let snapshots = world.snapshots()?;
        let mut slots = BodySlotRegistry::with_capacity(capacity)?;
        for body in &scenario.bodies {
            slots.reserve(body.id)?;
        }
        let mut renderer = ComparisonRenderer::new(device, color_format, capacity);
        renderer.update_snapshots(&snapshots, 0);
        Ok(Self {
            world,
            renderer,
            actions: scenario.actions.clone(),
            fixed_dt: scenario.settings.fixed_dt.get(),
            substeps: scenario.settings.substeps,
            completed_steps: 0,
            live_bodies: snapshots.len() as u32,
            slots,
            active_grab: None,
            failed: None,
        })
    }

    pub(crate) fn advance(&mut self, steps: u32) {
        if self.failed.is_some() {
            return;
        }
        for _ in 0..steps {
            self.advance_one();
        }
        self.refresh_snapshots();
    }

    pub(crate) fn apply_interactions(
        &mut self,
        commands: &[BodyCommandV1],
        grab_intents: &[GrabIntentV1],
        construction_intents: &[ConstructionIntentV1],
    ) {
        if self.failed.is_some() {
            return;
        }
        for command in commands {
            if let Err(error) = self.apply_body_command(command) {
                self.failed = Some(error);
                return;
            }
        }
        for intent in construction_intents {
            if let Err(error) = self.apply_construction_intent(*intent) {
                self.failed = Some(error);
                return;
            }
        }
        for intent in grab_intents {
            if let Err(error) = self.apply_grab_intent(*intent) {
                self.failed = Some(error);
                return;
            }
        }
    }

    pub(crate) fn advance_one(&mut self) {
        if self.failed.is_some() {
            return;
        }
        let step = self.completed_steps as u32;
        if let Err(error) = self.apply_neutral_actions(step) {
            self.failed = Some(error);
            return;
        }
        if let Err(error) = self.world.step(self.fixed_dt, self.substeps) {
            self.failed = Some(error);
            return;
        }
        self.completed_steps = self.completed_steps.saturating_add(1);
    }

    pub(crate) fn refresh_snapshots(&mut self) {
        if self.failed.is_some() {
            return;
        }
        match self.world.snapshots() {
            Ok(snapshots) => {
                self.live_bodies = snapshots.len() as u32;
                self.renderer
                    .update_snapshots(&snapshots, self.completed_steps as u32);
            }
            Err(error) => self.failed = Some(error),
        }
    }

    fn apply_body_command(&mut self, command: &BodyCommandV1) -> Result<(), String> {
        match command {
            BodyCommandV1::Reset {
                body,
                position,
                angle,
                linear_velocity,
                angular_velocity,
                ..
            } => {
                self.active_grab = None;
                self.world.reset_body(
                    *body,
                    *position,
                    angle.get(),
                    *linear_velocity,
                    angular_velocity.get(),
                )
            }
            BodyCommandV1::GrabBegin {
                body, world_target, ..
            } => self.begin_grab(*body, *world_target),
            BodyCommandV1::GrabTarget {
                body, world_target, ..
            } => {
                if self.active_grab == Some(*body) {
                    self.world.update_grab(*world_target)?;
                }
                Ok(())
            }
            BodyCommandV1::GrabEnd { body, .. } => {
                if self.active_grab == Some(*body) {
                    self.world.end_grab();
                    self.active_grab = None;
                }
                Ok(())
            }
        }
    }

    fn apply_grab_intent(&mut self, intent: GrabIntentV1) -> Result<(), String> {
        match intent {
            GrabIntentV1::Begin { world_target, .. } => {
                self.world.end_grab();
                self.active_grab = self.world.hit_test(world_target, true)?;
                if let Some(body) = self.active_grab {
                    self.begin_grab(body, world_target)?;
                }
            }
            GrabIntentV1::Target { world_target, .. } => {
                if self.active_grab.is_some() {
                    self.world.update_grab(world_target)?;
                }
            }
            GrabIntentV1::End { .. } => {
                self.world.end_grab();
                self.active_grab = None;
            }
        }
        Ok(())
    }

    fn begin_grab(&mut self, body: BodyId, world_target: Vec2) -> Result<(), String> {
        self.world.begin_grab(
            body,
            world_target,
            world_target,
            GrabTuning {
                spring_acceleration: GRAB_SPRING_ACCELERATION,
                damping_rate: GRAB_DAMPING_RATE,
                max_acceleration: GRAB_MAX_ACCELERATION,
            },
        )?;
        self.active_grab = Some(body);
        Ok(())
    }

    fn apply_construction_intent(&mut self, intent: ConstructionIntentV1) -> Result<(), String> {
        match intent {
            ConstructionIntentV1::PlaceCircle { world_target, .. } => {
                self.place_body(
                    ShapeV1::Circle {
                        radius: finite(CONSTRUCTION_CIRCLE_RADIUS),
                    },
                    world_target,
                    2.0 / (CONSTRUCTION_CIRCLE_RADIUS * CONSTRUCTION_CIRCLE_RADIUS),
                )?;
            }
            ConstructionIntentV1::PlaceRectangle { world_target, .. } => {
                let [x, y] = CONSTRUCTION_RECTANGLE_HALF_EXTENTS;
                self.place_body(
                    ShapeV1::Rectangle {
                        half_extents: Vec2::new(x, y),
                    },
                    world_target,
                    3.0 / (x * x + y * y),
                )?;
            }
            ConstructionIntentV1::EraseAt { world_target, .. } => {
                let Some(body) = self.world.hit_test(world_target, true)? else {
                    return Ok(());
                };
                self.erase_body(body)?;
            }
            ConstructionIntentV1::EraseBody { body, .. } => {
                if let Some(body) = body {
                    self.erase_body(body)?;
                }
            }
        }
        Ok(())
    }

    fn erase_body(&mut self, body: BodyId) -> Result<(), String> {
        if self.active_grab == Some(body) {
            self.world.end_grab();
            self.active_grab = None;
        }
        self.world.remove_body(body)?;
        self.slots.release(body)
    }

    fn place_body(
        &mut self,
        shape: ShapeV1,
        position: Vec2,
        inverse_inertia: f32,
    ) -> Result<(), String> {
        if self
            .world
            .snapshots()?
            .into_iter()
            .any(|body| body.contains(position))
        {
            return Ok(());
        }
        let id = match self.slots.allocate_lowest() {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let body = BodyStateV1 {
            id,
            shape,
            position,
            angle: finite(0.0),
            linear_velocity: Vec2::ZERO,
            angular_velocity: finite(0.0),
            inverse_mass: finite(1.0),
            inverse_inertia: finite(inverse_inertia),
            material: MaterialV1 {
                friction: finite(CONSTRUCTION_FRICTION),
                restitution: finite(CONSTRUCTION_RESTITUTION),
            },
            sleeping: false,
        };
        if let Err(error) = self.world.add_body(&body) {
            self.slots.release(id)?;
            return Err(error);
        }
        Ok(())
    }

    fn apply_neutral_actions(&mut self, step: u32) -> Result<(), String> {
        for action in self.actions.iter().filter(|action| action.step() == step) {
            match action {
                NeutralActionV1::ApplyImpulse {
                    body,
                    impulse,
                    world_point,
                    ..
                } => self.world.apply_impulse(*body, *impulse, *world_point)?,
                NeutralActionV1::SetGravity { gravity, .. } => {
                    self.world.set_gravity(*gravity);
                }
                NeutralActionV1::GrabBegin {
                    body,
                    world_anchor,
                    world_target,
                    tuning,
                    ..
                } => self.world.begin_grab(
                    *body,
                    *world_anchor,
                    *world_target,
                    GrabTuning {
                        spring_acceleration: tuning.spring_acceleration.get(),
                        damping_rate: tuning.damping_rate.get(),
                        max_acceleration: tuning.max_acceleration.get(),
                    },
                )?,
                NeutralActionV1::GrabTarget { world_target, .. } => {
                    self.world.update_grab(*world_target)?;
                }
                NeutralActionV1::GrabEnd { .. } => self.world.end_grab(),
            }
        }
        Ok(())
    }

    pub(crate) fn encode_render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        plan: ComparisonRenderPlan,
    ) {
        self.renderer.encode(queue, encoder, target, plan);
    }

    pub(crate) const fn completed_steps(&self) -> u64 {
        self.completed_steps
    }

    pub(crate) const fn live_bodies(&self) -> u32 {
        self.live_bodies
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    pub(crate) fn hit_test(&self, point: Vec2) -> Result<Option<BodyId>, String> {
        self.world.hit_test(point, true)
    }

    pub(crate) const fn active_grab(&self) -> Option<BodyId> {
        self.active_grab
    }

    pub(crate) const fn revision() -> &'static str {
        BOX2D_REVISION
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RenderParamsRaw {
    viewport: [f32; 4],
    camera: [f32; 4],
    sleep_visualization: [u32; 4],
}

pub(crate) struct ComparisonRenderPlan {
    pub(crate) frame_index: u64,
    pub(crate) camera: CameraFrame2d,
    pub(crate) sleep_visuals_enabled: bool,
    pub(crate) current_outer_step: u32,
    pub(crate) quiet_outer_steps: u32,
    pub(crate) wake_flash_outer_steps: u32,
}

struct ComparisonRenderer {
    capacity: u32,
    live_count: u32,
    poses: wgpu::Buffer,
    properties: wgpu::Buffer,
    metadata: wgpu::Buffer,
    params: wgpu::Buffer,
    active_bodies: wgpu::Buffer,
    sleep_states: wgpu::Buffer,
    _grab_state: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    background_pipeline: wgpu::RenderPipeline,
    body_pipeline: wgpu::RenderPipeline,
    pose_data: Vec<[f32; 4]>,
    property_data: Vec<[f32; 4]>,
    metadata_data: Vec<[u32; 4]>,
    active_data: Vec<u32>,
    sleep_data: Vec<[u32; 8]>,
    previous_awake: Vec<Option<bool>>,
}

impl ComparisonRenderer {
    fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat, capacity: u32) -> Self {
        let pose_data = vec![[0.0; 4]; capacity as usize];
        let property_data = vec![[0.0; 4]; capacity as usize];
        let metadata_data = vec![[0; 4]; capacity as usize];
        let active_data = vec![0; ACTIVE_BODY_HEADER_WORDS + capacity as usize];
        let sleep_data = vec![[0; 8]; capacity as usize];
        let poses = storage_buffer(device, "physbox5_box2d_poses", &pose_data);
        let properties = storage_buffer(device, "physbox5_box2d_properties", &property_data);
        let metadata = storage_buffer(device, "physbox5_box2d_metadata", &metadata_data);
        let active_bodies = storage_buffer(device, "physbox5_box2d_active_bodies", &active_data);
        let sleep_states = storage_buffer(device, "physbox5_box2d_sleep_states", &sleep_data);
        let grab_state = storage_buffer(device, "physbox5_box2d_grab_state", &[[0_u32; 16]]);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_box2d_render_params"),
            size: std::mem::size_of::<RenderParamsRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_box2d_render_layout"),
            entries: &[
                storage_entry(0, wgpu::ShaderStages::VERTEX),
                storage_entry(1, wgpu::ShaderStages::VERTEX),
                storage_entry(2, wgpu::ShaderStages::VERTEX),
                uniform_entry(3, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
                storage_entry(4, wgpu::ShaderStages::VERTEX),
                storage_entry(5, wgpu::ShaderStages::VERTEX),
                storage_entry(6, wgpu::ShaderStages::VERTEX),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_box2d_render_bind_group"),
            layout: &layout,
            entries: &[
                binding(0, &poses),
                binding(1, &properties),
                binding(2, &metadata),
                binding(3, &params),
                binding(4, &active_bodies),
                binding(5, &sleep_states),
                binding(6, &grab_state),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_box2d_render_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_render.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_box2d_render_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let background_pipeline = render_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "physbox5_box2d_background_pipeline",
            "background_vertex",
            "background_fragment",
            None,
        );
        let body_pipeline = render_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            "physbox5_box2d_body_pipeline",
            "body_vertex",
            "body_fragment",
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        Self {
            capacity,
            live_count: 0,
            poses,
            properties,
            metadata,
            params,
            active_bodies,
            sleep_states,
            _grab_state: grab_state,
            bind_group,
            background_pipeline,
            body_pipeline,
            pose_data,
            property_data,
            metadata_data,
            active_data,
            sleep_data,
            previous_awake: vec![None; capacity as usize],
        }
    }

    fn update_snapshots(&mut self, snapshots: &[BodySnapshot], outer_step: u32) {
        self.pose_data.fill([0.0; 4]);
        self.property_data.fill([0.0; 4]);
        self.metadata_data.fill([0; 4]);
        self.active_data.fill(0);
        self.sleep_data.fill([0; 8]);
        self.live_count = snapshots.len() as u32;
        for (active_index, snapshot) in snapshots.iter().enumerate() {
            let slot = snapshot.id.slot as usize;
            assert!(slot < self.capacity as usize);
            let (half_extents, shape_flag) = match snapshot.shape {
                ShapeV1::Circle { radius } => {
                    ([radius.get(), radius.get()], ANALYTIC_SHAPE_FLAG_CIRCLE)
                }
                ShapeV1::Rectangle { half_extents } => (
                    [half_extents.x, half_extents.y],
                    ANALYTIC_SHAPE_FLAG_RECTANGLE,
                ),
            };
            self.pose_data[slot] = [
                snapshot.position.x,
                snapshot.position.y,
                snapshot.angle,
                0.0,
            ];
            self.property_data[slot] = [half_extents[0], half_extents[1], 0.0, 0.0];
            self.metadata_data[slot] = [
                snapshot.id.generation,
                BODY_FLAG_LIVE | shape_flag | if snapshot.fixed { BODY_FLAG_FIXED } else { 0 },
                0,
                0,
            ];
            self.active_data[ACTIVE_BODY_HEADER_WORDS + active_index] = snapshot.id.slot;
            let observed_wake = self.previous_awake[slot] == Some(false) && snapshot.awake;
            self.sleep_data[slot] = [
                snapshot.id.generation,
                if snapshot.awake { SLEEP_FLAG_AWAKE } else { 0 },
                0,
                if observed_wake { outer_step } else { 0 },
                if observed_wake {
                    OBSERVED_WAKE_TRANSITION
                } else {
                    0
                },
                1,
                SLEEP_STATE_LAYOUT_VERSION,
                0,
            ];
            self.previous_awake[slot] = Some(snapshot.awake);
        }
    }

    fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        plan: ComparisonRenderPlan,
    ) {
        queue.write_buffer(&self.poses, 0, bytemuck::cast_slice(&self.pose_data));
        queue.write_buffer(
            &self.properties,
            0,
            bytemuck::cast_slice(&self.property_data),
        );
        queue.write_buffer(&self.metadata, 0, bytemuck::cast_slice(&self.metadata_data));
        queue.write_buffer(
            &self.active_bodies,
            0,
            bytemuck::cast_slice(&self.active_data),
        );
        queue.write_buffer(
            &self.sleep_states,
            0,
            bytemuck::cast_slice(&self.sleep_data),
        );
        let params = RenderParamsRaw {
            viewport: [
                target.size[0] as f32,
                target.size[1] as f32,
                target.size[0] as f32 / target.size[1].max(1) as f32,
                plan.frame_index as f32,
            ],
            camera: [
                plan.camera.center()[0],
                plan.camera.center()[1],
                plan.camera.half_extents()[0],
                plan.camera.half_extents()[1],
            ],
            sleep_visualization: [
                u32::from(plan.sleep_visuals_enabled),
                plan.current_outer_step,
                plan.quiet_outer_steps,
                plan.wake_flash_outer_steps,
            ],
        };
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox5_box2d_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.008,
                        g: 0.015,
                        b: 0.035,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_pipeline(&self.background_pipeline);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.body_pipeline);
        pass.draw(0..6, 0..self.live_count);
    }
}

fn storage_buffer<T: Pod>(device: &wgpu::Device, label: &'static str, data: &[T]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

fn storage_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn binding(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    label: &'static str,
    vertex: &'static str,
    fragment: &'static str,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("comparison construction constants are finite")
}
