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
        let host = EngineHost::new(&device, view_format, engine_config);
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
        let host = EngineHost::new(&device, format, engine_config);
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

    pub fn read_bootstrap_state(&self) -> Result<[f32; 8], String> {
        read_state(&self.device, &self.queue, self.host.state_buffer())
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
    source: &wgpu::Buffer,
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
    encoder.copy_buffer_to_buffer(source, 0, &buffer, 0, size);
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
