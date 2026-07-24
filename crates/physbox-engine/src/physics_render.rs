use std::fmt::{Display, Formatter};
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_sim::__private::render_view::{ParticleRenderView, RENDER_VIEW_VERSION, RenderView};

pub(crate) const BODY_SHADER_SOURCE: &str = include_str!("physics_body_render.wgsl");
pub(crate) const PARTICLE_SHADER_SOURCE: &str = include_str!("physics_particle_render.wgsl");

const GRAB_RENDER_VERTEX_COUNT: u32 = 12;
const GRAB_RENDER_VERTEX_BYTES: u64 = GRAB_RENDER_VERTEX_COUNT as u64 * 16;
const SURFACE_PREVIEW_HALF_THICKNESS: f32 = 0.04;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RenderParamsRaw {
    viewport: [f32; 4],
    camera: [f32; 4],
    sleep_visualization: [u32; 4],
    world_bounds: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParticleRenderParamsRaw {
    viewport: [f32; 4],
    camera: [f32; 4],
    material: [f32; 4],
    debug: [u32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    pub center: [f32; 2],
    pub half_extents: [f32; 2],
}

impl ViewTransform {
    pub fn validate(self) -> Result<(), RenderError> {
        if self
            .center
            .into_iter()
            .chain(self.half_extents)
            .all(f32::is_finite)
            && self.half_extents[0] > 0.0
            && self.half_extents[1] > 0.0
        {
            Ok(())
        } else {
            Err(RenderError(
                "view center must be finite and half extents positive and finite".to_owned(),
            ))
        }
    }
}

pub struct RenderTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub size: [u32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePreview {
    pub start: [f32; 2],
    pub end: [f32; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum Background {
    /// Draw the PhysBox world background before physics geometry.
    #[default]
    PhysBox,
    /// Clear to the supplied color before physics geometry.
    Clear(wgpu::Color),
}

#[derive(Clone, Copy)]
pub struct BodyRenderTimestamp<'a> {
    pub query_set: &'a wgpu::QuerySet,
    pub query_index: u32,
}

#[derive(Clone, Copy)]
pub struct RenderOptions<'a> {
    pub frame_index: u64,
    pub view: ViewTransform,
    pub background: Background,
    pub grab_pointer_active: bool,
    pub surface_preview: Option<SurfacePreview>,
    pub sleep_visuals_enabled: bool,
    pub current_outer_step: u32,
    pub quiet_outer_steps: u32,
    pub wake_flash_outer_steps: u32,
    pub density_debug: bool,
    pub body_timestamp: Option<BodyRenderTimestamp<'a>>,
}

impl Default for RenderOptions<'_> {
    fn default() -> Self {
        Self {
            frame_index: 0,
            view: ViewTransform {
                center: [0.0, 0.0],
                half_extents: [1.0, 1.0],
            },
            background: Background::default(),
            grab_pointer_active: false,
            surface_preview: None,
            sleep_visuals_enabled: false,
            current_outer_step: 0,
            quiet_outer_steps: 0,
            wake_flash_outer_steps: 0,
            density_debug: false,
            body_timestamp: None,
        }
    }
}

pub struct PhysicsRenderer {
    render_params: wgpu::Buffer,
    grab_render_vertices: wgpu::Buffer,
    grab_render_bind_group: wgpu::BindGroup,
    grab_render_prepare_pipeline: wgpu::ComputePipeline,
    render_bind_group: wgpu::BindGroup,
    background_pipeline: wgpu::RenderPipeline,
    body_pipeline: wgpu::RenderPipeline,
    grab_pipeline: wgpu::RenderPipeline,
    active_body_buffer: wgpu::Buffer,
    active_draw_offset: u64,
    kill_zone_y: f32,
    particle: Option<ParticleRenderer>,
}

struct ParticleRenderer {
    params: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    capacity: u32,
    material: [f32; 3],
}

impl PhysicsRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        view: RenderView<'_>,
    ) -> Result<Self, RenderError> {
        if view.version() != RENDER_VIEW_VERSION {
            return Err(RenderError(format!(
                "unsupported simulation render view version {}",
                view.version()
            )));
        }
        let render_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_render_params"),
            size: size_of::<RenderParamsRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let grab_render_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_grab_render_vertices"),
            size: GRAB_RENDER_VERTEX_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let grab_render_prepare_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("physbox5_grab_render_prepare_layout"),
                entries: &[
                    storage_entry(0, true, wgpu::ShaderStages::COMPUTE),
                    storage_entry(1, true, wgpu::ShaderStages::COMPUTE),
                    storage_entry(2, true, wgpu::ShaderStages::COMPUTE),
                    uniform_entry(3, wgpu::ShaderStages::COMPUTE),
                    storage_entry(4, false, wgpu::ShaderStages::COMPUTE),
                ],
            });
        let grab_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_grab_render_prepare_bind_group"),
            layout: &grab_render_prepare_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view.poses().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: view.metadata().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: view.grab_state().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: render_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: grab_render_vertices.as_entire_binding(),
                },
            ],
        });
        let grab_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_grab_render_prepare_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("physics_grab_render_prepare.wgsl").into(),
            ),
        });
        let grab_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_grab_render_prepare_pipeline_layout"),
            bind_group_layouts: &[&grab_render_prepare_layout],
            push_constant_ranges: &[],
        });
        let grab_render_prepare_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("physbox5_grab_render_prepare_pipeline"),
                layout: Some(&grab_layout),
                module: &grab_shader,
                entry_point: Some("prepare_grab_vertices"),
                compilation_options: Default::default(),
                cache: None,
            });

        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_body_render_layout"),
            entries: &[
                storage_entry(0, true, wgpu::ShaderStages::VERTEX),
                storage_entry(1, true, wgpu::ShaderStages::VERTEX),
                storage_entry(2, true, wgpu::ShaderStages::VERTEX),
                uniform_entry(3, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
                storage_entry(4, true, wgpu::ShaderStages::VERTEX),
                storage_entry(5, true, wgpu::ShaderStages::VERTEX),
                storage_entry(6, true, wgpu::ShaderStages::VERTEX),
            ],
        });
        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_body_render_bind_group"),
            layout: &render_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view.poses().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: view.properties().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: view.metadata().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: render_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: view.active_bodies().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: view.sleep_states().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: view.grab_state().as_entire_binding(),
                },
            ],
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_body_render_shader"),
            source: wgpu::ShaderSource::Wgsl(BODY_SHADER_SOURCE.into()),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_body_render_pipeline_layout"),
                bind_group_layouts: &[&render_layout],
                push_constant_ranges: &[],
            });
        let pipeline = |label, vertex, fragment, blend, buffers| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &render_shader,
                    entry_point: Some(vertex),
                    buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &render_shader,
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
        };
        let background_pipeline = pipeline(
            "physbox5_body_background_pipeline",
            "background_vertex",
            "background_fragment",
            None,
            &[],
        );
        let body_pipeline = pipeline(
            "physbox5_body_render_pipeline",
            "body_vertex",
            "body_fragment",
            Some(wgpu::BlendState::ALPHA_BLENDING),
            &[],
        );
        let grab_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            }],
        };
        let grab_pipeline = pipeline(
            "physbox5_grab_render_pipeline",
            "grab_vertex",
            "grab_fragment",
            Some(wgpu::BlendState::ALPHA_BLENDING),
            &[grab_vertex_layout],
        );
        let particle = view
            .particles()
            .map(|particle| ParticleRenderer::new(device, color_format, particle));

        Ok(Self {
            render_params,
            grab_render_vertices,
            grab_render_bind_group,
            grab_render_prepare_pipeline,
            render_bind_group,
            background_pipeline,
            body_pipeline,
            grab_pipeline,
            active_body_buffer: view.active_bodies().clone(),
            active_draw_offset: view.active_draw_offset(),
            kill_zone_y: view.kill_zone_y(),
            particle,
        })
    }

    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderTarget<'_>,
        options: RenderOptions<'_>,
    ) -> Result<(), RenderError> {
        if target.size[0] == 0 || target.size[1] == 0 {
            return Err(RenderError(
                "render target extent must be nonzero".to_owned(),
            ));
        }
        options.view.validate()?;
        queue.write_buffer(
            &self.render_params,
            0,
            bytemuck::bytes_of(&RenderParamsRaw {
                viewport: [
                    target.size[0] as f32,
                    target.size[1] as f32,
                    target.size[0] as f32 / target.size[1] as f32,
                    options.frame_index as f32,
                ],
                camera: [
                    options.view.center[0],
                    options.view.center[1],
                    options.view.half_extents[0],
                    options.view.half_extents[1],
                ],
                sleep_visualization: [
                    u32::from(options.sleep_visuals_enabled),
                    options.current_outer_step,
                    options.quiet_outer_steps,
                    options.wake_flash_outer_steps,
                ],
                world_bounds: [self.kill_zone_y, 0.0, 0.0, 0.0],
            }),
        );
        let surface_preview_active = options.surface_preview.is_some_and(|preview| {
            let Some(vertices) = surface_preview_vertices(preview) else {
                return false;
            };
            queue.write_buffer(
                &self.grab_render_vertices,
                0,
                bytemuck::cast_slice(&vertices),
            );
            true
        });
        if options.grab_pointer_active {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_grab_render_prepare_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.grab_render_prepare_pipeline);
            pass.set_bind_group(0, &self.grab_render_bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        let load = match options.background {
            Background::PhysBox => wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.008,
                g: 0.015,
                b: 0.035,
                a: 1.0,
            }),
            Background::Clear(color) => wgpu::LoadOp::Clear(color),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox5_body_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: options.body_timestamp.map(|timestamp| {
                wgpu::RenderPassTimestampWrites {
                    query_set: timestamp.query_set,
                    beginning_of_pass_write_index: None,
                    end_of_pass_write_index: Some(timestamp.query_index),
                }
            }),
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.render_bind_group, &[]);
        match options.background {
            Background::PhysBox => {
                pass.set_pipeline(&self.background_pipeline);
                pass.draw(0..3, 0..1);
            }
            Background::Clear(_) => {}
        }
        pass.set_pipeline(&self.body_pipeline);
        pass.draw_indirect(&self.active_body_buffer, self.active_draw_offset);
        if options.grab_pointer_active {
            pass.set_pipeline(&self.grab_pipeline);
            pass.set_vertex_buffer(0, self.grab_render_vertices.slice(..));
            pass.draw(0..GRAB_RENDER_VERTEX_COUNT, 0..1);
        } else if surface_preview_active {
            pass.set_pipeline(&self.grab_pipeline);
            pass.set_vertex_buffer(0, self.grab_render_vertices.slice(..));
            pass.draw(0..6, 0..1);
        }
        drop(pass);
        if let Some(particle) = &self.particle {
            particle.encode(
                queue,
                encoder,
                target,
                options.view,
                options.frame_index,
                options.density_debug,
            );
        }
        Ok(())
    }

    pub fn capacity_records(
        &self,
        width: u32,
        height: u32,
    ) -> Vec<physbox_contracts::CapacityRecordV1> {
        vec![
            physbox_contracts::CapacityRecordV1 {
                class: "grab-render-vertices".to_owned(),
                capacity_elements: u64::from(GRAB_RENDER_VERTEX_COUNT),
                used_elements: u64::from(GRAB_RENDER_VERTEX_COUNT),
                allocated_bytes: GRAB_RENDER_VERTEX_BYTES,
                high_water_bytes: GRAB_RENDER_VERTEX_BYTES,
                overflowed: false,
            },
            physbox_contracts::CapacityRecordV1 {
                class: "body-render-parameters".to_owned(),
                capacity_elements: 1,
                used_elements: 1,
                allocated_bytes: size_of::<RenderParamsRaw>() as u64,
                high_water_bytes: size_of::<RenderParamsRaw>() as u64,
                overflowed: false,
            },
            physbox_contracts::CapacityRecordV1 {
                class: "render-target-rgba8-logical".to_owned(),
                capacity_elements: u64::from(width) * u64::from(height),
                used_elements: u64::from(width) * u64::from(height),
                allocated_bytes: u64::from(width) * u64::from(height) * 4,
                high_water_bytes: u64::from(width) * u64::from(height) * 4,
                overflowed: false,
            },
        ]
    }
}

