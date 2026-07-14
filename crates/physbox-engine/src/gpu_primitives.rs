use std::{mem::size_of, sync::mpsc, time::Instant};

use bytemuck::{Pod, Zeroable};
use physbox_contracts::CapacityRecordV1;
use wgpu::util::DeviceExt;

use crate::gpu_context::{
    AdapterDiagnostics, create_instance, request_adapter, request_device_with_features,
};

const WORKGROUP_SIZE: u32 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuPrimitiveKind {
    PrefixScan,
    RadixSort,
    Unique,
    Compaction,
}

impl GpuPrimitiveKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::PrefixScan => "prefix-scan",
            Self::RadixSort => "radix-sort",
            Self::Unique => "unique",
            Self::Compaction => "compaction",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GpuPrimitiveBenchmarkOutput {
    pub primitive: GpuPrimitiveKind,
    pub adapter: AdapterDiagnostics,
    pub element_count: u32,
    pub warmup_steps: u32,
    pub samples: u32,
    pub gpu_step_seconds: Option<Vec<f64>>,
    pub host_completion_seconds: Vec<f64>,
    pub cpu_encode_submit_seconds: Vec<f64>,
    pub timestamp_period_nanoseconds: Option<f32>,
    pub capacities: Vec<CapacityRecordV1>,
    pub dispatches: u32,
    pub output_elements: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParamsRaw {
    count: u32,
    block_count: u32,
    shift: u32,
    padding: u32,
}

#[derive(Clone, Copy)]
enum PipelineKind {
    ScanBlocks,
    ScanBlockTotals,
    AddBlockOffsets,
    RadixHistograms,
    RadixPrefix,
    RadixScatter,
    UniqueFlags,
    CompactionFlags,
    FlaggedScatter,
}

struct Operation {
    pipeline: PipelineKind,
    bind_group: usize,
    workgroups: u32,
}

struct PrimitiveLab {
    source: wgpu::Buffer,
    destination: wgpu::Buffer,
    metadata: wgpu::Buffer,
    scan_blocks: wgpu::ComputePipeline,
    scan_block_totals: wgpu::ComputePipeline,
    add_block_offsets: wgpu::ComputePipeline,
    radix_histograms: wgpu::ComputePipeline,
    radix_prefix: wgpu::ComputePipeline,
    radix_scatter: wgpu::ComputePipeline,
    unique_flags: wgpu::ComputePipeline,
    compaction_flags: wgpu::ComputePipeline,
    flagged_scatter: wgpu::ComputePipeline,
    bind_groups: Vec<wgpu::BindGroup>,
    operations: Vec<Operation>,
    input: Vec<u32>,
    expected: Vec<u32>,
    output_count: u32,
    capacities: Vec<CapacityRecordV1>,
}

impl PrimitiveLab {
    fn new(device: &wgpu::Device, primitive: GpuPrimitiveKind, count: u32) -> Self {
        let block_count = count.div_ceil(WORKGROUP_SIZE);
        let input = primitive_input(primitive, count);
        let expected = expected_output(primitive, &input);
        let source = storage_init(device, "primitive_source", &input);
        let destination = storage_empty(device, "primitive_destination", count);
        let aux_a = storage_empty(device, "primitive_aux_a", count);
        let aux_b = storage_empty(device, "primitive_aux_b", count);
        let blocks = storage_empty(device, "primitive_blocks", block_count * 16);
        let metadata = storage_empty(device, "primitive_metadata", 1);
        let dummy_a = storage_empty(device, "primitive_dummy_a", 1);
        let dummy_b = storage_empty(device, "primitive_dummy_b", 1);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("primitive_bind_group_layout"),
            entries: &[
                storage_entry(0),
                storage_entry(1),
                storage_entry(2),
                storage_entry(3),
                storage_entry(4),
                storage_entry(5),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<ParamsRaw>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("portable_gpu_primitives"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_primitives.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("primitive_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = |label, entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let mut bind_groups = Vec::new();
        let mut operations = Vec::new();
        let mut add_group = |source_buffer: &wgpu::Buffer,
                             destination_buffer: &wgpu::Buffer,
                             aux_a_buffer: &wgpu::Buffer,
                             aux_b_buffer: &wgpu::Buffer,
                             shift: u32| {
            let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("primitive_parameters"),
                contents: bytemuck::bytes_of(&ParamsRaw {
                    count,
                    block_count,
                    shift,
                    padding: 0,
                }),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let index = bind_groups.len();
            bind_groups.push(bind_group(
                device,
                &layout,
                source_buffer,
                destination_buffer,
                aux_a_buffer,
                aux_b_buffer,
                &blocks,
                &metadata,
                &params,
            ));
            index
        };
        match primitive {
            GpuPrimitiveKind::PrefixScan => {
                let group = add_group(&source, &destination, &aux_a, &aux_b, 0);
                scan_operations(&mut operations, group, block_count);
            }
            GpuPrimitiveKind::RadixSort => {
                for digit in 0..8 {
                    let (input_buffer, output_buffer) = if digit % 2 == 0 {
                        (&source, &destination)
                    } else {
                        (&destination, &source)
                    };
                    let group = add_group(input_buffer, output_buffer, &aux_a, &aux_b, digit * 4);
                    operations.extend([
                        Operation {
                            pipeline: PipelineKind::RadixHistograms,
                            bind_group: group,
                            workgroups: block_count,
                        },
                        Operation {
                            pipeline: PipelineKind::RadixPrefix,
                            bind_group: group,
                            workgroups: 1,
                        },
                        Operation {
                            pipeline: PipelineKind::RadixScatter,
                            bind_group: group,
                            workgroups: block_count,
                        },
                    ]);
                }
            }
            GpuPrimitiveKind::Unique | GpuPrimitiveKind::Compaction => {
                let flags_group = add_group(&source, &destination, &aux_a, &aux_b, 0);
                operations.push(Operation {
                    pipeline: if primitive == GpuPrimitiveKind::Unique {
                        PipelineKind::UniqueFlags
                    } else {
                        PipelineKind::CompactionFlags
                    },
                    bind_group: flags_group,
                    workgroups: block_count,
                });
                let scan_group = add_group(&aux_a, &aux_b, &dummy_a, &dummy_b, 0);
                scan_operations(&mut operations, scan_group, block_count);
                operations.push(Operation {
                    pipeline: PipelineKind::FlaggedScatter,
                    bind_group: flags_group,
                    workgroups: block_count,
                });
            }
        }
        let output_count = expected.len() as u32;
        let uses_flags = matches!(
            primitive,
            GpuPrimitiveKind::Unique | GpuPrimitiveKind::Compaction
        );
        let block_capacity = u64::from(block_count) * 16;
        let block_used = if primitive == GpuPrimitiveKind::RadixSort {
            block_capacity
        } else {
            u64::from(block_count)
        };
        let scratch_capacity = u64::from(count) * 2;
        let capacities = vec![
            capacity("primitive-blocks", block_capacity, block_used),
            capacity(
                "primitive-elements",
                u64::from(count) * 2,
                u64::from(count) * 2,
            ),
            capacity("primitive-metadata", 1, u64::from(uses_flags)),
            capacity(
                "primitive-scratch",
                scratch_capacity,
                if uses_flags { scratch_capacity } else { 0 },
            ),
        ];
        Self {
            source,
            destination,
            metadata,
            scan_blocks: pipeline("primitive_scan_blocks", "scan_blocks"),
            scan_block_totals: pipeline("primitive_scan_block_totals", "scan_block_totals"),
            add_block_offsets: pipeline("primitive_add_block_offsets", "add_block_offsets"),
            radix_histograms: pipeline("primitive_radix_histograms", "radix_histograms"),
            radix_prefix: pipeline("primitive_radix_prefix", "radix_prefix"),
            radix_scatter: pipeline("primitive_radix_scatter", "radix_scatter"),
            unique_flags: pipeline("primitive_unique_flags", "unique_flags"),
            compaction_flags: pipeline("primitive_compaction_flags", "compaction_flags"),
            flagged_scatter: pipeline("primitive_flagged_scatter", "flagged_scatter"),
            bind_groups,
            operations,
            input,
            expected,
            output_count,
            capacities,
        }
    }

    fn reset(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.source, 0, bytemuck::cast_slice(&self.input));
        queue.write_buffer(&self.metadata, 0, bytemuck::bytes_of(&0_u32));
    }

    fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        timestamps: Option<(&wgpu::QuerySet, u32, u32)>,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("portable_gpu_primitive_passes"),
            timestamp_writes: timestamps.map(|(query_set, beginning, end)| {
                wgpu::ComputePassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(beginning),
                    end_of_pass_write_index: Some(end),
                }
            }),
        });
        for operation in &self.operations {
            pass.set_pipeline(match operation.pipeline {
                PipelineKind::ScanBlocks => &self.scan_blocks,
                PipelineKind::ScanBlockTotals => &self.scan_block_totals,
                PipelineKind::AddBlockOffsets => &self.add_block_offsets,
                PipelineKind::RadixHistograms => &self.radix_histograms,
                PipelineKind::RadixPrefix => &self.radix_prefix,
                PipelineKind::RadixScatter => &self.radix_scatter,
                PipelineKind::UniqueFlags => &self.unique_flags,
                PipelineKind::CompactionFlags => &self.compaction_flags,
                PipelineKind::FlaggedScatter => &self.flagged_scatter,
            });
            pass.set_bind_group(0, &self.bind_groups[operation.bind_group], &[]);
            pass.dispatch_workgroups(operation.workgroups, 1, 1);
        }
    }

    fn result_buffer(&self, primitive: GpuPrimitiveKind) -> &wgpu::Buffer {
        if primitive == GpuPrimitiveKind::RadixSort {
            &self.source
        } else {
            &self.destination
        }
    }
}

