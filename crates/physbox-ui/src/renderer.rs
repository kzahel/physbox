use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{
    Primitive, RenderReport,
    font::{
        GLYPH_CELL_WIDTH, GLYPH_HEIGHT, GLYPH_WIDTH, glyph_atlas_bytes, glyph_atlas_height,
        glyph_atlas_r8, glyph_atlas_width, glyph_uv,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceCapacities {
    pub retained: usize,
    pub live: usize,
}

impl InstanceCapacities {
    pub const fn new(retained: usize, live: usize) -> Self {
        Self { retained, live }
    }
}

#[derive(Clone, Copy)]
pub struct RenderTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub size: [u32; 2],
}

pub struct CanvasFrame<'a> {
    pub target: RenderTarget<'a>,
    pub retained_revision: u64,
    pub retained: &'a [Primitive],
    pub live: &'a [Primitive],
    pub source_overflowed: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
struct Params {
    viewport_padding: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
struct Instance {
    rect: [f32; 4],
    uv_or_border: [f32; 4],
    color: [f32; 4],
    data: [f32; 4],
}

fn instance_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 0,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 16,
            shader_location: 1,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 32,
            shader_location: 2,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 48,
            shader_location: 3,
        },
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Instance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATTRIBUTES,
    }
}

pub struct CanvasRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params: wgpu::Buffer,
    retained_instances: wgpu::Buffer,
    live_instances: wgpu::Buffer,
    capacities: InstanceCapacities,
    last_target_size: Option<[u32; 2]>,
    retained_key: Option<(u64, [u32; 2])>,
    retained_count: u32,
    retained_glyph_count: u32,
    live_count: u32,
    live_glyph_count: u32,
    retained_writes_total: u64,
    live_writes_total: u64,
    parameter_writes_total: u64,
    retained_upload_bytes_total: u64,
    live_upload_bytes_total: u64,
    parameter_upload_bytes_total: u64,
    overflowed: bool,
}

impl CanvasRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        capacities: InstanceCapacities,
    ) -> Self {
        assert!(capacities.retained > 0 && capacities.live > 0);
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox_ui_params"),
            contents: bytemuck::bytes_of(&Params::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let atlas = glyph_atlas_r8();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("physbox_ui_glyph_atlas"),
            size: wgpu::Extent3d {
                width: glyph_atlas_width(),
                height: glyph_atlas_height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Default::default(),
                aspect: wgpu::TextureAspect::All,
            },
            &atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(glyph_atlas_width()),
                rows_per_image: Some(glyph_atlas_height()),
            },
            wgpu::Extent3d {
                width: glyph_atlas_width(),
                height: glyph_atlas_height(),
                depth_or_array_layers: 1,
            },
        );
        let texture_view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("physbox_ui_glyph_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox_ui_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox_ui_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox_ui_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("renderer.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox_ui_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("physbox_ui_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[instance_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
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
        let retained_instances =
            instance_buffer(device, "physbox_ui_retained_instances", capacities.retained);
        let live_instances = instance_buffer(device, "physbox_ui_live_instances", capacities.live);
        Self {
            pipeline,
            bind_group,
            params,
            retained_instances,
            live_instances,
            capacities,
            last_target_size: None,
            retained_key: None,
            retained_count: 0,
            retained_glyph_count: 0,
            live_count: 0,
            live_glyph_count: 0,
            retained_writes_total: 0,
            live_writes_total: 0,
            parameter_writes_total: 0,
            retained_upload_bytes_total: 0,
            live_upload_bytes_total: 0,
            parameter_upload_bytes_total: 0,
            overflowed: false,
        }
    }

    pub fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: CanvasFrame<'_>,
    ) -> RenderReport {
        let CanvasFrame {
            target,
            retained_revision,
            retained,
            live,
            source_overflowed,
        } = frame;
        if self.last_target_size != Some(target.size) {
            let params = Params {
                viewport_padding: [target.size[0] as f32, target.size[1] as f32, 0.0, 0.0],
            };
            let bytes = bytemuck::bytes_of(&params);
            queue.write_buffer(&self.params, 0, bytes);
            self.parameter_writes_total = self.parameter_writes_total.saturating_add(1);
            self.parameter_upload_bytes_total = self
                .parameter_upload_bytes_total
                .saturating_add(bytes.len() as u64);
            self.last_target_size = Some(target.size);
        }
        let retained_key = (retained_revision, target.size);
        if self.retained_key != Some(retained_key) {
            let batch = Batch::from_primitives(retained, self.capacities.retained);
            upload(
                queue,
                &self.retained_instances,
                &batch.instances,
                &mut self.retained_upload_bytes_total,
            );
            self.retained_count = batch.instances.len() as u32;
            self.retained_glyph_count = batch.glyph_count;
            self.retained_writes_total = self.retained_writes_total.saturating_add(1);
            self.overflowed |= batch.overflowed;
            self.retained_key = Some(retained_key);
        }
        let live_batch = Batch::from_primitives(live, self.capacities.live);
        upload(
            queue,
            &self.live_instances,
            &live_batch.instances,
            &mut self.live_upload_bytes_total,
        );
        self.live_count = live_batch.instances.len() as u32;
        self.live_glyph_count = live_batch.glyph_count;
        self.live_writes_total = self.live_writes_total.saturating_add(1);
        self.overflowed |= live_batch.overflowed | source_overflowed;

        let mut draw_calls = 0;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox_ui_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        if self.retained_count > 0 {
            pass.set_vertex_buffer(0, self.retained_instances.slice(..));
            pass.draw(0..6, 0..self.retained_count);
            draw_calls += 1;
        }
        if self.live_count > 0 {
            pass.set_vertex_buffer(0, self.live_instances.slice(..));
            pass.draw(0..6, 0..self.live_count);
            draw_calls += 1;
        }
        drop(pass);

        RenderReport {
            retained_instance_capacity: self.capacities.retained as u32,
            live_instance_capacity: self.capacities.live as u32,
            instance_stride_bytes: std::mem::size_of::<Instance>() as u32,
            atlas_bytes: glyph_atlas_bytes(),
            allocated_bytes: glyph_atlas_bytes()
                + std::mem::size_of::<Params>() as u64
                + ((self.capacities.retained + self.capacities.live)
                    * std::mem::size_of::<Instance>()) as u64,
            retained_instances: self.retained_count,
            live_instances: self.live_count,
            retained_glyphs: self.retained_glyph_count,
            live_glyphs: self.live_glyph_count,
            retained_writes_total: self.retained_writes_total,
            live_writes_total: self.live_writes_total,
            parameter_writes_total: self.parameter_writes_total,
            retained_upload_bytes_total: self.retained_upload_bytes_total,
            live_upload_bytes_total: self.live_upload_bytes_total,
            parameter_upload_bytes_total: self.parameter_upload_bytes_total,
            render_passes_this_frame: 1,
            draw_calls_this_frame: draw_calls,
            overflowed: self.overflowed,
        }
    }
}

