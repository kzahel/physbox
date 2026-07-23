#[cfg(not(target_arch = "wasm32"))]
use crate::gpu_context::create_instance;
use crate::{
    AdapterDiagnostics, EngineConfig, FrameInput, FrameReport,
    gpu_context::{request_adapter, request_device, surface_format},
    host::EngineHost,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const ASYNC_INTERACTION_DIAGNOSTIC_CAPACITY: usize = 8;

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
    construction_epoch: Option<(String, u64, u64)>,
    pending_construction_diagnostics: VecDeque<PendingInteractionDiagnostic>,
    pending_sleep_diagnostic: Option<PendingInteractionDiagnostic>,
    #[cfg(feature = "box2d-comparator")]
    pending_grab_diagnostics: VecDeque<PendingInteractionDiagnostic>,
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
            construction_epoch: None,
            pending_construction_diagnostics: VecDeque::new(),
            pending_sleep_diagnostic: None,
            #[cfg(feature = "box2d-comparator")]
            pending_grab_diagnostics: VecDeque::new(),
        })
    }

    pub fn diagnostics(&self) -> &AdapterDiagnostics {
        &self.diagnostics
    }

    pub fn launch_preference_selection(&self) -> physbox_launch::LaunchPreferenceSelection {
        self.host.launch_preference_selection()
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
        self.resolve_interaction_diagnostics()?;
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
        let epoch = (
            report.scene_contract.scenario_hash.clone(),
            report.ui.scenario_activations_total,
            report.ui.scenario_restarts_total,
        );
        if self.construction_epoch.as_ref() != Some(&epoch) {
            self.construction_epoch = Some(epoch.clone());
            self.pending_construction_diagnostics.clear();
            self.pending_sleep_diagnostic = None;
            #[cfg(feature = "box2d-comparator")]
            self.pending_grab_diagnostics.clear();
        }
        let scheduled_construction = if report
            .body_commands
            .construction_intent_dispatches_this_frame
            != 0
        {
            if self.pending_construction_diagnostics.len() == ASYNC_INTERACTION_DIAGNOSTIC_CAPACITY
            {
                #[cfg(feature = "box2d-comparator")]
                self.host.fail_comparison(
                    "asynchronous construction feedback queue exceeded 8 snapshots".to_owned(),
                );
                None
            } else {
                let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("physbox5_async_construction_diagnostic"),
                    size: u64::from(crate::CONSTRUCTION_STATE_BYTES),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                self.host.copy_construction_state(&mut encoder, &buffer)?;
                Some(PendingInteractionDiagnostic::new(
                    buffer,
                    epoch.clone(),
                    report.frame_index,
                ))
            }
        } else {
            None
        };
        let scheduled_sleep = if report.scene_contract.effective_sleeping_enabled
            && self.pending_sleep_diagnostic.is_none()
        {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("physbox5_async_sleep_cycle_diagnostic"),
                size: u64::from(crate::SLEEP_CYCLE_DIAGNOSTIC_BYTES),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.host.copy_sleep_cycle_metrics(&mut encoder, &buffer)?;
            Some(PendingInteractionDiagnostic::new(
                buffer,
                epoch.clone(),
                report.frame_index,
            ))
        } else {
            None
        };
        #[cfg(feature = "box2d-comparator")]
        let scheduled_grab = if report.body_commands.grab_intent_dispatches_this_frame != 0 {
            if self.pending_grab_diagnostics.len() == ASYNC_INTERACTION_DIAGNOSTIC_CAPACITY {
                self.host.fail_comparison(
                    "asynchronous grab feedback queue exceeded 8 snapshots".to_owned(),
                );
                None
            } else {
                let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("physbox5_async_grab_diagnostic"),
                    size: u64::from(crate::grab::GRAB_STATE_BYTES),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                self.host.copy_grab_state(&mut encoder, &buffer)?;
                Some(PendingInteractionDiagnostic::new(
                    buffer,
                    epoch,
                    report.frame_index,
                ))
            }
        } else {
            None
        };
        self.queue.submit([encoder.finish()]);
        if let Some(pending) = scheduled_construction {
            pending.start_mapping();
            self.pending_construction_diagnostics.push_back(pending);
        }
        if let Some(pending) = scheduled_sleep {
            pending.start_mapping();
            self.pending_sleep_diagnostic = Some(pending);
        }
        #[cfg(feature = "box2d-comparator")]
        if let Some(pending) = scheduled_grab {
            pending.start_mapping();
            self.pending_grab_diagnostics.push_back(pending);
        }
        frame.present();
        Ok(report)
    }

    fn resolve_interaction_diagnostics(&mut self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Poll)
            .map_err(|error| format!("poll interaction diagnostics: {error}"))?;
        while self
            .pending_construction_diagnostics
            .front()
            .is_some_and(PendingInteractionDiagnostic::is_ready)
        {
            let pending = self
                .pending_construction_diagnostics
                .pop_front()
                .expect("ready construction diagnostic exists");
            let epoch = pending.epoch.clone();
            let observed_frame_index = pending.observed_frame_index;
            let bytes = pending.finish()?;
            if self.host.construction_diagnostic_epoch() == epoch {
                self.host.publish_construction_diagnostic(
                    crate::construction_state_from_bytes(&bytes)?,
                    observed_frame_index,
                );
            }
        }
        if self
            .pending_sleep_diagnostic
            .as_ref()
            .is_some_and(PendingInteractionDiagnostic::is_ready)
        {
            let pending = self
                .pending_sleep_diagnostic
                .take()
                .expect("ready sleep diagnostic exists");
            let epoch = pending.epoch.clone();
            let observed_frame_index = pending.observed_frame_index;
            let bytes = pending.finish()?;
            if self.host.construction_diagnostic_epoch() == epoch {
                self.host.publish_sleep_cycle_diagnostic(
                    crate::sleep_diagnostics::decode_sleep_cycle_metrics(&bytes)?,
                    observed_frame_index,
                );
            }
        }
        #[cfg(feature = "box2d-comparator")]
        while self
            .pending_grab_diagnostics
            .front()
            .is_some_and(PendingInteractionDiagnostic::is_ready)
        {
            let pending = self
                .pending_grab_diagnostics
                .pop_front()
                .expect("ready grab diagnostic exists");
            let epoch = pending.epoch.clone();
            let bytes = pending.finish()?;
            if self.host.construction_diagnostic_epoch() == epoch {
                self.host
                    .publish_grab_diagnostic(crate::grab::decode_grab_state(&bytes)?);
            }
        }
        Ok(())
    }
}

struct PendingInteractionDiagnostic {
    buffer: wgpu::Buffer,
    result: Arc<Mutex<Option<Result<(), String>>>>,
    epoch: (String, u64, u64),
    observed_frame_index: u64,
}

impl PendingInteractionDiagnostic {
    fn new(buffer: wgpu::Buffer, epoch: (String, u64, u64), observed_frame_index: u64) -> Self {
        let result = Arc::new(Mutex::new(None));
        Self {
            buffer,
            result,
            epoch,
            observed_frame_index,
        }
    }