pub async fn benchmark_gpu_primitive(
    primitive: GpuPrimitiveKind,
    element_count: u32,
    warmup_steps: u32,
    samples: u32,
) -> Result<GpuPrimitiveBenchmarkOutput, String> {
    if element_count == 0 || warmup_steps == 0 || samples == 0 {
        return Err(
            "GPU primitive benchmark requires non-zero elements, warmup, and samples".to_owned(),
        );
    }
    let query_count = samples
        .checked_mul(2)
        .ok_or_else(|| "GPU primitive benchmark query count overflow".to_owned())?;
    let instance = create_instance();
    let adapter = request_adapter(&instance, None).await?;
    let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
    let timestamps_available = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let features = if timestamps_available {
        wgpu::Features::TIMESTAMP_QUERY
    } else {
        wgpu::Features::empty()
    };
    let (device, queue) = request_device_with_features(&adapter, features).await?;
    let lab = PrimitiveLab::new(&device, primitive, element_count);
    let queries = timestamp_resources(&device, query_count, timestamps_available);
    for _ in 0..warmup_steps {
        lab.reset(&queue);
        poll(&device, "primitive warmup reset")?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("primitive_benchmark_warmup"),
        });
        lab.encode(&mut encoder, None);
        queue.submit([encoder.finish()]);
        poll(&device, "primitive warmup")?;
    }
    let mut host_completion_seconds = Vec::with_capacity(samples as usize);
    let mut cpu_encode_submit_seconds = Vec::with_capacity(samples as usize);
    for sample in 0..samples {
        lab.reset(&queue);
        poll(&device, "primitive sample reset")?;
        let started = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("primitive_benchmark_sample"),
        });
        let timestamps = queries
            .as_ref()
            .map(|(set, _, _)| (set, sample * 2, sample * 2 + 1));
        lab.encode(&mut encoder, timestamps);
        queue.submit([encoder.finish()]);
        cpu_encode_submit_seconds.push(started.elapsed().as_secs_f64());
        poll(&device, "primitive sample")?;
        host_completion_seconds.push(started.elapsed().as_secs_f64());
    }
    let (gpu_step_seconds, timestamp_period_nanoseconds) =
        resolve_timestamps(&device, &queue, queries, query_count)?;
    validate_output(&device, &queue, &lab, primitive, element_count)?;
    Ok(GpuPrimitiveBenchmarkOutput {
        primitive,
        adapter: diagnostics,
        element_count,
        warmup_steps,
        samples,
        gpu_step_seconds,
        host_completion_seconds,
        cpu_encode_submit_seconds,
        timestamp_period_nanoseconds,
        capacities: lab.capacities,
        dispatches: lab.operations.len() as u32,
        output_elements: lab.output_count,
    })
}

