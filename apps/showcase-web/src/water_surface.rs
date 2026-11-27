pub(super) struct WaterSurface {
    composite: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    field: Option<(wgpu::Extent3d, wgpu::TextureView, wgpu::BindGroup)>,
}

impl WaterSurface {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water surface"),
            source: wgpu::ShaderSource::Wgsl(include_str!("water_surface.wgsl").into()),
        });
        let composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("water surface"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &composite_shader,
                entry_point: Some("vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &composite_shader,
                entry_point: Some("fragment"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            composite,
            sampler,
            field: None,
        }
    }

    pub(super) fn accumulate(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        size: [u32; 2],
        pipeline: &wgpu::RenderPipeline,
        group: &wgpu::BindGroup,
        capacity: u32,
    ) {
        let extent = wgpu::Extent3d {
            width: size[0].div_ceil(2).max(1),
            height: size[1].div_ceil(2).max(1),
            depth_or_array_layers: 1,
        };
        if self.field.as_ref().is_none_or(|field| field.0 != extent) {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("water thickness"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("water thickness display"),
                layout: &self.composite.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.field = Some((extent, view, group));
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("water thickness"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.field.as_ref().unwrap().1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..6, 0..capacity);
    }

    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let Some((_, _, group)) = &self.field {
            pass.set_pipeline(&self.composite);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
