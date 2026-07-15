#[cfg(not(target_arch = "wasm32"))]
use crate::gpu_context::create_instance;
use crate::{
    AdapterDiagnostics, EngineConfig, FrameInput, FrameReport,
    gpu_context::{request_adapter, request_device, surface_format},
    host::EngineHost,
};

#[derive(Clone, Copy)]
pub(crate) struct RenderFrameTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub size: [u32; 2],
}

pub(crate) struct RenderFrameContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub target: RenderFrameTarget<'a>,
}

pub struct SurfaceRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    host: EngineHost,
    diagnostics: AdapterDiagnostics,
}

impl SurfaceRenderer {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        engine_config: EngineConfig,
    ) -> Result<Self, String> {
        let adapter = request_adapter(instance, Some(&surface)).await?;
        let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let (device, queue) = request_device(&adapter).await?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = surface_format(&capabilities.formats)
            .ok_or_else(|| "WebGPU surface reported no texture formats".to_owned())?;
        let view_format = format.add_srgb_suffix();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
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
        let host = EngineHost::new(
            &device,
            &queue,
            view_format,
            engine_config,
            diagnostics.clone(),
        );
        Ok(Self {
            surface,
            device,
            queue,
            surface_config,
            view_format,
            host,
            diagnostics,
        })
    }

    pub fn diagnostics(&self) -> &AdapterDiagnostics {
        &self.diagnostics
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn render(&mut self, input: FrameInput) -> Result<FrameReport, String> {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                self.surface.get_current_texture().map_err(|error| {
                    format!("failed to acquire WebGPU surface after recovery: {error}")
                })?
            }
            Err(error) => return Err(format!("failed to acquire WebGPU surface: {error}")),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("physbox5_surface_view"),
            format: Some(self.view_format),
            ..Default::default()
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_surface_encoder"),
            });
        let report = self.host.render_frame(
            input,
            RenderFrameContext {
                device: &self.device,
                queue: &self.queue,
                encoder: &mut encoder,
                target: RenderFrameTarget {
                    color_view: &view,
                    size: [self.surface_config.width, self.surface_config.height],
                },
            },
        );
        self.queue.submit([encoder.finish()]);
        frame.present();
        Ok(report)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct OffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    host: EngineHost,
    diagnostics: AdapterDiagnostics,
    width: u32,
    height: u32,
}