fn scan_operations(operations: &mut Vec<Operation>, bind_group: usize, block_count: u32) {
    operations.extend([
        Operation {
            pipeline: PipelineKind::ScanBlocks,
            bind_group,
            workgroups: block_count,
        },
        Operation {
            pipeline: PipelineKind::ScanBlockTotals,
            bind_group,
            workgroups: 1,
        },
        Operation {
            pipeline: PipelineKind::AddBlockOffsets,
            bind_group,
            workgroups: block_count,
        },
    ]);
}

fn primitive_input(primitive: GpuPrimitiveKind, count: u32) -> Vec<u32> {
    match primitive {
        GpuPrimitiveKind::Unique => (0..count).map(|index| index / 3).collect(),
        GpuPrimitiveKind::Compaction => (0..count)
            .map(|index| if index % 4 == 0 { 0 } else { index + 1 })
            .collect(),
        GpuPrimitiveKind::PrefixScan => (0..count)
            .map(|index| mix(index.wrapping_add(1)) & 7)
            .collect(),
        GpuPrimitiveKind::RadixSort => (0..count).map(|index| mix(index.wrapping_add(1))).collect(),
    }
}

fn expected_output(primitive: GpuPrimitiveKind, input: &[u32]) -> Vec<u32> {
    match primitive {
        GpuPrimitiveKind::PrefixScan => {
            let mut running = 0_u32;
            input
                .iter()
                .map(|value| {
                    let output = running;
                    running = running.wrapping_add(*value);
                    output
                })
                .collect()
        }
        GpuPrimitiveKind::RadixSort => {
            let mut output = input.to_vec();
            output.sort_unstable();
            output
        }
        GpuPrimitiveKind::Unique => {
            let mut output = input.to_vec();
            output.dedup();
            output
        }
        GpuPrimitiveKind::Compaction => input.iter().copied().filter(|value| *value != 0).collect(),
    }
}