impl ParticleRenderer {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        view: ParticleRenderView<'_>,
    ) -> Self {
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_particle_render_params"),
            size: size_of::<ParticleRenderParamsRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_particle_render_layout"),
            entries: &[
                storage_entry(0, true, wgpu::ShaderStages::VERTEX),
                storage_entry(1, true, wgpu::ShaderStages::VERTEX),
                storage_entry(2, true, wgpu::ShaderStages::VERTEX),
                storage_entry(3, true, wgpu::ShaderStages::VERTEX),
                uniform_entry(4, wgpu::ShaderStages::VERTEX),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_particle_render_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view.positions().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: view.velocities().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: view.density_states().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: view.metadata().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_particle_render"),
            source: wgpu::ShaderSource::Wgsl(PARTICLE_SHADER_SOURCE.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_particle_render_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("physbox5_particle_render_pipeline"),
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
        Self {
            params,
            bind_group,
            pipeline,
            capacity: view.capacity(),
            material: [view.radius(), view.support_radius(), view.rest_density()],
        }
    }

    fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderTarget<'_>,
        view: ViewTransform,
        frame_index: u64,
        density_debug: bool,
    ) {
        queue.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&ParticleRenderParamsRaw {
                viewport: [
                    target.size[0] as f32,
                    target.size[1] as f32,
                    target.size[0] as f32 / target.size[1].max(1) as f32,
                    frame_index as f32,
                ],
                camera: [
                    view.center[0],
                    view.center[1],
                    view.half_extents[0],
                    view.half_extents[1],
                ],
                material: [
                    self.material[0] * 1.12,
                    self.material[1],
                    self.material[2],
                    0.0,
                ],
                debug: [u32::from(density_debug), 0, 0, 0],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox5_particle_render_pass"),
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
        pass.draw(0..6, 0..self.capacity);
    }
}

fn surface_preview_vertices(preview: SurfacePreview) -> Option<[[f32; 4]; 6]> {
    let delta = [
        preview.end[0] - preview.start[0],
        preview.end[1] - preview.start[1],
    ];
    let length = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt();
    if !length.is_finite() || length < 0.001 {
        return None;
    }
    let perpendicular = [
        -delta[1] / length * SURFACE_PREVIEW_HALF_THICKNESS,
        delta[0] / length * SURFACE_PREVIEW_HALF_THICKNESS,
    ];
    let start_low = [
        preview.start[0] - perpendicular[0],
        preview.start[1] - perpendicular[1],
        1.0,
        1.0,
    ];
    let start_high = [
        preview.start[0] + perpendicular[0],
        preview.start[1] + perpendicular[1],
        1.0,
        1.0,
    ];
    let end_low = [
        preview.end[0] - perpendicular[0],
        preview.end[1] - perpendicular[1],
        1.0,
        1.0,
    ];
    let end_high = [
        preview.end[0] + perpendicular[0],
        preview.end[1] + perpendicular[1],
        1.0,
        1.0,
    ];
    Some([
        start_low, end_low, start_high, start_high, end_low, end_high,
    ])
}

fn storage_entry(
    binding: u32,
    read_only: bool,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderError(String);

impl Display for RenderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RenderError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_contract_rejects_nonfinite_and_degenerate_extents() {
        ViewTransform {
            center: [0.0, 1.0],
            half_extents: [2.0, 3.0],
        }
        .validate()
        .unwrap();
        assert!(
            ViewTransform {
                center: [f32::NAN, 0.0],
                half_extents: [1.0, 1.0],
            }
            .validate()
            .is_err()
        );
        assert!(
            ViewTransform {
                center: [0.0; 2],
                half_extents: [0.0, 1.0],
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn surface_preview_is_bounded_and_has_two_triangles() {
        let vertices = surface_preview_vertices(SurfacePreview {
            start: [-1.0, 0.0],
            end: [1.0, 0.0],
        })
        .unwrap();
        assert_eq!(vertices.len(), 6);
        assert_eq!(vertices[0][1], -SURFACE_PREVIEW_HALF_THICKNESS);
        assert_eq!(vertices[2][1], SURFACE_PREVIEW_HALF_THICKNESS);
    }
}
