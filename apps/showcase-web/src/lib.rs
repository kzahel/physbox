#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use std::mem::size_of;

#[cfg(target_arch = "wasm32")]
use bytemuck::{Pod, Zeroable};
#[cfg(target_arch = "wasm32")]
use physbox_sim::{
    BodyDef, CommandResultRequest, GpuBodyObservation, InteractiveClockAction,
    InteractiveClockInput, PointConstraintDef, PointConstraintId, PointQueryDef, ShapeDef,
    ShapeGeometry, SimulationClock, StepDef, World, WorldCommandKind, WorldCommandStatus, WorldDef,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SceneName {
    Playground,
    StackAndSleep,
}

#[cfg(any(target_arch = "wasm32", test))]
impl SceneName {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            PLAYGROUND => Ok(Self::Playground),
            STACK_AND_SLEEP => Ok(Self::StackAndSleep),
            _ => Err(format!("unknown showcase scene {value:?}")),
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::Playground => PLAYGROUND,
            Self::StackAndSleep => STACK_AND_SLEEP,
        }
    }
}

#[cfg(target_arch = "wasm32")]
const BODY_CAPACITY: usize = 64;

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
}

#[cfg(target_arch = "wasm32")]
struct Scene {
    name: SceneName,
    world: World,
    observation: GpuBodyObservation,
    renderer: BodyRenderer,
    clock: SimulationClock,
    camera: [f32; 4],
    frame_count: u64,
    last_steps: u32,
    last_timestamp_ms: Option<f64>,
    paused: bool,
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
    ) -> Result<Self, String> {
        let (mut world, visuals, camera) = build_scene(device, name)?;
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
        queue.submit([encoder.finish()]);
        let renderer = BodyRenderer::new(device, format, &observation, &visuals);
        Ok(Self {
            name,
            world,
            observation,
            renderer,
            clock: SimulationClock::default(),
            camera,
            frame_count: 0,
            last_steps: 1,
            last_timestamp_ms: None,
            paused: false,
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

    fn pointer(&mut self, phase: u8, normalized_x: f32, normalized_y: f32) {
        self.pointer_target = [
            self.camera[0] + (normalized_x.clamp(0.0, 1.0) * 2.0 - 1.0) * self.camera[2],
            self.camera[1] + (1.0 - normalized_y.clamp(0.0, 1.0) * 2.0) * self.camera[3],
        ];
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

    fn reconcile_results(&mut self, device: &wgpu::Device) -> Result<(), String> {
        let Some(request) = self.pending_results.as_mut() else {
            return Ok(());
        };
        let _ = device.poll(wgpu::PollType::Poll);
        let Some(results) = self.world.try_take_command_results(request) else {
            return Ok(());
        };
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
        } else if self.pointer_dirty {
            if let Some(constraint) = self.constraint {
                self.world
                    .queue_set_point_constraint_target(constraint, self.pointer_target)
                    .map_err(|error| error.to_string())?;
                self.pointer_dirty = false;
                self.commands_queued = true;
            }
        }
        Ok(())
    }

    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: [u32; 2],
        timestamp_ms: f64,
    ) -> Result<(), String> {
        self.reconcile_results(device)?;
        self.queue_pointer_action()?;
        let elapsed_seconds = self
            .last_timestamp_ms
            .map_or(1.0 / 60.0, |last| (timestamp_ms - last).max(0.0) / 1000.0);
        self.last_timestamp_ms = Some(timestamp_ms);
        let (steps, _) = self.clock.admit(InteractiveClockInput {
            elapsed_seconds,
            requested_time_scale: if self.paused { 0.0 } else { 1.0 },
            action: InteractiveClockAction::Run,
        });
        self.last_steps = steps;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox_showcase_frame"),
        });
        let mut encoded_commands = false;
        if steps == 1 {
            let report = self
                .world
                .step(
                    device,
                    queue,
                    &mut encoder,
                    StepDef::fixed(self.clock.fixed_step_seconds() as f32),
                )
                .map_err(|error| error.to_string())?;
            encoded_commands = report.queued_operations > 0;
        }
        self.observation.encode(&mut encoder);
        let mut result_request = if encoded_commands {
            Some(
                self.world
                    .request_command_results(device, &mut encoder)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        self.renderer
            .encode(queue, &mut encoder, target, size, self.camera);
        queue.submit([encoder.finish()]);
        if let Some(request) = result_request.as_mut() {
            request.begin_map().map_err(|error| error.to_string())?;
        }
        if result_request.is_some() {
            self.pending_results = result_request;
        }
        self.frame_count = self.frame_count.saturating_add(1);
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
struct BodyRenderer {
    params: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    capacity: u32,
}

#[cfg(target_arch = "wasm32")]
impl BodyRenderer {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        observation: &GpuBodyObservation,
        visuals: &[ShapeVisual],
    ) -> Self {
        let shape_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox_showcase_shape_visuals"),
            contents: bytemuck::cast_slice(visuals),
            usage: wgpu::BufferUsages::STORAGE,
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
        Self {
            params,
            bind_group,
            pipeline,
            capacity: observation.capacity(),
        }
    }

    fn encode(
        &self,
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
            }),
        );
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..6, 0..self.capacity);
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
        self.scene =
            Scene::new(&self.device, &self.queue, self.view_format, name).map_err(js_error)?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), JsValue> {
        self.scene = Scene::new(&self.device, &self.queue, self.view_format, self.scene.name)
            .map_err(js_error)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = setPaused)]
    pub fn set_paused(&mut self, paused: bool) {
        self.scene.paused = paused;
    }

    #[wasm_bindgen(js_name = pointerEvent)]
    pub fn pointer_event(&mut self, phase: u8, x: f32, y: f32) {
        self.scene.pointer(phase, x, y);
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
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("physbox_showcase_device"),
            required_features: wgpu::Features::empty(),
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
    let scene = Scene::new(&device, &queue, view_format, scene_name).map_err(js_error)?;
    Ok(ShowcaseApp {
        _instance: instance,
        surface,
        device,
        queue,
        surface_config,
        view_format,
        scene,
    })
}