fn mix(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

fn timestamp_resources(
    device: &wgpu::Device,
    query_count: u32,
    available: bool,
) -> Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer)> {
    available.then(|| {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("primitive_benchmark_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: query_count,
        });
        let bytes = u64::from(query_count) * 8;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("primitive_timestamp_resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("primitive_timestamp_readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        (query_set, resolve, readback)
    })
}

fn resolve_timestamps(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    queries: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer)>,
    query_count: u32,
) -> Result<(Option<Vec<f64>>, Option<f32>), String> {
    let Some((query_set, resolve, readback)) = queries else {
        return Ok((None, None));
    };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("primitive_timestamp_resolve_encoder"),
    });
    encoder.resolve_query_set(&query_set, 0..query_count, &resolve, 0);
    encoder.copy_buffer_to_buffer(&resolve, 0, &readback, 0, u64::from(query_count) * 8);
    queue.submit([encoder.finish()]);
    let values = map_u64(device, &readback, query_count as usize)?;
    let period = queue.get_timestamp_period();
    let seconds = values
        .chunks_exact(2)
        .map(|pair| pair[1].saturating_sub(pair[0]) as f64 * f64::from(period) * 1.0e-9)
        .collect();
    Ok((Some(seconds), Some(period)))
}

fn validate_output(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    lab: &PrimitiveLab,
    primitive: GpuPrimitiveKind,
    element_count: u32,
) -> Result<(), String> {
    let result_bytes = u64::from(element_count) * 4;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("primitive_validation_readback"),
        size: result_bytes + 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("primitive_validation_encoder"),
    });
    encoder.copy_buffer_to_buffer(lab.result_buffer(primitive), 0, &readback, 0, result_bytes);
    encoder.copy_buffer_to_buffer(&lab.metadata, 0, &readback, result_bytes, 4);
    queue.submit([encoder.finish()]);
    let values = map_u32(device, &readback, element_count as usize + 1)?;
    let output_count = if matches!(
        primitive,
        GpuPrimitiveKind::Unique | GpuPrimitiveKind::Compaction
    ) {
        values[element_count as usize] as usize
    } else {
        element_count as usize
    };
    if output_count != lab.expected.len() || values[..output_count] != lab.expected {
        return Err(format!(
            "{} GPU output failed final validation",
            primitive.name()
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &wgpu::Buffer,
    destination: &wgpu::Buffer,
    aux_a: &wgpu::Buffer,
    aux_b: &wgpu::Buffer,
    blocks: &wgpu::Buffer,
    metadata: &wgpu::Buffer,
    params: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let buffers = [source, destination, aux_a, aux_b, blocks, metadata];
    let mut entries = buffers
        .iter()
        .enumerate()
        .map(|(binding, buffer)| wgpu::BindGroupEntry {
            binding: binding as u32,
            resource: buffer.as_entire_binding(),
        })
        .collect::<Vec<_>>();
    entries.push(wgpu::BindGroupEntry {
        binding: 6,
        resource: params.as_entire_binding(),
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("primitive_bind_group"),
        layout,
        entries: &entries,
    })
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_init(device: &wgpu::Device, label: &str, values: &[u32]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(values),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    })
}

fn storage_empty(device: &wgpu::Device, label: &str, count: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: u64::from(count) * 4,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn capacity(class: &str, elements: u64, used: u64) -> CapacityRecordV1 {
    CapacityRecordV1 {
        class: class.to_owned(),
        capacity_elements: elements,
        used_elements: used,
        allocated_bytes: elements * 4,
        high_water_bytes: used * 4,
        overflowed: false,
    }
}

fn poll(device: &wgpu::Device, context: &str) -> Result<(), String> {
    device
        .poll(wgpu::PollType::Wait)
        .map(|_| ())
        .map_err(|error| format!("poll GPU {context}: {error}"))
}

fn map_u32(device: &wgpu::Device, buffer: &wgpu::Buffer, count: usize) -> Result<Vec<u32>, String> {
    map_bytes(device, buffer, count * size_of::<u32>())
        .map(|bytes| bytemuck::cast_slice::<u8, u32>(&bytes).to_vec())
}

fn map_u64(device: &wgpu::Device, buffer: &wgpu::Buffer, count: usize) -> Result<Vec<u64>, String> {
    map_bytes(device, buffer, count * size_of::<u64>())
        .map(|bytes| bytemuck::cast_slice::<u8, u64>(&bytes).to_vec())
}

fn map_bytes(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    bytes: usize,
) -> Result<Vec<u8>, String> {
    let slice = buffer.slice(..bytes as u64);
    let (sender, receiver) = mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result.map_err(|error| error.to_string()));
    });
    poll(device, "primitive readback")?;
    receiver
        .recv()
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("map GPU primitive readback: {error}"))?;
    let mapped = slice.get_mapped_range();
    let values = mapped.to_vec();
    drop(mapped);
    buffer.unmap();
    Ok(values)
}