    fn start_mapping(&self) {
        let callback = Arc::clone(&self.result);
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |mapped| {
                *callback
                    .lock()
                    .expect("interaction diagnostic callback lock") =
                    Some(mapped.map_err(|error| error.to_string()));
            });
    }

    fn is_ready(&self) -> bool {
        self.result
            .lock()
            .expect("interaction diagnostic readiness lock")
            .is_some()
    }

    fn finish(self) -> Result<Vec<u8>, String> {
        self.result
            .lock()
            .map_err(|_| "interaction diagnostic result lock poisoned".to_owned())?
            .take()
            .ok_or_else(|| "interaction diagnostic is not ready".to_owned())??;
        let mapped = self.buffer.slice(..).get_mapped_range();
        let bytes = mapped.to_vec();
        drop(mapped);
        self.buffer.unmap();
        Ok(bytes)
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

    pub fn read_particle_diagnostics(
        &self,
    ) -> Result<Option<crate::GpuParticleDiagnosticsV2>, String> {
        let Some(layout) = self.host.particle_validation_layout() else {
            return Ok(None);
        };
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_particle_validation_readback"),
            size: layout.total_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_particle_validation_copy"),
            });
        self.host
            .copy_particle_validation_state(&mut encoder, &buffer)?;
        self.queue.submit([encoder.finish()]);
        let mapped = map_buffer(&self.device, &buffer)?;
        let diagnostics = self.host.decode_particle_validation_state(&mapped)?;
        drop(mapped);
        buffer.unmap();
        Ok(Some(diagnostics))
    }

    pub fn read_grab_state(&self) -> Result<crate::GrabValidationState, String> {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_grab_state_readback"),
            size: u64::from(crate::grab::GRAB_STATE_BYTES),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_grab_state_copy"),
            });
        self.host.copy_grab_state(&mut encoder, &buffer)?;
        self.queue.submit([encoder.finish()]);
        let mapped = map_buffer(&self.device, &buffer)?;
        let state = crate::grab::decode_grab_state(&mapped)?;
        drop(mapped);
        buffer.unmap();
        Ok(state)
    }

    pub fn read_construction_state(
        &self,
    ) -> Result<physbox_contracts::ConstructionStateV1, String> {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_construction_state_readback"),
            size: u64::from(crate::CONSTRUCTION_STATE_BYTES),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_construction_state_copy"),
            });
        self.host.copy_construction_state(&mut encoder, &buffer)?;
        self.queue.submit([encoder.finish()]);
        let mapped = map_buffer(&self.device, &buffer)?;
        let state = crate::construction_state_from_bytes(&mapped)?;
        drop(mapped);
        buffer.unmap();
        Ok(state)
    }

    pub fn read_sleep_cycle_metrics(&self) -> Result<crate::SleepCycleMetricsV1, String> {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_sleep_cycle_readback"),
            size: u64::from(crate::SLEEP_CYCLE_DIAGNOSTIC_BYTES),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_sleep_cycle_copy"),
            });
        self.host.copy_sleep_cycle_metrics(&mut encoder, &buffer)?;
        self.queue.submit([encoder.finish()]);
        let mapped = map_buffer(&self.device, &buffer)?;
        let state = crate::sleep_diagnostics::decode_sleep_cycle_metrics(&mapped)?;
        drop(mapped);
        buffer.unmap();
        Ok(state)
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

    pub fn read_construction_graph_evidence(
        &self,
    ) -> Result<crate::GpuConstructionGraphEvidence, String> {
        let size = u64::from(physbox_contracts::COLLISION_CAPACITY_COUNTER_BYTES) * 4;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_construction_graph_evidence_readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_construction_graph_evidence_copy"),
            });
        self.host
            .copy_construction_graph_evidence(&mut encoder, &buffer)?;
        self.queue.submit([encoder.finish()]);
        let mapped = map_buffer(&self.device, &buffer)?;
        let evidence = crate::generated_contacts::decode_construction_graph_evidence(&mapped);
        drop(mapped);
        buffer.unmap();
        evidence
    }

    pub fn read_construction_body_evidence(
        &self,
    ) -> Result<Vec<crate::GpuConstructionBodySlotEvidence>, String> {
        let capacity = self.host.construction_body_capacity()?;
        let lane_bytes = u64::from(capacity) * std::mem::size_of::<[u32; 4]>() as u64;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_construction_body_evidence_readback"),
            size: lane_bytes * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_construction_body_evidence_copy"),
            });
        self.host
            .copy_construction_body_evidence(&mut encoder, &buffer)?;
        self.queue.submit([encoder.finish()]);
        let mapped = map_buffer(&self.device, &buffer)?;
        let lane = |index: usize| -> Result<&[[u32; 4]], String> {
            let start = index * lane_bytes as usize;
            bytemuck::try_cast_slice(&mapped[start..start + lane_bytes as usize])
                .map_err(|error| format!("decode construction body lane {index}: {error}"))
        };
        let poses = lane(0)?;
        let motions = lane(1)?;
        let properties = lane(2)?;
        let metadata = lane(3)?;
        let slots = (0..capacity as usize)
            .map(|slot| crate::GpuConstructionBodySlotEvidence {
                slot: slot as u32,
                pose: poses[slot].map(f32::from_bits),
                motion: motions[slot].map(f32::from_bits),
                properties: properties[slot].map(f32::from_bits),
                metadata: metadata[slot],
            })
            .collect();
        drop(mapped);
        buffer.unmap();
        Ok(slots)
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
                ..Default::default()
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
    fn soft_floor_pipeline_performance_smoke_is_finite_and_deterministic() {
        let mut physics = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
        physics.sleeping_enabled = true;
        let config = EngineConfig {
            scenario: ValidationScenario::RigidFloor,
            solver_profile: crate::SolverProfileConfig::from_physics(physics).unwrap(),
            ..Default::default()
        };
        let run = || {
            let mut renderer = pollster::block_on(OffscreenRenderer::new(320, 200, config.clone()))
                .expect("generated-contact renderer initializes");
            let report = renderer.render(FrameInput::exact(4));
            assert_eq!(report.scene_contract.contact_point_count, 1_024);
            assert_eq!(report.scene_contract.solver_dispatches_per_step, 158);
            assert_eq!(report.scene_contract.solver_capacity_bytes, 819_332);
            assert!(!report.scene_contract.capacity_overflowed);
            assert!(report.scene_contract.requested_sleeping_enabled);
            assert!(report.scene_contract.effective_sleeping_enabled);
            assert!(report.scene_contract.awake_compaction_enabled);
            assert!(
                renderer
                    .read_fixed_floor_pass_evidence()
                    .expect("active-work pass evidence decodes")
                    .coloring
                    .is_some()
            );
            (report, renderer.read_capture_state().unwrap())
        };
        let (first_report, first) = run();
        let (replay_report, replay) = run();
        assert_eq!(first, replay);
        assert_eq!(first_report.scene_contract, replay_report.scene_contract);
        assert!(first.into_iter().all(f32::is_finite));
        assert!(first[1] < 0.65, "soft-floor probe did not advance downward");
        println!(
            "PHYSBOX5_PERF_SMOKE scope=soft-floor exact_steps=4 dispatches_per_step={} allocated_bytes={} replay=exact capacity=clean",
            first_report.scene_contract.solver_dispatches_per_step,
            first_report.scene_contract.solver_capacity_bytes,
        );
    }

    #[test]
    fn sleeping_floor_pass_evidence_clears_zero_work_topology() {
        let mut physics = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
        physics.sleeping_enabled = true;
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                solver_profile: crate::SolverProfileConfig::from_physics(physics).unwrap(),
                ..Default::default()
            },
        ))
        .expect("generated-contact renderer initializes");

        let report = renderer.render(FrameInput::exact(120));
        assert!(report.scenario.complete && report.scenario.passed);
        let evidence = renderer
            .read_fixed_floor_pass_evidence()
            .expect("sleeping zero-work pass evidence decodes");
        assert!(evidence.contacts.iter().all(|contact| !contact.valid));
        let coloring = evidence.coloring.expect("selected solver uses coloring");
        assert_eq!(coloring.color_count, 0);
        assert!(coloring.color_occupancy.is_empty());
        assert_eq!(coloring.active_color_passes, 0);
        assert_eq!(coloring.island_count, 0);
    }

    fn assert_soft_resolution_replays(substeps: u32, iterations: u32) -> u32 {
        let mut physics = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
        physics.substeps = substeps;
        physics.biased_iterations = iterations;
        let config = EngineConfig {
            scenario: ValidationScenario::Bootstrap,
            world_capacity: Default::default(),
            solver_profile: crate::SolverProfileConfig::from_physics(physics).unwrap(),
        };
        let run = || {
            let mut renderer = pollster::block_on(OffscreenRenderer::new(320, 200, config.clone()))
                .expect("bounded soft configuration initializes");
            let report = renderer.render(FrameInput::exact(3));
            let state = renderer.read_capture_state().unwrap();
            (report, state)
        };
        let (first_report, first_state) = run();
        let (replay_report, replay_state) = run();
        assert_eq!(first_state, replay_state);
        assert!(first_state.into_iter().all(f32::is_finite));
        assert_eq!(first_report.scene_contract, replay_report.scene_contract);
        assert_eq!(first_report.scene_contract.solver_substeps, substeps);
        assert_eq!(
            first_report.scene_contract.solver_biased_iterations,
            iterations
        );
        assert_eq!(
            first_report.scene_contract.internal_substep_hz,
            60 * substeps
        );
        assert!(!first_report.scene_contract.capacity_overflowed);
        first_report.scene_contract.solver_dispatches_per_step
    }

    #[cfg(feature = "production-validation")]
    #[test]
    fn soft_resolution_boundaries_replay_and_report_their_work_graph() {
        let mut previous_dispatches = 0;
        for (substeps, iterations) in [(1, 1), (4, 1), (8, 4)] {
            let dispatches = assert_soft_resolution_replays(substeps, iterations);
            assert!(dispatches > previous_dispatches);
            previous_dispatches = dispatches;
        }
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn exhaustive_soft_resolution_matrix_replays_and_reports_its_work_graph() {
        for substeps in [1, 2, 4, 8] {
            let mut previous_dispatches = 0;
            for iterations in [1, 2, 4] {
                let dispatches = assert_soft_resolution_replays(substeps, iterations);
                assert!(dispatches > previous_dispatches);
                previous_dispatches = dispatches;
            }
        }
    }

    #[test]
    fn soft_rate_anchors_replay_with_the_declared_fixed_and_internal_rates() {
        for fixed_step_hz in [30, 60, 120] {
            let mut physics = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
            physics.fixed_step_hz = fixed_step_hz;
            let config = EngineConfig {
                scenario: ValidationScenario::Bootstrap,
                world_capacity: Default::default(),
                solver_profile: crate::SolverProfileConfig::from_physics(physics).unwrap(),
            };
            let run = || {
                let mut renderer =
                    pollster::block_on(OffscreenRenderer::new(320, 200, config.clone()))
                        .expect("soft rate anchor initializes");
                let report = renderer.render(FrameInput::exact(3));
                (report, renderer.read_capture_state().unwrap())
            };
            let (first_report, first_state) = run();
            let (replay_report, replay_state) = run();
            assert_eq!(first_state, replay_state);
            assert!(first_state.into_iter().all(f32::is_finite));
            assert_eq!(first_report.scene_contract, replay_report.scene_contract);
            assert_eq!(first_report.scene_contract.fixed_step_hz, fixed_step_hz);
            assert_eq!(
                first_report.scene_contract.internal_substep_hz,
                fixed_step_hz * physics.substeps
            );
            assert_eq!(
                first_report.scene_contract.fixed_dt_seconds.to_bits(),
                (1.0 / fixed_step_hz as f32).to_bits()
            );
            assert!(!first_report.scene_contract.capacity_overflowed);
        }
    }

    fn assert_soft_resolutions_preserve_grab_and_construction(configurations: &[(u32, u32)]) {
        let target_size = [960, 600];
        let layout = crate::ui::UiLayout::for_target(target_size);
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
        let placement = crate::Camera2d::fixed()
            .resolve(target_size)
            .unwrap()
            .world_to_screen([-0.8, 0.72]);

        for &(substeps, iterations) in configurations {
            let mut physics = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
            physics.substeps = substeps;
            physics.biased_iterations = iterations;
            let profile = crate::SolverProfileConfig::from_physics(physics).unwrap();

            let mut scripted = pollster::block_on(OffscreenRenderer::new(
                target_size[0],
                target_size[1],
                EngineConfig {
                    scenario: ValidationScenario::ResetGrab,
                    world_capacity: Default::default(),
                    solver_profile: profile.clone(),
                },
            ))
            .expect("configured reset/grab world initializes");
            let scripted_report = scripted.render(FrameInput::exact(11));
            assert!(scripted.read_grab_state().unwrap().active);
            assert!(scripted_report.body_commands.executed_commands_total > 0);
            assert!(!scripted_report.scene_contract.capacity_overflowed);
            assert!(
                scripted
                    .read_capture_state()
                    .unwrap()
                    .into_iter()
                    .all(f32::is_finite)
            );

            let mut construction = pollster::block_on(OffscreenRenderer::new(
                target_size[0],
                target_size[1],
                EngineConfig {
                    scenario: ValidationScenario::Bootstrap,
                    world_capacity: Default::default(),
                    solver_profile: profile,
                },
            ))
            .expect("configured construction world initializes");
            construction.render(
                FrameInput::exact(0)
                    .with_input_events(click(1, layout.center_normalized("tool", 1))),
            );
            let placed =
                construction.render(FrameInput::exact(1).with_input_events(click(2, placement)));
            assert_eq!(placed.body_commands.executed_construction_intents_total, 1);
            assert!(!placed.scene_contract.capacity_overflowed);
            assert_eq!(
                construction
                    .read_construction_state()
                    .unwrap()
                    .placed_circles,
                1
            );
        }
    }

    #[cfg(feature = "production-validation")]
    #[test]
    fn soft_resolution_boundary_preserves_grab_entry_and_construction() {
        assert_soft_resolutions_preserve_grab_and_construction(&[(8, 4)]);
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn exhaustive_soft_resolution_matrix_preserves_grab_entry_and_construction() {
        assert_soft_resolutions_preserve_grab_and_construction(&[
            (2, 1),
            (2, 2),
            (4, 1),
            (4, 2),
            (8, 1),
            (8, 2),
        ]);
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn scripted_reset_grab_uses_compliant_anchor_and_replays_after_reset() {
        let mut scripted = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::ResetGrab,
                ..Default::default()
            },
        ))
        .unwrap();
        scripted.render(FrameInput::exact(11));
        let begin = scripted.read_capture_state().unwrap();
        assert_eq!(begin[0], 0.0);
        assert!(begin[1] > 0.2 && begin[1] < 0.65);
        let begin_grab = scripted.read_grab_state().unwrap();
        assert!(begin_grab.active);
        assert_eq!(
            begin_grab.selected_body,
            Some(physbox_contracts::BodyId {
                slot: 0,
                generation: 1
            })
        );
        assert_eq!(begin_grab.local_anchor[0], 0.0);
        assert!(begin_grab.local_anchor[1] > 0.0 && begin_grab.local_anchor[1] < 0.25);
        scripted.render(FrameInput::exact(11));
        let first_target = scripted.read_capture_state().unwrap();
        assert!(first_target[0] > 0.0 && first_target[4] > 0.0);
        assert_ne!([first_target[0], first_target[1]], [0.45, 0.75]);
        assert_eq!(
            scripted.read_grab_state().unwrap().world_target,
            [0.45, 0.75]
        );
        scripted.render(FrameInput::exact(11));
        let second_target = scripted.read_capture_state().unwrap();
        assert!(second_target[4] < first_target[4]);
        assert_ne!([second_target[0], second_target[1]], [-0.4, 0.55]);
        assert_eq!(
            scripted.read_grab_state().unwrap().world_target,
            [-0.4, 0.55]
        );
        scripted.render(FrameInput::exact(9));
        let released = scripted.read_capture_state().unwrap();
        assert!(!scripted.read_grab_state().unwrap().active);
        assert!(released[4].hypot(released[5]) <= crate::grab::GRAB_MAX_LINEAR_SPEED);
        let report = scripted.render(FrameInput::exact(108));
        assert!(report.scenario.complete);
        assert!(report.scenario.passed);
        assert_eq!(report.body_commands.executed_commands_total, 6);
        assert_eq!(report.body_commands.reset_commands_total, 2);
        assert!(!report.body_commands.grab_pointer_active);
        #[cfg(feature = "box2d-comparator")]
        assert!(report.ui.box2d_failure.is_none());
        let replayed = scripted.read_capture_state().unwrap();

        let mut fresh = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
            },
        ))
        .unwrap();
        fresh.render(FrameInput::exact(70));
        assert_eq!(replayed, fresh.read_capture_state().unwrap());
    }

    #[test]
    fn live_pointer_targets_coalesce_until_an_admitted_step() {
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            960,
            600,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
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
            .resolve([960, 600])
            .unwrap()
            .world_to_screen([initial[0], initial[1]]);
        let queued = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 0 },
            input_events: vec![pointer(crate::PointerPhase::Down, initial_screen)],
        });
        assert_eq!(renderer.read_capture_state().unwrap(), initial);
        assert_eq!(queued.body_commands.queued_commands_total, 1);
        assert_eq!(queued.body_commands.executed_commands_total, 0);
        assert_eq!(queued.body_commands.grab_intent_dispatches_this_frame, 0);
        assert_eq!(
            queued.body_commands.grab_constraint_dispatches_this_frame,
            0
        );
        assert_eq!(
            queued
                .body_commands
                .grab_tether_prepare_dispatches_this_frame,
            1
        );
        assert_eq!(queued.body_commands.grab_tether_draw_calls_this_frame, 1);

        let admitted = renderer.render(FrameInput::exact(1));
        assert_eq!(admitted.body_commands.grab_intent_dispatches_this_frame, 1);
        assert_eq!(
            admitted.body_commands.grab_constraint_dispatches_this_frame,
            1
        );
        let held = renderer.read_capture_state().unwrap();
        assert_eq!(held[0], initial[0]);
        assert!(held[1] < initial[1] && held[1] > initial[1] - 0.01);
        let selected = renderer.read_grab_state().unwrap();
        assert!(selected.active);
        assert_eq!(selected.successful_selections, 1);
        assert_eq!(selected.local_anchor, [0.0, 0.0]);
        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(crate::PointerPhase::Move, [0.75, 0.25])],
        });
        let first_target = renderer.read_capture_state().unwrap();
        assert!(first_target[0] > held[0]);
        assert_ne!([first_target[0], first_target[1]], [0.8, 0.5]);
        assert_eq!(renderer.read_grab_state().unwrap().world_target, [0.8, 0.5]);
        let coalesced = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 0 },
            input_events: vec![
                pointer(crate::PointerPhase::Move, [0.25, 0.4]),
                pointer(crate::PointerPhase::Move, [0.5, 0.3]),
            ],
        });
        assert_eq!(renderer.read_capture_state().unwrap(), first_target);
        assert_eq!(coalesced.body_commands.coalesced_targets_total, 1);
        assert_eq!(coalesced.body_commands.grab_intent_dispatches_this_frame, 0);
        renderer.render(FrameInput::exact(1));
        let moved = renderer.read_capture_state().unwrap();
        assert_ne!([moved[0], moved[1]], [0.0, 0.39999998]);
        assert_eq!(
            renderer.read_grab_state().unwrap().world_target,
            [0.0, 0.39999998]
        );

        let release_report = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(crate::PointerPhase::Up, [0.5, 0.3])],
        });
        let released = renderer.read_capture_state().unwrap();
        assert!(!renderer.read_grab_state().unwrap().active);
        assert_eq!(
            release_report
                .body_commands
                .grab_tether_prepare_dispatches_this_frame,
            0
        );
        assert_eq!(
            release_report
                .body_commands
                .grab_tether_draw_calls_this_frame,
            0
        );
        assert!(released[4].hypot(released[5]) <= crate::grab::GRAB_MAX_LINEAR_SPEED);
        let stale_move = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(crate::PointerPhase::Move, [1.0, 0.0])],
        });
        let after_stale = renderer.read_capture_state().unwrap();
        assert_eq!(
            stale_move.body_commands.queued_commands_total,
            release_report.body_commands.queued_commands_total
        );
        assert_ne!(after_stale[0], 1.6);
        assert!((after_stale[0] - released[0]).abs() <= crate::grab::GRAB_MAX_LINEAR_SPEED / 60.0);
        assert!(after_stale[1] < released[1]);
    }

    #[test]
    fn zero_step_motion_preserves_the_pending_begin_hit_point() {
        let target_size = [960, 600];
        let frame = crate::Camera2d::fixed().resolve(target_size).unwrap();
        let pointer = |phase, world| {
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 18,
                phase,
                button: crate::PointerButton::Primary,
                normalized_position: frame.world_to_screen(world),
            })
        };
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target_size[0],
            target_size[1],
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
            },
        ))
        .unwrap();
        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 0 },
            input_events: vec![pointer(crate::PointerPhase::Down, [0.0, 0.65])],
        });
        let moved = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 0 },
            input_events: vec![pointer(crate::PointerPhase::Move, [0.8, 0.65])],
        });
        assert_eq!(moved.body_commands.coalesced_targets_total, 0);
        assert_eq!(moved.body_commands.queued_commands_total, 2);

        renderer.render(FrameInput::exact(1));
        let selected = renderer.read_grab_state().unwrap();
        assert!(selected.active);
        assert_eq!(selected.local_anchor, [0.0, 0.0]);
        assert_eq!(selected.world_target, [0.8, 0.65]);
    }

    #[test]
    fn gpu_selection_supports_circle_and_rectangle_and_rejects_fixed_only_hits() {
        let target_size = [960, 600];
        let click_world = |renderer: &mut OffscreenRenderer, id, world: [f32; 2]| {
            let at = crate::Camera2d::fixed()
                .resolve(target_size)
                .unwrap()
                .world_to_screen(world);
            renderer.render(FrameInput {
                step_request: crate::StepRequest::Exact { steps: 1 },
                input_events: vec![crate::InputEvent::Pointer(crate::PointerEvent {
                    id,
                    phase: crate::PointerPhase::Down,
                    button: crate::PointerButton::Primary,
                    normalized_position: at,
                })],
            });
        };

        for (world, slot) in [([0.0, 0.98], 3), ([0.0, 0.59], 2), ([0.0, 0.395], 1)] {
            let mut renderer = pollster::block_on(OffscreenRenderer::new(
                target_size[0],
                target_size[1],
                EngineConfig {
                    scenario: ValidationScenario::RigidStack,
                    ..Default::default()
                },
            ))
            .unwrap();
            click_world(&mut renderer, u64::from(slot), world);
            let selection = renderer.read_grab_state().unwrap();
            assert!(selection.active);
            assert_eq!(
                selection.selected_body,
                Some(physbox_contracts::BodyId {
                    slot,
                    generation: 1
                })
            );
            assert_eq!(selection.successful_selections, 1);
            assert_eq!(selection.scanned_bodies_high_water, 64);
        }

        let mut fixed = pollster::block_on(OffscreenRenderer::new(
            target_size[0],
            target_size[1],
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
            },
        ))
        .unwrap();
        click_world(&mut fixed, 10, [1.0, -0.1]);
        let rejected = fixed.read_grab_state().unwrap();
        assert!(!rejected.active);
        assert_eq!(rejected.selected_body, None);
        assert_eq!(rejected.begin_attempts, 1);
        assert_eq!(rejected.no_hit_selections, 1);
        assert_eq!(rejected.fixed_only_rejections, 1);
    }

    #[test]
    fn exactly_overlapping_hits_select_the_lower_body_slot() {
        let target_size = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target_size[0],
            target_size[1],
            EngineConfig {
                scenario: ValidationScenario::RigidStack,
                ..Default::default()
            },
        ))
        .unwrap();
        let pose = [0.0, 0.7, 0.0, 0.0];
        renderer
            .host
            .overwrite_body_pose_for_test(&renderer.queue, 1, pose);
        renderer
            .host
            .overwrite_body_pose_for_test(&renderer.queue, 2, pose);
        let at = crate::Camera2d::fixed()
            .resolve(target_size)
            .unwrap()
            .world_to_screen([pose[0], pose[1]]);
        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![crate::InputEvent::Pointer(crate::PointerEvent {
                id: 19,
                phase: crate::PointerPhase::Down,
                button: crate::PointerButton::Primary,
                normalized_position: at,
            })],
        });
        assert_eq!(
            renderer.read_grab_state().unwrap().selected_body,
            Some(physbox_contracts::BodyId {
                slot: 1,
                generation: 1,
            })
        );
    }

    #[test]
    fn active_grab_clears_before_steering_a_reused_generation() {
        let target_size = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target_size[0],
            target_size[1],
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
            },
        ))
        .unwrap();
        let at = crate::Camera2d::fixed()
            .resolve(target_size)
            .unwrap()
            .world_to_screen([0.0, 0.65]);
        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![crate::InputEvent::Pointer(crate::PointerEvent {
                id: 32,
                phase: crate::PointerPhase::Down,
                button: crate::PointerButton::Primary,
                normalized_position: at,
            })],
        });
        assert_eq!(
            renderer.read_grab_state().unwrap().selected_body,
            Some(physbox_contracts::BodyId {
                slot: 0,
                generation: 1,
            })
        );

        renderer
            .host
            .overwrite_body_generation_for_test(&renderer.queue, 0, 2);
        renderer.render(FrameInput::exact(1));
        let cleared = renderer.read_grab_state().unwrap();
        assert!(!cleared.active);
        assert_eq!(cleared.stale_generation_clears, 1);
        assert_eq!(cleared.selected_body, None);
    }

    #[test]
    fn off_center_gpu_grab_retains_anchor_rotates_and_releases_bounded_velocity() {
        let target_size = [960, 600];
        let frame = crate::Camera2d::fixed().resolve(target_size).unwrap();
        let pointer = |phase, world| {
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 31,
                phase,
                button: crate::PointerButton::Primary,
                normalized_position: frame.world_to_screen(world),
            })
        };
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target_size[0],
            target_size[1],
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
            },
        ))
        .unwrap();
        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(crate::PointerPhase::Down, [0.3, 0.65])],
        });
        let selected = renderer.read_grab_state().unwrap();
        assert!(selected.active);
        assert!((selected.local_anchor[0] - 0.3).abs() <= 1.0e-6);
        assert!(selected.local_anchor[1].abs() <= 1.0e-6);

        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 8 },
            input_events: vec![pointer(crate::PointerPhase::Move, [0.3, 1.2])],
        });
        let pulled = renderer.read_capture_state().unwrap();
        assert!(
            pulled[2] > 0.05,
            "off-center upward pull must rotate counter-clockwise"
        );
        assert!(pulled[6] > 0.0 && pulled[6] <= crate::grab::GRAB_MAX_ANGULAR_SPEED);
        assert_ne!([pulled[0], pulled[1]], [0.3, 1.2]);

        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(crate::PointerPhase::Up, [0.3, 1.2])],
        });
        let released = renderer.read_capture_state().unwrap();
        assert!(!renderer.read_grab_state().unwrap().active);
        assert!(released[4].hypot(released[5]) <= crate::grab::GRAB_MAX_LINEAR_SPEED);
        assert!(released[6].abs() <= crate::grab::GRAB_MAX_ANGULAR_SPEED);
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn active_grab_remains_collidable_against_the_fixed_floor() {
        let target_size = [960, 600];
        let frame = crate::Camera2d::fixed().resolve(target_size).unwrap();
        let pointer = |phase, world| {
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 33,
                phase,
                button: crate::PointerButton::Primary,
                normalized_position: frame.world_to_screen(world),
            })
        };
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target_size[0],
            target_size[1],
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
            },
        ))
        .unwrap();
        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(crate::PointerPhase::Down, [0.0, 0.65])],
        });
        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 29 },
            input_events: vec![pointer(crate::PointerPhase::Move, [0.0, -0.5])],
        });
        let held = renderer.read_capture_state().unwrap();
        assert!(renderer.read_grab_state().unwrap().active);
        assert!(
            held[1] >= 0.17,
            "grabbed body crossed the fixed floor: {held:?}"
        );
        assert!(held[1] > -0.4, "grab path must not teleport to its target");
    }

    #[test]
    fn empty_touch_is_inert_and_secondary_drag_changes_only_camera() {
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
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
        let no_hit = renderer.read_grab_state().unwrap();
        assert!(!no_hit.active);
        assert_eq!(no_hit.begin_attempts, 1);
        assert_eq!(no_hit.no_hit_selections, 1);
        assert_eq!(no_hit.fixed_only_rejections, 0);
        let mut control = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
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
                ..Default::default()
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
        assert_eq!(initial.ui.allocated_bytes, 48_144);

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
                grab_intent_contract_version: physbox_contracts::GRAB_INTENT_CONTRACT_VERSION,
                construction_intent_contract_version:
                    physbox_contracts::CONSTRUCTION_INTENT_CONTRACT_VERSION,
                capacity_commands: crate::BODY_COMMAND_CAPACITY,
                capacity_grab_intents: crate::grab::GRAB_INTENT_CAPACITY,
                capacity_construction_intents: crate::CONSTRUCTION_INTENT_CAPACITY,
                grab_state_bytes: crate::grab::GRAB_STATE_BYTES,
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

    #[cfg(not(feature = "box2d-comparator"))]
    #[test]
    fn feature_off_frame_has_no_comparator_selection_or_runtime_state() {
        let mut renderer =
            pollster::block_on(OffscreenRenderer::new(320, 200, EngineConfig::default())).unwrap();
        let report = renderer.render(FrameInput::exact(1));
        assert_eq!(report.ui.selected_engine, "physbox");
        assert!(!report.ui.comparator_available);
        assert_eq!(report.ui.box2d_revision, None);
        assert_eq!(report.ui.box2d_completed_steps, None);
        assert_eq!(report.ui.box2d_live_bodies, None);
        assert_eq!(report.ui.box2d_pending_actions, None);
        assert_eq!(report.ui.box2d_resolved_actions, None);
        assert_eq!(report.ui.box2d_identity_disagreements, None);
        assert_eq!(report.ui.box2d_action_queue_overflowed, None);
        assert_eq!(report.ui.box2d_aligned, None);
        assert_eq!(report.ui.box2d_failure, None);
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn shared_ui_switches_continuously_aligned_box2d_shadow_without_state_copy() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig {
                scenario: ValidationScenario::RectangleStack,
                ..Default::default()
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
        let physbox = renderer.render(FrameInput::exact(3));
        assert_eq!(physbox.ui.selected_engine, "physbox");
        assert!(physbox.ui.comparator_available);
        assert_eq!(physbox.ui.box2d_completed_steps, Some(3));
        assert_eq!(physbox.ui.box2d_aligned, Some(true));

        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(2, layout.center_normalized("world-tab", 0))),
        );
        let box2d = renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(3, layout.center_normalized("world-engine", 1))),
        );
        assert_eq!(box2d.ui.selected_engine, "box2d");
        assert_eq!(
            box2d.ui.box2d_revision,
            Some(physbox_box2d_comparator::BOX2D_REVISION)
        );
        assert_eq!(box2d.ui.box2d_aligned, Some(true));
        assert!(box2d.ui.box2d_failure.is_none());

        let evolved = renderer.render(FrameInput::exact(2));
        assert_eq!(evolved.fixed_steps_total, 5);
        assert_eq!(evolved.ui.box2d_completed_steps, Some(5));
        assert_eq!(evolved.ui.box2d_aligned, Some(true));

        renderer.render(
            FrameInput::exact(0).with_input_events(click(4, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(5, layout.center_normalized("world-tab", 0))),
        );
        let returned = renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(6, layout.center_normalized("world-engine", 0))),
        );
        assert_eq!(returned.ui.selected_engine, "physbox");
        assert_eq!(returned.fixed_steps_total, 5);
        assert_eq!(returned.ui.box2d_completed_steps, Some(5));
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn placement_and_erase_are_mirrored_into_the_box2d_shadow() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig::default(),
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
        let placement = [0.80, 0.20];
        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("tool", 1))),
        );
        renderer.render(FrameInput::exact(0).with_input_events(click(2, placement)));
        let placed = renderer.render(FrameInput::exact(1));
        assert_eq!(placed.body_commands.executed_construction_intents_total, 1);
        let placement_feedback = renderer.read_construction_state().unwrap();
        assert_eq!(placement_feedback.live_bodies, 3);
        renderer
            .host
            .publish_construction_diagnostic(placement_feedback, placed.frame_index);

        renderer.render(
            FrameInput::exact(0).with_input_events(click(3, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(4, layout.center_normalized("world-tab", 0))),
        );
        let box2d = renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(5, layout.center_normalized("world-engine", 1))),
        );
        assert_eq!(box2d.ui.selected_engine, "box2d");
        assert_eq!(box2d.ui.box2d_failure, None);
        assert_eq!(box2d.ui.box2d_completed_steps, Some(1));
        assert_eq!(box2d.ui.box2d_live_bodies, Some(3));

        renderer.render(
            FrameInput::exact(0).with_input_events(click(6, layout.center_normalized("tool", 3))),
        );
        renderer.render(FrameInput::exact(0).with_input_events(click(7, placement)));
        let erased = renderer.render(FrameInput::exact(1));
        assert_eq!(erased.body_commands.executed_construction_intents_total, 2);
        assert_eq!(renderer.read_construction_state().unwrap().live_bodies, 2);
        assert!(erased.ui.box2d_failure.is_none());
        assert_eq!(erased.ui.box2d_live_bodies, Some(2));
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn physbox_visible_gpu_identity_feedback_mirrors_divergent_erase_asynchronously() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig::default(),
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
        renderer
            .host
            .overwrite_body_pose_for_test(&renderer.queue, 0, [1.2, 0.65, 0.0, 0.0]);
        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("tool", 3))),
        );
        let at_physbox_body = crate::Camera2d::fixed()
            .resolve(target)
            .unwrap()
            .world_to_screen([1.2, 0.65]);
        renderer.render(FrameInput::exact(0).with_input_events(click(2, at_physbox_body)));
        let dispatched = renderer.render(FrameInput::exact(1));
        assert_eq!(dispatched.ui.box2d_pending_actions, Some(1));
        assert_eq!(dispatched.ui.box2d_live_bodies, Some(2));

        let feedback = renderer.read_construction_state().unwrap();
        assert_eq!(
            feedback.last_body,
            Some(physbox_contracts::BodyId {
                slot: 0,
                generation: 1,
            })
        );
        renderer
            .host
            .publish_construction_diagnostic(feedback, dispatched.frame_index);
        let resolved = renderer.render(FrameInput::exact(0));
        assert_eq!(resolved.ui.box2d_pending_actions, Some(0));
        assert_eq!(resolved.ui.box2d_resolved_actions, Some(1));
        assert_eq!(resolved.ui.box2d_live_bodies, Some(1));
        assert_eq!(resolved.ui.box2d_identity_disagreements, Some(0));
        assert!(resolved.ui.box2d_failure.is_none());
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn physbox_visible_gpu_identity_feedback_mirrors_divergent_grab_asynchronously() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig::default(),
        ))
        .unwrap();
        renderer
            .host
            .overwrite_body_pose_for_test(&renderer.queue, 0, [1.2, 0.65, 0.0, 0.0]);
        let at_physbox_body = crate::Camera2d::fixed()
            .resolve(target)
            .unwrap()
            .world_to_screen([1.2, 0.65]);
        let dispatched = renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: [crate::PointerPhase::Down, crate::PointerPhase::Up]
                .into_iter()
                .map(|phase| {
                    crate::InputEvent::Pointer(crate::PointerEvent {
                        id: 1,
                        phase,
                        button: crate::PointerButton::Primary,
                        normalized_position: at_physbox_body,
                    })
                })
                .collect(),
        });
        assert_eq!(dispatched.ui.box2d_pending_actions, Some(2));
        let feedback = renderer.read_grab_state().unwrap();
        assert!(!feedback.active);
        assert_eq!(
            feedback.last_selected_body,
            Some(physbox_contracts::BodyId {
                slot: 0,
                generation: 1,
            })
        );
        renderer.host.publish_grab_diagnostic(feedback);
        let resolved = renderer.render(FrameInput::exact(0));
        assert_eq!(resolved.ui.box2d_pending_actions, Some(0));
        assert_eq!(resolved.ui.box2d_resolved_actions, Some(2));
        assert_eq!(resolved.ui.box2d_identity_disagreements, Some(0));
        assert!(resolved.ui.box2d_failure.is_none());
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn compact_gpu_feedback_preserves_multiple_construction_results_in_one_frame() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig::default(),
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
        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("tool", 1))),
        );
        let mut clicks = click(2, [0.80, 0.20]);
        clicks.extend(click(3, [0.80, 0.35]));
        renderer.render(FrameInput::exact(0).with_input_events(clicks));
        let dispatched = renderer.render(FrameInput::exact(1));
        assert_eq!(dispatched.ui.box2d_pending_actions, Some(2));
        let feedback = renderer.read_construction_state().unwrap();
        assert_eq!(feedback.recent_results.len(), 2);
        assert_eq!(feedback.live_bodies, 4);
        renderer
            .host
            .publish_construction_diagnostic(feedback, dispatched.frame_index);
        let resolved = renderer.render(FrameInput::exact(0));
        assert_eq!(resolved.ui.box2d_pending_actions, Some(0));
        assert_eq!(resolved.ui.box2d_resolved_actions, Some(2));
        assert_eq!(resolved.ui.box2d_live_bodies, Some(4));
        assert!(resolved.ui.box2d_failure.is_none());
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn box2d_visible_grab_targets_the_same_stable_id_after_trajectories_diverge() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig::default(),
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
        renderer
            .host
            .overwrite_body_pose_for_test(&renderer.queue, 0, [1.2, 0.65, 0.0, 0.0]);
        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(2, layout.center_normalized("world-tab", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(3, layout.center_normalized("world-engine", 1))),
        );

        let at_box2d_body = crate::Camera2d::fixed()
            .resolve(target)
            .unwrap()
            .world_to_screen([0.0, 0.65]);
        renderer.render(FrameInput {
            step_request: crate::StepRequest::Exact { steps: 1 },
            input_events: vec![crate::InputEvent::Pointer(crate::PointerEvent {
                id: 4,
                phase: crate::PointerPhase::Down,
                button: crate::PointerButton::Primary,
                normalized_position: at_box2d_body,
            })],
        });
        assert_eq!(
            renderer.read_grab_state().unwrap().selected_body,
            Some(physbox_contracts::BodyId {
                slot: 0,
                generation: 1,
            })
        );
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn box2d_visible_erase_targets_the_same_stable_id_after_trajectories_diverge() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig::default(),
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
        renderer
            .host
            .overwrite_body_pose_for_test(&renderer.queue, 0, [1.2, 0.65, 0.0, 0.0]);
        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(2, layout.center_normalized("world-tab", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(3, layout.center_normalized("world-engine", 1))),
        );
        renderer.render(
            FrameInput::exact(0).with_input_events(click(4, layout.center_normalized("tool", 3))),
        );

        let at_box2d_body = crate::Camera2d::fixed()
            .resolve(target)
            .unwrap()
            .world_to_screen([0.0, 0.65]);
        renderer.render(FrameInput::exact(0).with_input_events(click(5, at_box2d_body)));
        let erased = renderer.render(FrameInput::exact(1));
        let construction = renderer.read_construction_state().unwrap();
        assert_eq!(
            construction.last_body,
            Some(physbox_contracts::BodyId {
                slot: 0,
                generation: 1,
            })
        );
        assert_eq!(construction.live_bodies, 1);
        assert_eq!(erased.ui.box2d_live_bodies, Some(1));
        assert!(erased.ui.box2d_failure.is_none());
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn box2d_visible_placement_miss_is_authoritative_after_trajectories_diverge() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig::default(),
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
        renderer
            .host
            .overwrite_body_pose_for_test(&renderer.queue, 0, [1.2, 0.65, 0.0, 0.0]);
        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(2, layout.center_normalized("world-tab", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(3, layout.center_normalized("world-engine", 1))),
        );
        renderer.render(
            FrameInput::exact(0).with_input_events(click(4, layout.center_normalized("tool", 1))),
        );

        let at_box2d_body = crate::Camera2d::fixed()
            .resolve(target)
            .unwrap()
            .world_to_screen([0.0, 0.65]);
        renderer.render(FrameInput::exact(0).with_input_events(click(5, at_box2d_body)));
        let rejected = renderer.render(FrameInput::exact(1));
        let construction = renderer.read_construction_state().unwrap();
        assert_eq!(
            construction.last_result,
            physbox_contracts::ConstructionResultV1::DeepOverlap
        );
        assert_eq!(construction.live_bodies, 2);
        assert_eq!(rejected.ui.box2d_live_bodies, Some(2));
        assert!(rejected.ui.box2d_failure.is_none());
        renderer
            .host
            .publish_construction_diagnostic(construction, rejected.frame_index);
        let verified = renderer.render(FrameInput::exact(0));
        assert_eq!(verified.ui.box2d_pending_actions, Some(0));
        assert_eq!(verified.ui.box2d_resolved_actions, Some(1));
        assert_eq!(verified.ui.box2d_identity_disagreements, Some(0));
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn box2d_visible_successful_placement_is_verified_by_gpu_identity_feedback() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig::default(),
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
        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(2, layout.center_normalized("world-tab", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(3, layout.center_normalized("world-engine", 1))),
        );
        renderer.render(
            FrameInput::exact(0).with_input_events(click(4, layout.center_normalized("tool", 1))),
        );
        renderer.render(FrameInput::exact(0).with_input_events(click(5, [0.80, 0.20])));
        let dispatched = renderer.render(FrameInput::exact(1));
        let construction = renderer.read_construction_state().unwrap();
        assert_eq!(
            construction.last_result,
            physbox_contracts::ConstructionResultV1::Placed
        );
        assert_eq!(
            construction.last_body,
            Some(physbox_contracts::BodyId {
                slot: 2,
                generation: 1,
            })
        );
        assert_eq!(dispatched.ui.box2d_pending_actions, Some(1));
        renderer
            .host
            .publish_construction_diagnostic(construction, dispatched.frame_index);
        let verified = renderer.render(FrameInput::exact(0));
        assert_eq!(verified.ui.box2d_pending_actions, Some(0));
        assert_eq!(verified.ui.box2d_resolved_actions, Some(1));
        assert_eq!(verified.ui.box2d_live_bodies, Some(3));
        assert!(verified.ui.box2d_failure.is_none());
    }

    #[test]
    fn world_menu_rebuilds_capacity_and_sleep_request_while_preserving_tool() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig::default(),
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
        let initial = renderer.render(FrameInput::exact(0));
        assert_eq!(initial.scene_contract.requested_body_capacity, 64);
        assert_eq!(initial.scene_contract.body_capacity, 64);

        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("tool", 1))),
        );
        renderer.render(
            FrameInput::exact(0).with_input_events(click(2, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(3, layout.center_normalized("world-tab", 0))),
        );
        let changed = renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(4, layout.center_normalized("world-capacity", 1))),
        );
        assert_eq!(changed.scene_contract.requested_body_capacity, 256);
        assert_eq!(changed.scene_contract.body_capacity, 256);
        assert_eq!(changed.scene_contract.body_count, 2);
        assert!(
            changed.scene_contract.solver_capacity_bytes
                > initial.scene_contract.solver_capacity_bytes
        );
        assert!(!changed.scene_contract.capacity_overflowed);
        assert_eq!(changed.ui.current_tool, "circle");
        assert_eq!(changed.ui.scenario_restarts_total, 1);
        assert!(!changed.ui.menu_open);

        renderer.render(
            FrameInput::exact(0).with_input_events(click(5, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(6, layout.center_normalized("world-tab", 0))),
        );
        let sleeping_requested = renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(7, layout.center_normalized("world-sleeping", 0))),
        );
        assert!(sleeping_requested.scene_contract.requested_sleeping_enabled);
        assert!(sleeping_requested.scene_contract.effective_sleeping_enabled);
        assert!(sleeping_requested.scene_contract.awake_compaction_enabled);
        assert_eq!(
            sleeping_requested.scene_contract.requested_body_capacity,
            256
        );
        assert_eq!(sleeping_requested.ui.current_tool, "circle");
        assert!(!sleeping_requested.ui.sleep_visuals_enabled);
        assert!(!sleeping_requested.ui.menu_open);

        renderer.render(
            FrameInput::exact(0).with_input_events(click(8, layout.center_normalized("menu", 0))),
        );
        renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(9, layout.center_normalized("world-tab", 0))),
        );
        let visuals_enabled = renderer.render(FrameInput::exact(0).with_input_events(click(
            10,
            layout.center_normalized("world-sleep-visuals", 0),
        )));
        assert!(visuals_enabled.ui.sleep_visuals_enabled);
        assert!(visuals_enabled.scene_contract.requested_sleeping_enabled);
        assert!(visuals_enabled.scene_contract.effective_sleeping_enabled);
        assert_eq!(visuals_enabled.ui.scenario_restarts_total, 1);
        assert!(!visuals_enabled.ui.menu_open);
    }

    #[cfg(feature = "solver-lab")]
    #[test]
    fn scene_menu_rejects_an_incompatible_scene_without_automatic_hard_fallback() {
        let target = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target[0],
            target[1],
            EngineConfig {
                scenario: ValidationScenario::Bootstrap,
                solver_profile: crate::SolverProfileConfig::soft_coupled(),
                ..Default::default()
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
        renderer.render(
            FrameInput::exact(0).with_input_events(click(40, layout.center_normalized("menu", 0))),
        );
        let switched = renderer.render(
            FrameInput::exact(0)
                .with_input_events(click(41, layout.center_normalized("scenario", 9))),
        );
        assert_eq!(switched.scene_contract.scenario, "bootstrap");
        assert_eq!(switched.scene_contract.solver_profile, "soft-coupled");
        assert_eq!(
            switched.scene_contract.solver_profile_options.as_deref(),
            Some("hz=30,damping=10,static-scale=2")
        );
        assert_eq!(switched.ui.scenario_activations_total, 0);
    }

    #[test]
    fn manifold_i2_validation_reports_the_exact_selected_execution() {
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::SolverManifoldI2,
                world_capacity: Default::default(),
                solver_profile: crate::SolverProfileConfig::hard_direct(),
            },
        ))
        .unwrap();
        let report = renderer.render(FrameInput::exact(
            crate::P5_2_MANIFOLD_COLOR_VALIDATION_STEPS,
        ));
        assert!(report.scenario.complete && report.scenario.passed);
        assert_eq!(
            report.scene_contract.generated_solver_execution,
            Some(crate::GpuSolverExecutionV4::manifold_colored_block_experiment(true, true,))
        );
        assert_eq!(
            report.scene_contract.generated_solver_scheduling_policy,
            Some(crate::GpuSolverSchedulingPolicyV1::SERIAL_CONTROL)
        );
        assert_eq!(
            report.scene_contract.generated_contact_coefficient_policy,
            Some(crate::GpuContactCoefficientPolicyV1::SELECTED)
        );
        assert_eq!(
            crate::GpuSolverExecutionV4::SELECTED.normal_solver,
            crate::GpuNormalSolverV4::ManifoldBlock
        );
        assert_eq!(report.fixed_steps_total, 8);
        assert!(!report.scene_contract.capacity_overflowed);
    }

    #[test]
    fn parallel_coloring_validation_reports_the_selected_product_schedule() {
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            320,
            200,
            EngineConfig {
                scenario: ValidationScenario::SolverParallelColoring,
                world_capacity: Default::default(),
                solver_profile: crate::SolverProfileConfig::hard_direct(),
            },
        ))
        .unwrap();
        let report = renderer.render(FrameInput::exact(
            crate::P5_2_MANIFOLD_COLOR_VALIDATION_STEPS,
        ));
        assert!(report.scenario.complete && report.scenario.passed);
        assert_eq!(
            report.scene_contract.generated_solver_execution,
            Some(crate::GpuSolverExecutionV4::SELECTED)
        );
        assert_eq!(
            report.scene_contract.generated_solver_scheduling_policy,
            Some(crate::GpuSolverSchedulingPolicyV1::SELECTED)
        );
        assert_eq!(
            report.scene_contract.generated_contact_coefficient_policy,
            Some(crate::GpuContactCoefficientPolicyV1::SELECTED)
        );
        assert_eq!(report.fixed_steps_total, 8);
        assert!(!report.scene_contract.capacity_overflowed);
        assert_eq!(
            crate::GpuSolverSchedulingPolicyV1::SELECTED.color_maintenance,
            crate::GpuColorMaintenanceV1::FullRebuildParallelSpeculative
        );
    }

    #[test]
    fn shared_toolbar_routes_release_committed_construction_and_two_touch_cancel() {
        let target_size = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target_size[0],
            target_size[1],
            EngineConfig {
                scenario: ValidationScenario::Bootstrap,
                ..Default::default()
            },
        ))
        .unwrap();
        let layout = crate::ui::UiLayout::for_target(target_size);
        let click = |id, button, at| {
            [crate::PointerPhase::Down, crate::PointerPhase::Up]
                .into_iter()
                .map(|phase| {
                    crate::InputEvent::Pointer(crate::PointerEvent {
                        id,
                        phase,
                        button,
                        normalized_position: at,
                    })
                })
                .collect::<Vec<_>>()
        };
        let world_at = |world| {
            crate::Camera2d::fixed()
                .resolve(target_size)
                .unwrap()
                .world_to_screen(world)
        };
        let circle_at = world_at([-0.8, 0.72]);
        let rectangle_at = world_at([0.8, 0.72]);

        let circle_tool = renderer.render(FrameInput::exact(0).with_input_events(click(
            1,
            crate::PointerButton::Primary,
            layout.center_normalized("tool", 1),
        )));
        assert_eq!(circle_tool.ui.current_tool, "circle");
        let placed_circle = renderer.render(FrameInput::exact(1).with_input_events(click(
            2,
            crate::PointerButton::Primary,
            circle_at,
        )));
        assert_eq!(
            placed_circle
                .body_commands
                .executed_construction_intents_total,
            1
        );

        let rectangle_tool = renderer.render(FrameInput::exact(0).with_input_events(click(
            3,
            crate::PointerButton::Primary,
            layout.center_normalized("tool", 2),
        )));
        assert_eq!(rectangle_tool.ui.current_tool, "rectangle");
        renderer.render(FrameInput::exact(1).with_input_events(click(
            4,
            crate::PointerButton::Primary,
            rectangle_at,
        )));

        renderer.render(FrameInput::exact(0).with_input_events(click(
            5,
            crate::PointerButton::Primary,
            layout.center_normalized("tool", 3),
        )));
        renderer.render(FrameInput::exact(1).with_input_events(click(
            6,
            crate::PointerButton::Primary,
            circle_at,
        )));
        renderer.render(FrameInput::exact(0).with_input_events(click(
            7,
            crate::PointerButton::Primary,
            layout.center_normalized("tool", 1),
        )));
        renderer.render(FrameInput::exact(1).with_input_events(click(
            8,
            crate::PointerButton::Primary,
            circle_at,
        )));
        let lifecycle = renderer.read_construction_state().unwrap();
        assert_eq!(lifecycle.live_bodies, 4);
        assert_eq!(lifecycle.attempted_intents, 4);
        assert_eq!(lifecycle.succeeded_intents, 4);
        assert_eq!(lifecycle.placed_circles, 2);
        assert_eq!(lifecycle.placed_rectangles, 1);
        assert_eq!(lifecycle.erased_bodies, 1);
        assert_eq!(
            lifecycle.last_body,
            Some(physbox_contracts::BodyId {
                slot: 2,
                generation: 2,
            })
        );

        renderer.render(FrameInput::exact(0).with_input_events(click(
            9,
            crate::PointerButton::Primary,
            layout.center_normalized("tool", 0),
        )));
        let grabbed = renderer.render(FrameInput::exact(1).with_input_events(vec![
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 10,
                phase: crate::PointerPhase::Down,
                button: crate::PointerButton::Primary,
                normalized_position: world_at([0.0, 0.62]),
            }),
        ]));
        assert!(grabbed.body_commands.grab_pointer_active);
        assert!(renderer.read_grab_state().unwrap().active);
        let changed_tool = renderer.render(FrameInput::exact(1).with_input_events(click(
            11,
            crate::PointerButton::Primary,
            layout.center_normalized("tool", 1),
        )));
        assert_eq!(changed_tool.ui.current_tool, "circle");
        assert!(!changed_tool.body_commands.grab_pointer_active);
        assert!(!renderer.read_grab_state().unwrap().active);

        let touch_a = world_at([0.45, 0.85]);
        let touch_b = world_at([0.65, 0.85]);
        let cancelled = renderer.render(FrameInput::exact(0).with_input_events(vec![
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 20,
                phase: crate::PointerPhase::Down,
                button: crate::PointerButton::Contact,
                normalized_position: touch_a,
            }),
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 21,
                phase: crate::PointerPhase::Down,
                button: crate::PointerButton::Contact,
                normalized_position: touch_b,
            }),
        ]));
        assert!(!cancelled.body_commands.construction_pointer_active);
        renderer.render(FrameInput::exact(0).with_input_events(vec![
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 20,
                phase: crate::PointerPhase::Up,
                button: crate::PointerButton::Contact,
                normalized_position: touch_a,
            }),
            crate::InputEvent::Pointer(crate::PointerEvent {
                id: 21,
                phase: crate::PointerPhase::Up,
                button: crate::PointerButton::Contact,
                normalized_position: touch_b,
            }),
        ]));
        let after_cancel = renderer.render(FrameInput::exact(1));
        assert_eq!(
            after_cancel.body_commands.queued_construction_intents_total,
            4
        );
        assert_eq!(
            renderer
                .read_construction_state()
                .unwrap()
                .attempted_intents,
            4
        );
    }

    #[test]
    fn held_placement_paints_moved_targets_through_the_gpu_construction_path() {
        let target_size = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target_size[0],
            target_size[1],
            EngineConfig {
                scenario: ValidationScenario::Bootstrap,
                ..Default::default()
            },
        ))
        .unwrap();
        let layout = crate::ui::UiLayout::for_target(target_size);
        let world_at = |world| {
            crate::Camera2d::fixed()
                .resolve(target_size)
                .unwrap()
                .world_to_screen(world)
        };
        let pointer = |id, phase, at| {
            crate::InputEvent::Pointer(crate::PointerEvent {
                id,
                phase,
                button: crate::PointerButton::Primary,
                normalized_position: at,
            })
        };
        let circle_tool = layout.center_normalized("tool", 1);
        let selected = renderer.render(FrameInput::exact(0).with_input_events(vec![
            pointer(1, crate::PointerPhase::Down, circle_tool),
            pointer(1, crate::PointerPhase::Up, circle_tool),
        ]));
        assert_eq!(selected.ui.current_tool, "circle");

        let first_target = world_at([-0.65, 0.55]);
        let second_target = world_at([0.65, 0.55]);
        renderer.render(FrameInput::exact(0).with_input_events(vec![pointer(
            2,
            crate::PointerPhase::Down,
            first_target,
        )]));
        renderer.render(FrameInput::exact(14));
        renderer.render(FrameInput::exact(1).with_input_events(vec![pointer(
            2,
            crate::PointerPhase::Move,
            first_target,
        )]));
        renderer.render(FrameInput::exact(6).with_input_events(vec![pointer(
            2,
            crate::PointerPhase::Move,
            second_target,
        )]));
        let released = renderer.render(FrameInput::exact(1).with_input_events(vec![pointer(
            2,
            crate::PointerPhase::Up,
            second_target,
        )]));

        assert_eq!(
            released.body_commands.executed_construction_intents_total,
            2
        );
        assert!(!released.body_commands.construction_pointer_active);
        let construction = renderer.read_construction_state().unwrap();
        assert_eq!(construction.attempted_intents, 2);
        assert_eq!(construction.succeeded_intents, 2);
        assert_eq!(construction.placed_circles, 2);
        assert_eq!(construction.live_bodies, 4);
    }

    #[test]
    fn erase_and_generation_reuse_exclude_dead_proxy_and_recontact() {
        let target_size = [960, 600];
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            target_size[0],
            target_size[1],
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
            },
        ))
        .unwrap();
        let layout = crate::ui::UiLayout::for_target(target_size);
        let screen = crate::Camera2d::fixed().resolve(target_size).unwrap();
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

        renderer.render(FrameInput::exact(1));
        let initial = renderer.read_construction_graph_evidence().unwrap();
        assert_eq!(initial.candidate_pairs, 0);
        assert!(!initial.overflowed);

        renderer.render(
            FrameInput::exact(0).with_input_events(click(1, layout.center_normalized("tool", 3))),
        );
        renderer.render(
            FrameInput::exact(1).with_input_events(click(2, screen.world_to_screen([0.0, 0.65]))),
        );
        let erased = renderer.read_construction_state().unwrap();
        assert_eq!(erased.erased_bodies, 1);
        assert_eq!(erased.live_bodies, 1);
        let cleared = renderer.read_construction_graph_evidence().unwrap();
        assert_eq!(cleared.candidate_pairs, 0);
        assert_eq!(cleared.manifolds, 0);
        assert_eq!(cleared.contact_points, 0);
        assert_eq!(cleared.persistent_pairs, 0);

        renderer.render(
            FrameInput::exact(0).with_input_events(click(3, layout.center_normalized("tool", 1))),
        );
        let placed = renderer.render(
            FrameInput::exact(1).with_input_events(click(4, screen.world_to_screen([0.0, 0.1]))),
        );
        assert!(
            !placed
                .construction_diagnostic
                .blocking_readback_for_advancement
        );
        let reused = renderer.read_construction_state().unwrap();
        assert_eq!(
            reused.last_body,
            Some(physbox_contracts::BodyId {
                slot: 0,
                generation: 2,
            })
        );
        let rebuilt = renderer.read_construction_graph_evidence().unwrap();
        assert!(rebuilt.candidate_pairs > 0);
        assert!(rebuilt.manifolds > 0);
        assert!(rebuilt.contact_points > 0);
        assert!(rebuilt.persistent_pairs > 0);
        assert!(!rebuilt.overflowed);
    }

    #[test]
    fn command_capacity_overflow_is_reported_without_partial_execution() {
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            960,
            600,
            EngineConfig {
                scenario: ValidationScenario::RigidFloor,
                ..Default::default()
            },
        ))
        .unwrap();
        let target = crate::Camera2d::fixed()
            .resolve([960, 600])
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
