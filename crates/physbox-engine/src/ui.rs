use std::collections::BTreeSet;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{ConstructionResultV1, ConstructionStateV1};
use physbox_launch::ValidationScenario;
use wgpu::util::DeviceExt;

use crate::{
    InputEvent, PointerButton, PointerPhase,
    clock::PacingReport,
    renderer::RenderFrameTarget,
    report::UiReport,
    ui_font::{
        GLYPH_CELL_WIDTH, GLYPH_HEIGHT, GLYPH_WIDTH, glyph_atlas_bytes, glyph_atlas_height,
        glyph_atlas_r8, glyph_atlas_width, glyph_uv,
    },
};

pub(crate) const UI_CONTRACT_VERSION: u32 = 2;
const STATIC_INSTANCE_CAPACITY: usize = 256;
const DYNAMIC_INSTANCE_CAPACITY: usize = 192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAction {
    ActivateScenario(ValidationScenario),
    RestartScenario,
    Recenter,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum WorkbenchTool {
    #[default]
    Grab,
    Circle,
    Rectangle,
    Erase,
}

impl WorkbenchTool {
    const ALL: [Self; 4] = [Self::Grab, Self::Circle, Self::Rectangle, Self::Erase];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Grab => "grab",
            Self::Circle => "circle",
            Self::Rectangle => "rectangle",
            Self::Erase => "erase",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Grab => "GRAB",
            Self::Circle => "CIRC",
            Self::Rectangle => "RECT",
            Self::Erase => "ERASE",
        }
    }
}

pub(crate) struct UiRouting {
    pub events: Vec<InputEvent>,
    pub action: Option<UiAction>,
    pub selected_tool: WorkbenchTool,
}

pub(crate) struct UiFrameModel<'a> {
    pub scenario: ValidationScenario,
    pub pacing: &'a PacingReport,
    pub body_count: u32,
    pub contact_count: u32,
    pub capacity_bytes: u64,
    pub overflowed: bool,
    pub adapter_backend: &'a str,
    pub scenario_activations_total: u64,
    pub scenario_restarts_total: u64,
    pub construction_state: Option<&'a ConstructionStateV1>,
    pub construction_age_frames: Option<u64>,
    pub construction_diagnostic_source: &'static str,
}

/// Shared canvas UI and input capture.
///
/// The compact atlas, instanced glyph batch, static-write caching, and toolbar
/// sizing are selectively ported from PhysBox 4 revision
/// `4295957e15480ef701e6006c7cafb5f0728ef101`, principally `toolbar.rs`,
/// `options/font.rs`, `options/text.rs`, and `options/renderer.rs`. PhysBox 5
/// owns a smaller workbench-only model and one combined quad/glyph pipeline.
pub(crate) struct WorkbenchUi {
    renderer: UiRenderer,
    menu_open: bool,
    selected_tool: WorkbenchTool,
    captured_pointers: BTreeSet<u64>,
}

impl WorkbenchUi {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            renderer: UiRenderer::new(device, queue, color_format),
            menu_open: false,
            selected_tool: WorkbenchTool::default(),
            captured_pointers: BTreeSet::new(),
        }
    }

    pub(crate) fn route_events(
        &mut self,
        events: &[InputEvent],
        target_size: [u32; 2],
    ) -> UiRouting {
        route_events(
            &mut self.menu_open,
            &mut self.selected_tool,
            &mut self.captured_pointers,
            events,
            target_size,
        )
    }

    pub(crate) fn select_tool(&mut self, tool: WorkbenchTool) {
        self.selected_tool = tool;
    }

    pub(crate) fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        model: UiFrameModel<'_>,
    ) -> UiReport {
        self.renderer.encode(
            queue,
            encoder,
            target,
            model,
            self.menu_open,
            self.selected_tool,
        )
    }
}