#[cfg(target_arch = "wasm32")]
fn build_scene(
    device: &wgpu::Device,
    name: SceneName,
) -> Result<(World, Vec<ShapeVisual>, [f32; 4]), String> {
    let mut world = World::new(device, WorldDef::default()).map_err(|error| error.to_string())?;
    let mut visuals = vec![ShapeVisual::zeroed(); BODY_CAPACITY];
    add_body(
        &mut world,
        &mut visuals,
        BodyDef::fixed().with_position([0.0, -4.45]),
        ShapeDef::rectangle([7.5, 0.35])
            .with_density(0.0)
            .with_friction(0.7)
            .with_restitution(0.0),
        [0.17, 0.23, 0.32, 1.0],
    )?;
    add_body(
        &mut world,
        &mut visuals,
        BodyDef::fixed().with_position([-7.15, 0.0]),
        ShapeDef::rectangle([0.2, 4.2])
            .with_density(0.0)
            .with_friction(0.7)
            .with_restitution(0.0),
        [0.12, 0.18, 0.27, 1.0],
    )?;
    add_body(
        &mut world,
        &mut visuals,
        BodyDef::fixed().with_position([7.15, 0.0]),
        ShapeDef::rectangle([0.2, 4.2])
            .with_density(0.0)
            .with_friction(0.7)
            .with_restitution(0.0),
        [0.12, 0.18, 0.27, 1.0],
    )?;
    match name {
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
        }
        SceneName::StackAndSleep => {
            for index in 0..8 {
                add_body(
                    &mut world,
                    &mut visuals,
                    BodyDef::dynamic().with_position([0.0, -3.55 + index as f32 * 0.86]),
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
                    BodyDef::dynamic().with_position([x, -3.55]),
                    ShapeDef::circle(0.42)
                        .with_density(0.8)
                        .with_friction(0.5)
                        .with_restitution(0.05),
                    palette(index + 8),
                )?;
            }
        }
    }
    Ok((world, visuals, [0.0, 0.0, 8.0, 5.0]))
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
    visuals[id.slot() as usize] = ShapeVisual {
        geometry,
        color,
        identity: [id.generation(), kind, id.slot(), 0],
    };
    Ok(())
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
        assert!(SceneName::parse("liquid-sandbox").is_err());
    }
}