#[cfg(not(target_arch = "wasm32"))]
impl OffscreenRenderer {
    pub async fn new(width: u32, height: u32, engine_config: EngineConfig) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("offscreen dimensions must be non-zero".to_owned());
        }
        let instance = create_instance();
        let adapter = request_adapter(&instance, None).await?;
        let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let (device, queue) = request_device(&adapter).await?;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("physbox5_offscreen_texture"),
            size: wgpu::Extent3d {
                width,
                height,
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
        let host = EngineHost::new(&device, &queue, format, engine_config, diagnostics.clone());
        Ok(Self {
            device,
            queue,
            texture,
            view,
            host,
            diagnostics,
            width,
            height,
        })
    }

    pub fn diagnostics(&self) -> &AdapterDiagnostics {
        &self.diagnostics
    }

    pub fn render(&mut self, input: FrameInput) -> FrameReport {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_offscreen_encoder"),
            });
        let report = self.host.render_frame(
            input,
            RenderFrameContext {
                device: &self.device,
                queue: &self.queue,
                encoder: &mut encoder,
                target: RenderFrameTarget {
                    color_view: &self.view,
                    size: [self.width, self.height],
                },
            },
        );
        self.queue.submit([encoder.finish()]);
        report
    }

    pub fn read_pixels(&self) -> Result<Vec<u8>, String> {
        read_texture(
            &self.device,
            &self.queue,
            &self.texture,
            self.width,
            self.height,
        )
    }

    pub fn read_capture_state(&self) -> Result<[f32; 8], String> {
        read_state(&self.device, &self.queue, &self.host)
    }

    pub fn read_capture_identity(&self) -> Result<[u32; 4], String> {
        read_identity(&self.device, &self.queue, &self.host)
    }

    pub fn read_fixed_floor_pass_evidence(
        &self,
    ) -> Result<crate::GpuFixedFloorPassEvidence, String> {
        let layout = self.host.floor_pass_evidence_layout()?;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_floor_pass_evidence_readback"),
            size: layout.total_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_floor_pass_evidence_copy"),
            });
        self.host.copy_floor_pass_evidence(&mut encoder, &buffer)?;
        self.queue.submit([encoder.finish()]);
        let mapped = map_buffer(&self.device, &buffer)?;
        let result = crate::body_floor_solver::decode_pass_evidence(&mapped, layout);
        drop(mapped);
        buffer.unmap();
        result
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    const ALIGNMENT: u32 = 256;
    const BYTES_PER_PIXEL: u32 = 4;
    let row_bytes = width * BYTES_PER_PIXEL;
    let padded_row_bytes = row_bytes.div_ceil(ALIGNMENT) * ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_capture_readback"),
        size: u64::from(padded_row_bytes) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_capture_copy"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: Default::default(),
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let mut pixels = Vec::with_capacity((row_bytes * height) as usize);
    for row in 0..height {
        let start = (row * padded_row_bytes) as usize;
        pixels.extend_from_slice(&mapped[start..start + row_bytes as usize]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(pixels)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_state(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    host: &EngineHost,
) -> Result<[f32; 8], String> {
    let size = (8 * std::mem::size_of::<f32>()) as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_state_readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_state_copy"),
    });
    host.copy_capture_state(&mut encoder, &buffer);
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let values = bytemuck::cast_slice::<u8, f32>(&mapped);
    let result: [f32; 8] = values
        .try_into()
        .map_err(|_| "bootstrap state readback had the wrong size".to_owned())?;
    drop(mapped);
    buffer.unmap();
    Ok(result)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_identity(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    host: &EngineHost,
) -> Result<[u32; 4], String> {
    let size = std::mem::size_of::<[u32; 4]>() as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_identity_readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_identity_copy"),
    });
    host.copy_capture_identity(&mut encoder, &buffer)?;
    queue.submit([encoder.finish()]);
    let mapped = map_buffer(device, &buffer)?;
    let values = bytemuck::cast_slice::<u8, u32>(&mapped);
    let result: [u32; 4] = values
        .try_into()
        .map_err(|_| "body identity readback had the wrong size".to_owned())?;
    drop(mapped);
    buffer.unmap();
    Ok(result)
}