fn instance_buffer(device: &wgpu::Device, label: &str, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (capacity * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn upload(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    instances: &[Instance],
    bytes_total: &mut u64,
) {
    if instances.is_empty() {
        return;
    }
    let bytes = bytemuck::cast_slice(instances);
    queue.write_buffer(buffer, 0, bytes);
    *bytes_total = bytes_total.saturating_add(bytes.len() as u64);
}

struct Batch {
    instances: Vec<Instance>,
    capacity: usize,
    glyph_count: u32,
    overflowed: bool,
}

impl Batch {
    fn from_primitives(primitives: &[Primitive], capacity: usize) -> Self {
        let mut batch = Self {
            instances: Vec::with_capacity(capacity),
            capacity,
            glyph_count: 0,
            overflowed: false,
        };
        for primitive in primitives {
            match primitive {
                Primitive::RoundedRect {
                    bounds,
                    fill,
                    radius,
                    border_width,
                    border,
                } => batch.push(Instance {
                    rect: bounds.to_array(),
                    uv_or_border: *border,
                    color: *fill,
                    data: [0.0, *radius, *border_width, 0.0],
                }),
                Primitive::Text {
                    text,
                    origin,
                    pixel_size,
                    color,
                } => batch.push_text(text, [origin.x, origin.y], *pixel_size, *color),
                Primitive::Icon {
                    icon,
                    bounds,
                    color,
                } => batch.push(Instance {
                    rect: bounds.to_array(),
                    uv_or_border: [0.0; 4],
                    color: *color,
                    data: [2.0, 0.0, 0.0, icon.0 as f32],
                }),
            }
        }
        batch
    }

    fn push_text(&mut self, text: &str, origin: [f32; 2], pixel: f32, color: [f32; 4]) {
        let mut x = origin[0];
        for byte in text.bytes() {
            if byte != b' ' {
                let before = self.instances.len();
                self.push(Instance {
                    rect: [
                        x,
                        origin[1],
                        GLYPH_WIDTH as f32 * pixel,
                        GLYPH_HEIGHT as f32 * pixel,
                    ],
                    uv_or_border: glyph_uv(byte),
                    color,
                    data: [1.0, 0.0, 0.0, 0.0],
                });
                if self.instances.len() != before {
                    self.glyph_count = self.glyph_count.saturating_add(1);
                }
            }
            x += GLYPH_CELL_WIDTH as f32 * pixel;
        }
    }

    fn push(&mut self, instance: Instance) {
        if self.instances.len() == self.capacity {
            self.overflowed = true;
        } else {
            self.instances.push(instance);
        }
    }
}