fn route_events(
    menu_open: &mut bool,
    selected_tool: &mut WorkbenchTool,
    captured_pointers: &mut BTreeSet<u64>,
    events: &[InputEvent],
    target_size: [u32; 2],
) -> UiRouting {
    let mut forwarded = Vec::with_capacity(events.len());
    let mut action = None;
    for event in events.iter().copied() {
        let InputEvent::Pointer(pointer) = event else {
            forwarded.push(event);
            continue;
        };
        if captured_pointers.contains(&pointer.id) {
            if matches!(pointer.phase, PointerPhase::Up | PointerPhase::Cancel) {
                captured_pointers.remove(&pointer.id);
            }
            continue;
        }
        let activates_ui = matches!(
            pointer.button,
            PointerButton::Primary | PointerButton::Contact
        ) && pointer.phase == PointerPhase::Down;
        if activates_ui {
            let layout = UiLayout::for_target(target_size);
            if let Some(hit) = layout.hit_test(pointer.normalized_position, *menu_open) {
                captured_pointers.insert(pointer.id);
                match hit {
                    UiHit::Menu => *menu_open = !*menu_open,
                    UiHit::Tool(tool) => *selected_tool = tool,
                    UiHit::Scenario(scenario) => {
                        *menu_open = false;
                        action = Some(UiAction::ActivateScenario(scenario));
                    }
                    UiHit::Restart => {
                        *menu_open = false;
                        action = Some(UiAction::RestartScenario);
                    }
                    UiHit::Recenter => {
                        *menu_open = false;
                        action = Some(UiAction::Recenter);
                    }
                    UiHit::Panel => {}
                }
                continue;
            }
        }
        forwarded.push(event);
    }
    UiRouting {
        events: forwarded,
        action,
        selected_tool: *selected_tool,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiHit {
    Menu,
    Tool(WorkbenchTool),
    Scenario(ValidationScenario),
    Restart,
    Recenter,
    Panel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiLayout {
    target_size: [u32; 2],
    scale: f32,
}

impl UiLayout {
    pub(crate) fn for_target(target_size: [u32; 2]) -> Self {
        let minimum_dimension = target_size[0].min(target_size[1]).max(1) as f32;
        Self {
            target_size,
            scale: (minimum_dimension / 600.0).clamp(1.0, 2.0),
        }
    }

    fn hit_test(self, normalized: [f32; 2], menu_open: bool) -> Option<UiHit> {
        let pixel = [
            normalized[0] * self.target_size[0] as f32,
            normalized[1] * self.target_size[1] as f32,
        ];
        if contains(self.menu_button(), pixel) {
            return Some(UiHit::Menu);
        }
        for tool in WorkbenchTool::ALL {
            if contains(self.tool_button(tool), pixel) {
                return Some(UiHit::Tool(tool));
            }
        }
        if !menu_open {
            return None;
        }
        for (index, scenario) in ValidationScenario::ALL.iter().copied().enumerate() {
            if contains(self.scenario_button(index), pixel) {
                return Some(UiHit::Scenario(scenario));
            }
        }
        if contains(self.restart_button(), pixel) {
            return Some(UiHit::Restart);
        }
        if contains(self.recenter_button(), pixel) {
            return Some(UiHit::Recenter);
        }
        contains(self.menu_panel(), pixel).then_some(UiHit::Panel)
    }

    fn menu_button(self) -> [f32; 4] {
        [
            12.0 * self.scale,
            24.0 * self.scale,
            100.0 * self.scale,
            52.0 * self.scale,
        ]
    }

    fn tool_button(self, tool: WorkbenchTool) -> [f32; 4] {
        let index = match tool {
            WorkbenchTool::Grab => 0,
            WorkbenchTool::Circle => 1,
            WorkbenchTool::Rectangle => 2,
            WorkbenchTool::Erase => 3,
        };
        [
            (118.0 + index as f32 * 76.0) * self.scale,
            24.0 * self.scale,
            70.0 * self.scale,
            52.0 * self.scale,
        ]
    }

    fn menu_panel(self) -> [f32; 4] {
        let restart_y = self.restart_y_logical();
        let logical = if self.compact_landscape() {
            [12.0, 84.0, 500.0, restart_y + 50.0 - 84.0]
        } else {
            [12.0, 84.0, 360.0, restart_y + 50.0 - 84.0]
        };
        logical.map(|value| value * self.scale)
    }

    fn scenario_button(self, index: usize) -> [f32; 4] {
        if self.compact_landscape() {
            let column = index % 2;
            let row = index / 2;
            return [
                (24.0 + column as f32 * 238.0) * self.scale,
                (126.0 + row as f32 * 38.0) * self.scale,
                226.0 * self.scale,
                34.0 * self.scale,
            ];
        }
        [
            24.0 * self.scale,
            (126.0 + index as f32 * 38.0) * self.scale,
            336.0 * self.scale,
            34.0 * self.scale,
        ]
    }

    fn restart_button(self) -> [f32; 4] {
        if self.compact_landscape() {
            return [
                24.0 * self.scale,
                self.restart_y_logical() * self.scale,
                226.0 * self.scale,
                38.0 * self.scale,
            ];
        }
        [
            24.0 * self.scale,
            self.restart_y_logical() * self.scale,
            162.0 * self.scale,
            38.0 * self.scale,
        ]
    }

    fn recenter_button(self) -> [f32; 4] {
        if self.compact_landscape() {
            return [
                262.0 * self.scale,
                self.restart_y_logical() * self.scale,
                226.0 * self.scale,
                38.0 * self.scale,
            ];
        }
        [
            198.0 * self.scale,
            self.restart_y_logical() * self.scale,
            162.0 * self.scale,
            38.0 * self.scale,
        ]
    }

    fn compact_landscape(self) -> bool {
        self.target_size[1] < 520 && self.target_size[0] > self.target_size[1]
    }

    fn restart_y_logical(self) -> f32 {
        let rows = if self.compact_landscape() {
            ValidationScenario::ALL.len().div_ceil(2)
        } else {
            ValidationScenario::ALL.len()
        };
        let gap = if self.compact_landscape() { 20.0 } else { 14.0 };
        126.0 + (rows.saturating_sub(1)) as f32 * 38.0 + 34.0 + gap
    }

    #[cfg(test)]
    pub(crate) fn center_normalized(self, hit: &str, index: usize) -> [f32; 2] {
        let bounds = match hit {
            "menu" => self.menu_button(),
            "tool" => self.tool_button(WorkbenchTool::ALL[index]),
            "scenario" => self.scenario_button(index),
            "restart" => self.restart_button(),
            "recenter" => self.recenter_button(),
            _ => panic!("unknown UI test target"),
        };
        [
            (bounds[0] + bounds[2] * 0.5) / self.target_size[0] as f32,
            (bounds[1] + bounds[3] * 0.5) / self.target_size[1] as f32,
        ]
    }
}

fn contains(bounds: [f32; 4], pixel: [f32; 2]) -> bool {
    pixel[0] >= bounds[0]
        && pixel[1] >= bounds[1]
        && pixel[0] <= bounds[0] + bounds[2]
        && pixel[1] <= bounds[1] + bounds[3]
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
struct UiParams {
    viewport_padding: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
struct UiInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    mode: [f32; 4],
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
        array_stride: std::mem::size_of::<UiInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATTRIBUTES,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StaticKey {
    target_size: [u32; 2],
    scenario: ValidationScenario,
    menu_open: bool,
    selected_tool: WorkbenchTool,
}

struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params: wgpu::Buffer,
    static_instances: wgpu::Buffer,
    dynamic_instances: wgpu::Buffer,
    last_target_size: Option<[u32; 2]>,
    static_key: Option<StaticKey>,
    static_count: u32,
    static_glyph_count: u32,
    dynamic_count: u32,
    dynamic_glyph_count: u32,
    static_writes_total: u64,
    dynamic_writes_total: u64,
    parameter_writes_total: u64,
    static_upload_bytes_total: u64,
    dynamic_upload_bytes_total: u64,
    parameter_upload_bytes_total: u64,
    overflowed: bool,
}

impl UiRenderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, color_format: wgpu::TextureFormat) -> Self {
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_ui_params"),
            contents: bytemuck::bytes_of(&UiParams::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let atlas = glyph_atlas_r8();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("physbox5_ui_glyph_atlas"),
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
            label: Some("physbox5_ui_glyph_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_ui_bind_group_layout"),
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
            label: Some("physbox5_ui_bind_group"),
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
            label: Some("physbox5_ui_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_ui_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("physbox5_ui_pipeline"),
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
        let static_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_ui_static_instances"),
            size: (STATIC_INSTANCE_CAPACITY * std::mem::size_of::<UiInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dynamic_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_ui_dynamic_instances"),
            size: (DYNAMIC_INSTANCE_CAPACITY * std::mem::size_of::<UiInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind_group,
            params,
            static_instances,
            dynamic_instances,
            last_target_size: None,
            static_key: None,
            static_count: 0,
            static_glyph_count: 0,
            dynamic_count: 0,
            dynamic_glyph_count: 0,
            static_writes_total: 0,
            dynamic_writes_total: 0,
            parameter_writes_total: 0,
            static_upload_bytes_total: 0,
            dynamic_upload_bytes_total: 0,
            parameter_upload_bytes_total: 0,
            overflowed: false,
        }
    }

    fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        model: UiFrameModel<'_>,
        menu_open: bool,
        selected_tool: WorkbenchTool,
    ) -> UiReport {
        if self.last_target_size != Some(target.size) {
            let params = UiParams {
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
        let key = StaticKey {
            target_size: target.size,
            scenario: model.scenario,
            menu_open,
            selected_tool,
        };
        if self.static_key != Some(key) {
            let batch = static_batch(target.size, model.scenario, menu_open, selected_tool);
            if !batch.instances.is_empty() {
                let bytes = bytemuck::cast_slice(&batch.instances);
                queue.write_buffer(&self.static_instances, 0, bytes);
                self.static_upload_bytes_total = self
                    .static_upload_bytes_total
                    .saturating_add(bytes.len() as u64);
            }
            self.static_count = batch.instances.len() as u32;
            self.static_glyph_count = batch.glyph_count;
            self.static_writes_total = self.static_writes_total.saturating_add(1);
            self.overflowed |= batch.overflowed;
            self.static_key = Some(key);
        }
        let dynamic = dynamic_batch(target.size, &model);
        if !dynamic.instances.is_empty() {
            let bytes = bytemuck::cast_slice(&dynamic.instances);
            queue.write_buffer(&self.dynamic_instances, 0, bytes);
            self.dynamic_upload_bytes_total = self
                .dynamic_upload_bytes_total
                .saturating_add(bytes.len() as u64);
        }
        self.dynamic_count = dynamic.instances.len() as u32;
        self.dynamic_glyph_count = dynamic.glyph_count;
        self.dynamic_writes_total = self.dynamic_writes_total.saturating_add(1);
        self.overflowed |= dynamic.overflowed;

        let mut draw_calls = 0;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox5_ui_render_pass"),
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
        if self.static_count > 0 {
            pass.set_vertex_buffer(0, self.static_instances.slice(..));
            pass.draw(0..6, 0..self.static_count);
            draw_calls += 1;
        }
        if self.dynamic_count > 0 {
            pass.set_vertex_buffer(0, self.dynamic_instances.slice(..));
            pass.draw(0..6, 0..self.dynamic_count);
            draw_calls += 1;
        }
        drop(pass);

        UiReport {
            version: UI_CONTRACT_VERSION,
            menu_open,
            current_tool: selected_tool.name(),
            current_scene: model.scenario.name(),
            static_instance_capacity: STATIC_INSTANCE_CAPACITY as u32,
            dynamic_instance_capacity: DYNAMIC_INSTANCE_CAPACITY as u32,
            instance_stride_bytes: std::mem::size_of::<UiInstance>() as u32,
            glyph_atlas_bytes: glyph_atlas_bytes(),
            allocated_bytes: glyph_atlas_bytes()
                + std::mem::size_of::<UiParams>() as u64
                + ((STATIC_INSTANCE_CAPACITY + DYNAMIC_INSTANCE_CAPACITY)
                    * std::mem::size_of::<UiInstance>()) as u64,
            static_instances: self.static_count,
            dynamic_instances: self.dynamic_count,
            static_glyphs: self.static_glyph_count,
            dynamic_glyphs: self.dynamic_glyph_count,
            static_writes_total: self.static_writes_total,
            dynamic_writes_total: self.dynamic_writes_total,
            parameter_writes_total: self.parameter_writes_total,
            static_upload_bytes_total: self.static_upload_bytes_total,
            dynamic_upload_bytes_total: self.dynamic_upload_bytes_total,
            parameter_upload_bytes_total: self.parameter_upload_bytes_total,
            render_passes_this_frame: 1,
            draw_calls_this_frame: draw_calls,
            diagnostic_source: model.construction_diagnostic_source,
            diagnostic_age_frames: model.construction_age_frames,
            scenario_activations_total: model.scenario_activations_total,
            scenario_restarts_total: model.scenario_restarts_total,
            overflowed: self.overflowed,
        }
    }
}

struct Batch {
    instances: Vec<UiInstance>,
    capacity: usize,
    glyph_count: u32,
    overflowed: bool,
}

impl Batch {
    fn new(capacity: usize) -> Self {
        Self {
            instances: Vec::with_capacity(capacity),
            capacity,
            glyph_count: 0,
            overflowed: false,
        }
    }

    fn push_quad(&mut self, rect: [f32; 4], color: [f32; 4]) {
        self.push(UiInstance {
            rect,
            uv: [0.0; 4],
            color,
            mode: [0.0; 4],
        });
    }

    fn push_text(&mut self, text: &str, origin: [f32; 2], pixel: f32, color: [f32; 4]) {
        let mut x = origin[0];
        for byte in text.bytes() {
            if byte != b' ' {
                let before = self.instances.len();
                self.push(UiInstance {
                    rect: [
                        x,
                        origin[1],
                        GLYPH_WIDTH as f32 * pixel,
                        GLYPH_HEIGHT as f32 * pixel,
                    ],
                    uv: glyph_uv(byte),
                    color,
                    mode: [1.0, 0.0, 0.0, 0.0],
                });
                if self.instances.len() != before {
                    self.glyph_count = self.glyph_count.saturating_add(1);
                }
            }
            x += GLYPH_CELL_WIDTH as f32 * pixel;
        }
    }

    fn push(&mut self, instance: UiInstance) {
        if self.instances.len() == self.capacity {
            self.overflowed = true;
            return;
        }
        self.instances.push(instance);
    }
}

fn static_batch(
    target_size: [u32; 2],
    scenario: ValidationScenario,
    menu_open: bool,
    selected_tool: WorkbenchTool,
) -> Batch {
    let layout = UiLayout::for_target(target_size);
    let scale = layout.scale;
    let mut batch = Batch::new(STATIC_INSTANCE_CAPACITY);
    let panel = [0.025, 0.045, 0.085, 0.92];
    let button = [0.08, 0.14, 0.22, 0.94];
    let selected = [0.08, 0.38, 0.52, 0.96];
    let text = [0.82, 0.92, 1.0, 1.0];
    let accent = [0.32, 0.86, 1.0, 1.0];
    batch.push_quad(
        layout.menu_button(),
        if menu_open { selected } else { button },
    );
    batch.push_text("MENU", [27.0 * scale, 41.0 * scale], 2.4 * scale, text);
    for tool in WorkbenchTool::ALL {
        let bounds = layout.tool_button(tool);
        batch.push_quad(
            bounds,
            if tool == selected_tool {
                selected
            } else {
                button
            },
        );
        batch.push_text(
            tool.label(),
            [bounds[0] + 7.0 * scale, bounds[1] + 17.0 * scale],
            1.8 * scale,
            text,
        );
    }
    if menu_open {
        batch.push_quad(layout.menu_panel(), panel);
        batch.push_text("SCENES", [28.0 * scale, 98.0 * scale], 2.4 * scale, accent);
        for (index, candidate) in ValidationScenario::ALL.iter().copied().enumerate() {
            let bounds = layout.scenario_button(index);
            batch.push_quad(
                bounds,
                if candidate == scenario {
                    selected
                } else {
                    button
                },
            );
            batch.push_text(
                candidate.name(),
                [bounds[0] + 10.0 * scale, bounds[1] + 9.0 * scale],
                1.8 * scale,
                text,
            );
        }
        let restart = layout.restart_button();
        let recenter = layout.recenter_button();
        batch.push_quad(restart, button);
        batch.push_quad(recenter, button);
        batch.push_text(
            "RESTART",
            [restart[0] + 11.0 * scale, restart[1] + 10.0 * scale],
            1.8 * scale,
            text,
        );
        batch.push_text(
            "RECENTER",
            [recenter[0] + 10.0 * scale, recenter[1] + 10.0 * scale],
            1.8 * scale,
            text,
        );
    }
    batch
}

fn dynamic_batch(target_size: [u32; 2], model: &UiFrameModel<'_>) -> Batch {
    let layout = UiLayout::for_target(target_size);
    let scale = layout.scale;
    let width = 310.0 * scale;
    let height = 176.0 * scale;
    let origin = [
        target_size[0] as f32 - width - 12.0 * scale,
        if target_size[0] < 720 {
            target_size[1] as f32 - height - 12.0 * scale
        } else {
            24.0 * scale
        },
    ];
    let mut batch = Batch::new(DYNAMIC_INSTANCE_CAPACITY);
    batch.push_quad(
        [origin[0], origin[1], width, height],
        [0.015, 0.03, 0.065, 0.80],
    );
    let construction_failures = model.construction_state.map_or(0, |state| {
        state
            .capacity_failures
            .saturating_add(state.deep_overlap_failures)
            .saturating_add(state.erase_miss_failures)
            .saturating_add(state.generation_failures)
    });
    let construction_result =
        model
            .construction_state
            .map_or("WAIT", |state| match state.last_result {
                ConstructionResultV1::None => "READY",
                ConstructionResultV1::Placed => "PLACED",
                ConstructionResultV1::Erased => "ERASED",
                ConstructionResultV1::CapacityFull => "FULL",
                ConstructionResultV1::DeepOverlap => "OVERLAP",
                ConstructionResultV1::EraseMiss => "MISS",
                ConstructionResultV1::GenerationExhausted => "GENFAIL",
            });
    let live_bodies = model
        .construction_state
        .map_or(model.body_count, |state| state.live_bodies);
    let lines = [
        format!("SCENE {}", model.scenario.name()),
        format!(
            "FPS {:.0}",
            model.pacing.presented_frame_hz.clamp(0.0, 999.0)
        ),
        format!(
            "SIM {:.0}% SLIP {:.0}MS",
            (model.pacing.effective_simulation_scale * 100.0).clamp(0.0, 999.0),
            (model.pacing.overload_slipped_wall_seconds_total * 1000.0).clamp(0.0, 999_999.0)
        ),
        format!(
            "STEP {} TOTAL {}",
            model.pacing.completed_outer_steps_this_frame, model.pacing.completed_outer_steps_total
        ),
        format!("BODY {} CONTACT {}", live_bodies, model.contact_count),
        format!(
            "CAP {}KB OVERFLOW {}",
            model.capacity_bytes.div_ceil(1024),
            if model.overflowed { "YES" } else { "NO" }
        ),
        format!("GPU {}", model.adapter_backend),
        format!(
            "BUILD {} FAIL {} AGE {}",
            construction_result,
            construction_failures,
            model
                .construction_age_frames
                .map_or_else(|| "N/A".to_owned(), |age| age.to_string())
        ),
    ];
    let color = [0.72, 0.84, 0.96, 1.0];
    for (index, line) in lines.iter().enumerate() {
        batch.push_text(
            line,
            [
                origin[0] + 10.0 * scale,
                origin[1] + (10.0 + index as f32 * 20.0) * scale,
            ],
            1.7 * scale,
            color,
        );
    }
    batch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PointerEvent, WheelEvent};

    fn pointer(id: u64, phase: PointerPhase, button: PointerButton, at: [f32; 2]) -> InputEvent {
        InputEvent::Pointer(PointerEvent {
            id,
            phase,
            button,
            normalized_position: at,
        })
    }

    #[test]
    fn layout_keeps_touch_targets_on_screen_at_phone_and_desktop_aspects() {
        for target in [[960, 600], [600, 960], [430, 932], [2400, 1080], [932, 430]] {
            let layout = UiLayout::for_target(target);
            for bounds in [
                layout.menu_button(),
                layout.menu_panel(),
                layout.restart_button(),
                layout.recenter_button(),
            ]
            .into_iter()
            .chain(
                WorkbenchTool::ALL
                    .into_iter()
                    .map(|tool| layout.tool_button(tool)),
            )
            .chain(
                ValidationScenario::ALL
                    .iter()
                    .enumerate()
                    .map(|(index, _)| layout.scenario_button(index)),
            ) {
                assert!(bounds[0] >= 0.0 && bounds[1] >= 0.0);
                assert!(bounds[0] + bounds[2] <= target[0] as f32);
                assert!(bounds[1] + bounds[3] <= target[1] as f32);
            }
        }
    }

    #[test]
    fn batches_are_bounded_and_use_one_instance_per_visible_glyph() {
        let static_closed = static_batch(
            [960, 600],
            ValidationScenario::Bootstrap,
            false,
            WorkbenchTool::Grab,
        );
        let static_open = static_batch(
            [960, 600],
            ValidationScenario::Bootstrap,
            true,
            WorkbenchTool::Rectangle,
        );
        assert!(!static_closed.overflowed && !static_open.overflowed);
        assert!(static_open.instances.len() > static_closed.instances.len());
        assert!(static_open.glyph_count > 0);
        assert!(static_open.instances.len() <= STATIC_INSTANCE_CAPACITY);
    }

    #[test]
    fn ui_capture_precedes_world_input_and_secondary_stays_camera_owned() {
        let target = [960, 600];
        let menu = UiLayout::for_target(target).center_normalized("menu", 0);
        let mut captured = BTreeSet::new();
        captured.insert(99);
        let mut menu_open = false;
        let mut selected_tool = WorkbenchTool::Grab;
        let events = [
            pointer(1, PointerPhase::Down, PointerButton::Primary, menu),
            pointer(1, PointerPhase::Up, PointerButton::Primary, menu),
            pointer(2, PointerPhase::Down, PointerButton::Secondary, menu),
            InputEvent::Wheel(WheelEvent {
                normalized_position: menu,
                zoom_steps: 1.0,
            }),
        ];
        let routed = route_events(
            &mut menu_open,
            &mut selected_tool,
            &mut captured,
            &events,
            target,
        );
        assert!(menu_open);
        assert_eq!(routed.events.len(), 2);
        assert!(matches!(routed.events[0], InputEvent::Pointer(_)));
        assert!(matches!(routed.events[1], InputEvent::Wheel(_)));
    }

    #[test]
    fn tool_buttons_are_shared_captured_and_select_exactly_one_tool() {
        let target = [430, 932];
        let layout = UiLayout::for_target(target);
        let rectangle = layout.center_normalized("tool", 2);
        let mut captured = BTreeSet::new();
        let mut menu_open = false;
        let mut selected_tool = WorkbenchTool::Grab;
        let routed = route_events(
            &mut menu_open,
            &mut selected_tool,
            &mut captured,
            &[
                pointer(7, PointerPhase::Down, PointerButton::Contact, rectangle),
                pointer(7, PointerPhase::Up, PointerButton::Contact, rectangle),
            ],
            target,
        );
        assert_eq!(selected_tool, WorkbenchTool::Rectangle);
        assert_eq!(routed.selected_tool, WorkbenchTool::Rectangle);
        assert!(routed.events.is_empty());
        assert!(captured.is_empty());
    }
}