#[cfg(not(target_arch = "wasm32"))]
fn map_buffer<'a>(
    device: &wgpu::Device,
    buffer: &'a wgpu::Buffer,
) -> Result<wgpu::BufferView<'a>, String> {
    use std::sync::mpsc;

    let slice = buffer.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("GPU readback poll failed: {error}"))?;
    receiver
        .recv()
        .map_err(|error| format!("GPU readback channel failed: {error}"))?
        .map_err(|error| format!("GPU readback mapping failed: {error}"))?;
    Ok(slice.get_mapped_range())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use physbox_launch::ValidationScenario;

    use super::*;

    #[test]
    fn exact_free_flight_matches_f32_semi_implicit_integration() {
        let scenario = crate::scenario::p5_1_direct_render_scenario(3).unwrap();
        let body = &scenario.bodies[0];
        let dt = scenario.settings.fixed_dt.get();
        let mut expected = [
            body.position.x,
            body.position.y,
            body.angle.get(),
            0.0,
            body.linear_velocity.x,
            body.linear_velocity.y,
            body.angular_velocity.get(),
            0.0,
        ];
        for _ in 0..3 {
            expected[4] += scenario.settings.gravity.x * dt;
            expected[5] += scenario.settings.gravity.y * dt;
            expected[0] += expected[4] * dt;
            expected[1] += expected[5] * dt;
            expected[2] += expected[6] * dt;
        }

        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::BodyFreeFlight,
            },
        ))
        .unwrap();
        let initial = renderer.read_capture_state().unwrap();
        let initial_identity = renderer.read_capture_identity().unwrap();
        assert_eq!(initial_identity[0], 1);
        assert_ne!(initial_identity[1] & 1, 0);
        let zero_report = renderer.render(FrameInput::exact(0));
        assert_eq!(zero_report.fixed_steps_this_frame, 0);
        assert_eq!(renderer.read_capture_state().unwrap(), initial);

        let report = renderer.render(FrameInput::exact(3));
        assert!(report.scenario.complete);
        assert!(report.scenario.passed);
        let actual = renderer.read_capture_state().unwrap();
        assert_eq!(renderer.read_capture_identity().unwrap(), initial_identity);
        for (index, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
            assert!(
                (actual - expected).abs() <= 1.0e-5,
                "capture value {index} differed: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn exact_generated_floor_landing_is_finite_and_deterministic() {
        let config = EngineConfig {
            scenario: ValidationScenario::RigidFloor,
        };
        let run = || {
            let mut renderer = pollster::block_on(OffscreenRenderer::new(320, 200, config.clone()))
                .expect("generated-contact renderer initializes");
            let report = renderer.render(FrameInput::exact(120));
            assert!(report.scenario.complete);
            assert_eq!(report.scene_contract.contact_point_count, 2);
            assert_eq!(report.scene_contract.solver_dispatches_per_step, 282);
            assert_eq!(report.scene_contract.solver_capacity_bytes, 23_700);
            assert!(!report.scene_contract.capacity_overflowed);
            renderer.read_capture_state().unwrap()
        };
        let first = run();
        let replay = run();
        assert_eq!(first, replay);
        assert!(first.into_iter().all(f32::is_finite));
        assert!((first[1] - 0.2).abs() <= 1.0e-5);
    }

    #[test]
    fn scripted_reset_grab_holds_releases_and_replays_after_reset() {
        let mut scripted = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::ResetGrab,
            },
        ))
        .unwrap();
        scripted.render(FrameInput::exact(11));
        let begin = scripted.read_capture_state().unwrap();
        assert_eq!([begin[0], begin[1]], [0.0, 0.65]);
        scripted.render(FrameInput::exact(10));
        let first_target = scripted.read_capture_state().unwrap();
        assert_eq!([first_target[0], first_target[1]], [0.45, 0.75]);
        scripted.render(FrameInput::exact(10));
        let second_target = scripted.read_capture_state().unwrap();
        assert_eq!([second_target[0], second_target[1]], [-0.4, 0.55]);
        scripted.render(FrameInput::exact(10));
        let released = scripted.read_capture_state().unwrap();
        assert!(released[1] < 0.55);
        let report = scripted.render(FrameInput::exact(109));
        assert!(report.scenario.complete);
        assert!(report.scenario.passed);
        assert_eq!(report.body_commands.executed_commands_total, 6);
        assert_eq!(report.body_commands.reset_commands_total, 2);
        assert!(!report.body_commands.grab_active);
        let replayed = scripted.read_capture_state().unwrap();

        let mut fresh = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
            },
        ))
        .unwrap();
        fresh.render(FrameInput::exact(70));
        assert_eq!(replayed, fresh.read_capture_state().unwrap());
    }

    #[test]
    fn live_pointer_targets_coalesce_until_an_admitted_step() {
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            640,
            400,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
            },
        ))
        .unwrap();
        let pointer = |phase, normalized_position| {
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 17,
                phase,
                button: crate::PointerButton::Primary,
                normalized_position,
            })
        };
        let initial = renderer.read_capture_state().unwrap();
        let initial_screen = crate::Camera2d::fixed()
            .resolve([640, 400])
            .unwrap()
            .world_to_screen([initial[0], initial[1]]);
        let queued = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 0 },
            input_events: vec![pointer(crate::PointerPhase::Down, initial_screen)],
        });
        assert_eq!(renderer.read_capture_state().unwrap(), initial);
        assert_eq!(queued.body_commands.queued_commands_total, 1);
        assert_eq!(queued.body_commands.executed_commands_total, 0);

        renderer.render(FrameInput::exact(1));
        let held = renderer.read_capture_state().unwrap();
        assert_eq!([held[0], held[1]], [initial[0], initial[1]]);
        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(crate::PointerPhase::Move, [0.75, 0.25])],
        });
        let first_target = renderer.read_capture_state().unwrap();
        assert_eq!([first_target[0], first_target[1]], [0.8, 0.5]);
        let coalesced = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 0 },
            input_events: vec![
                pointer(crate::PointerPhase::Move, [0.25, 0.4]),
                pointer(crate::PointerPhase::Move, [0.5, 0.3]),
            ],
        });
        assert_eq!(renderer.read_capture_state().unwrap(), first_target);
        assert_eq!(coalesced.body_commands.coalesced_targets_total, 1);
        renderer.render(FrameInput::exact(1));
        let moved = renderer.read_capture_state().unwrap();
        assert_eq!([moved[0], moved[1]], [0.0, 0.39999998]);

        let release_report = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(crate::PointerPhase::Up, [0.5, 0.3])],
        });
        let released = renderer.read_capture_state().unwrap();
        assert!(released[1] < moved[1]);
        let stale_move = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(crate::PointerPhase::Move, [1.0, 0.0])],
        });
        let after_stale = renderer.read_capture_state().unwrap();
        assert_eq!(
            stale_move.body_commands.queued_commands_total,
            release_report.body_commands.queued_commands_total
        );
        assert_eq!(after_stale[0], released[0]);
        assert!(after_stale[1] < released[1]);
    }

    #[test]
    fn empty_touch_is_inert_and_secondary_drag_changes_only_camera() {
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
            },
        ))
        .unwrap();
        let touch = |phase, at| {
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 9,
                phase,
                button: crate::PointerButton::Contact,
                normalized_position: at,
            })
        };
        let empty_report = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![
                touch(crate::PointerPhase::Down, [0.02, 0.02]),
                touch(crate::PointerPhase::Move, [0.2, 0.2]),
                touch(crate::PointerPhase::Up, [0.2, 0.2]),
            ],
        });
        assert_eq!(empty_report.camera.center, [0.0, 0.0]);
        assert_eq!(empty_report.camera.pan_changes_total, 0);
        assert_eq!(empty_report.camera.zoom_changes_total, 0);
        let after_empty = renderer.read_capture_state().unwrap();
        let mut control = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
            },
        ))
        .unwrap();
        control.render(FrameInput::exact(1));
        assert_eq!(after_empty, control.read_capture_state().unwrap());

        let before_pan = after_empty;
        let report = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 0 },
            input_events: vec![
                crate::InputEvent::Pointer(crate::PointerEvent {
                    id: 1,
                    phase: crate::PointerPhase::Down,
                    button: crate::PointerButton::Secondary,
                    normalized_position: [0.5, 0.5],
                }),
                crate::InputEvent::Pointer(crate::PointerEvent {
                    id: 1,
                    phase: crate::PointerPhase::Move,
                    button: crate::PointerButton::Primary,
                    normalized_position: [0.6, 0.4],
                }),
                crate::InputEvent::Pointer(crate::PointerEvent {
                    id: 1,
                    phase: crate::PointerPhase::Up,
                    button: crate::PointerButton::Secondary,
                    normalized_position: [0.6, 0.4],
                }),
            ],
        });
        assert_eq!(renderer.read_capture_state().unwrap(), before_pan);
        assert_eq!(report.fixed_steps_this_frame, 0);
        assert_eq!(report.camera.pan_changes_total, 1);
        assert!((report.camera.center[0] + 0.32).abs() <= 1.0e-5);
        assert!((report.camera.center[1] + 0.2).abs() <= 1.0e-5);
        assert_eq!(
            report.scene_contract.scenario_hash,
            renderer
                .render(FrameInput::exact(0))
                .scene_contract
                .scenario_hash
        );
    }

    #[test]
    fn shared_ui_switches_and_restarts_scenes_without_stale_state() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig {
                scenario: ValidationScenario::Bootstrap,
            },
        ))
        .unwrap();
        let click = |id, at| {
            [crate::PointerPhase::Down, crate::PointerPhase::Up]
                .into_iter()
                .map(|phase| {
                    crate::InputEvent::Pointer(crate::PointerEvent {
                        id,
                        phase,
                        button: crate::PointerButton::Primary,
                        normalized_position: at,
                    })
                })
                .collect::<Vec<_>>()
        };
        let layout = crate::ui::UiLayout::for_target(target);
        let initial = renderer.render(FrameInput::exact(1));
        let unchanged = renderer.render(FrameInput::exact(0));
        assert_eq!(
            initial.ui.static_writes_total,
            unchanged.ui.static_writes_total
        );
        assert_eq!(
            unchanged.ui.dynamic_writes_total,
            initial.ui.dynamic_writes_total + 1
        );
        assert_eq!(
            unchanged.ui.parameter_writes_total,
            initial.ui.parameter_writes_total
        );
        assert_eq!(initial.ui.allocated_bytes, 31_760);

        renderer.render(FrameInput::exact(0).with_input_events(vec![
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 20,
                phase: crate::PointerPhase::Down,
                button: crate::PointerButton::Secondary,
                normalized_position: [0.5, 0.5],
            }),
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 20,
                phase: crate::PointerPhase::Move,
                button: crate::PointerButton::Primary,
                normalized_position: [0.6, 0.4],
            }),
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 20,
                phase: crate::PointerPhase::Up,
                button: crate::PointerButton::Secondary,
                normalized_position: [0.6, 0.4],
            }),
        ]));

        let opened = renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("menu", 0))),
        );
        assert!(opened.ui.menu_open);
        assert_eq!(opened.body_commands.queued_commands_total, 0);
        assert_eq!(
            opened.ui.static_writes_total,
            initial.ui.static_writes_total + 1
        );

        let recentered = renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(21, layout.center_normalized("recenter", 0))),
        );
        assert_eq!(recentered.camera.center, [0.0, 0.0]);
        assert_eq!(recentered.camera.recenter_changes_total, 1);
        assert_eq!(recentered.fixed_steps_this_frame, 0);
        renderer.render(
            FrameInput::exact(0).with_input_events(click(22, layout.center_normalized("menu", 0))),
        );

        let switched = renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(2, layout.center_normalized("scenario", 2))),
        );
        assert_eq!(switched.scene_contract.scenario, "rigid-stack");
        assert_eq!(switched.fixed_steps_total, 0);
        assert_eq!(switched.camera.center, [0.0, 0.0]);
        assert_eq!(switched.ui.current_scene, "rigid-stack");
        assert_eq!(switched.ui.scenario_activations_total, 1);
        assert_eq!(
            switched.body_commands,
            crate::BodyCommandReport {
                capacity_commands: crate::BODY_COMMAND_CAPACITY,
                ..Default::default()
            }
        );

        renderer.render(FrameInput::exact(3));
        renderer.render(
            FrameInput::exact(0).with_input_events(click(3, layout.center_normalized("menu", 0))),
        );
        let restarted = renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(4, layout.center_normalized("restart", 0))),
        );
        assert_eq!(restarted.scene_contract.scenario, "rigid-stack");
        assert_eq!(restarted.fixed_steps_total, 0);
        assert_eq!(restarted.ui.scenario_activations_total, 1);
        assert_eq!(restarted.ui.scenario_restarts_total, 1);
        assert!(!restarted.ui.overflowed);
    }

    #[test]
    fn command_capacity_overflow_is_reported_without_partial_execution() {
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            640,
            400,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
            },
        ))
        .unwrap();
        let target = crate::Camera2d::fixed()
            .resolve([640, 400])
            .unwrap()
            .world_to_screen([0.0, 0.65]);
        let mut events = Vec::new();
        for index in 0..crate::BODY_COMMAND_CAPACITY / 2 {
            for phase in [crate::PointerPhase::Down, crate::PointerPhase::Up] {
                events.push(crate::InputEvent::Pointer(crate::PointerEvent {
                    id: u64::from(index),
                    phase,
                    button: crate::PointerButton::Primary,
                    normalized_position: target,
                }));
            }
        }
        events.push(crate::InputEvent::Pointer(crate::PointerEvent {
            id: u64::from(crate::BODY_COMMAND_CAPACITY),
            phase: crate::PointerPhase::Down,
            button: crate::PointerButton::Primary,
            normalized_position: target,
        }));
        let queued = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 0 },
            input_events: events,
        });
        assert!(queued.body_commands.overflowed);
        assert_eq!(queued.body_commands.queued_commands_total, 65);
        let stepped = renderer.render(FrameInput::exact(1));
        assert!(stepped.scene_contract.capacity_overflowed);
        assert_eq!(stepped.body_commands.executed_commands_total, 0);
    }
}
