//! Observation-only connected components for the generated contact graph.
//!
//! This module is deliberately outside the product graph. S26 uses the scalar
//! recurrence as an independent executable specification for an opt-in GPU
//! observer; no label produced here is a sleep or solver input.

const INVALID_LABEL: u32 = u32::MAX;

#[cfg(feature = "solver-lab")]
pub(crate) const CONTACT_COMPONENT_PHASE_BOUND_VERSION: &str =
    "physbox5-contact-component-phase-bound-v1";

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScalarComponentObservation {
    pub labels: Vec<u32>,
    pub phases_executed: u32,
    pub last_changed_phase: u32,
    pub converged: bool,
}

/// Returns the fixed hook/shortcut phase count encoded by the GPU observer.
///
/// A hook only lowers a parent, and the following shortcut replaces every
/// parent by its grandparent. With roots hooked from the greater label to the
/// lesser label, every two phases at least double the already-reached prefix
/// of a worst-case ordered path. `2 * ceil(log2(N)) + 1` is therefore a
/// conservative finite bound for `N` reserved slots. The GPU independently
/// rejects any result that still has a non-root label or an edge crossing two
/// labels, so this bound can never silently admit a partial component.
pub(crate) fn component_phase_bound(body_capacity: u32) -> Result<u32, String> {
    if body_capacity == 0 {
        return Err("contact-component body capacity must be positive".to_owned());
    }
    let logarithm = u32::BITS - body_capacity.saturating_sub(1).leading_zeros();
    logarithm
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "contact-component phase bound overflowed".to_owned())
}

pub(crate) fn reference_component_labels(
    live_dynamic: &[bool],
    edges: &[(u32, u32)],
) -> Result<Vec<u32>, String> {
    validate_edges(live_dynamic, edges)?;
    let mut adjacency = vec![Vec::new(); live_dynamic.len()];
    for &(a, b) in edges {
        adjacency[a as usize].push(b);
        adjacency[b as usize].push(a);
    }
    let mut labels = vec![INVALID_LABEL; live_dynamic.len()];
    for slot in 0..live_dynamic.len() {
        if !live_dynamic[slot] || labels[slot] != INVALID_LABEL {
            continue;
        }
        let root = slot as u32;
        labels[slot] = root;
        let mut stack = vec![root];
        while let Some(body) = stack.pop() {
            for &neighbor in &adjacency[body as usize] {
                if labels[neighbor as usize] == INVALID_LABEL {
                    labels[neighbor as usize] = root;
                    stack.push(neighbor);
                }
            }
        }
    }
    Ok(labels)
}

#[cfg(test)]
pub(crate) fn observe_components_scalar(
    live_dynamic: &[bool],
    edges: &[(u32, u32)],
) -> Result<ScalarComponentObservation, String> {
    validate_edges(live_dynamic, edges)?;
    let capacity = u32::try_from(live_dynamic.len())
        .map_err(|_| "contact-component body capacity exceeds u32".to_owned())?;
    let phase_bound = component_phase_bound(capacity)?;
    let mut parents = live_dynamic
        .iter()
        .enumerate()
        .map(|(slot, &live)| if live { slot as u32 } else { INVALID_LABEL })
        .collect::<Vec<_>>();
    let mut last_changed_phase = 0;

    for phase in 1..=phase_bound {
        let before_hook = parents.clone();
        for &(a, b) in edges {
            let parent_a = before_hook[a as usize];
            let parent_b = before_hook[b as usize];
            let high = parent_a.max(parent_b) as usize;
            let low = parent_a.min(parent_b);
            parents[high] = parents[high].min(low);
        }

        let before_shortcut = parents.clone();
        for (slot, parent) in parents.iter_mut().enumerate() {
            if !live_dynamic[slot] {
                continue;
            }
            *parent = before_shortcut[before_shortcut[slot] as usize];
        }
        if parents != before_hook {
            last_changed_phase = phase;
        }
    }

    let converged = live_dynamic.iter().enumerate().all(|(slot, &live)| {
        !live || parents[slot] != INVALID_LABEL && parents[parents[slot] as usize] == parents[slot]
    }) && edges
        .iter()
        .all(|&(a, b)| parents[a as usize] == parents[b as usize]);

    Ok(ScalarComponentObservation {
        labels: parents,
        phases_executed: phase_bound,
        last_changed_phase,
        converged,
    })
}

fn validate_edges(live_dynamic: &[bool], edges: &[(u32, u32)]) -> Result<(), String> {
    if live_dynamic.is_empty() {
        return Err("contact-component body capacity must be positive".to_owned());
    }
    for &(a, b) in edges {
        let Some(&a_live) = live_dynamic.get(a as usize) else {
            return Err(format!(
                "contact-component edge endpoint {a} exceeds capacity"
            ));
        };
        let Some(&b_live) = live_dynamic.get(b as usize) else {
            return Err(format!(
                "contact-component edge endpoint {b} exceeds capacity"
            ));
        };
        if a == b || !a_live || !b_live {
            return Err(format!(
                "contact-component edge ({a}, {b}) is not a live dynamic pair"
            ));
        }
    }
    Ok(())
}

#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
mod gpu {
    use std::{mem::size_of, time::Instant};

    use bytemuck::{Pod, Zeroable};
    use physbox_contracts::{ScenarioV1, Validate, canonical_hash};
    use wgpu::util::DeviceExt;

    use super::{
        CONTACT_COMPONENT_PHASE_BOUND_VERSION, INVALID_LABEL, component_phase_bound,
        reference_component_labels,
    };
    use crate::{
        AdapterDiagnostics, BODY_COMMAND_CAPACITY, GpuBodyStorage,
        body_floor_solver::{
            GpuGatherTopology, GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1, PointInputRaw,
        },
        generated_contacts::{GpuContactPersistenceTopology, GpuGeneratedContactGraph},
    };

    const WORKGROUP_SIZE: u32 = 64;
    const POINT_DISPATCH_64_OFFSET: u64 = 10 * size_of::<u32>() as u64;
    const STAT_WORDS: usize = 16;
    const OUTPUT_VERSION: &str = "physbox5-contact-component-observation-v1";

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Pod, Zeroable)]
    struct ParamsRaw {
        counts: [u32; 4],
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Pod, Zeroable)]
    struct StatsRaw {
        values: [u32; STAT_WORDS],
    }

    #[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct GpuContactComponentStatsV1 {
        pub dynamic_body_count: u32,
        pub unique_edge_count: u32,
        pub hook_attempts: u32,
        pub hook_successes: u32,
        pub shortcut_writes: u32,
        pub last_changed_phase: u32,
        pub component_count: u32,
        pub largest_component_size: u32,
        pub invalid_root_count: u32,
        pub cross_label_edge_count: u32,
        pub invalid_edge_count: u32,
        pub phase_bound: u32,
        pub body_capacity: u32,
        pub point_capacity: u32,
    }

    impl From<StatsRaw> for GpuContactComponentStatsV1 {
        fn from(raw: StatsRaw) -> Self {
            Self {
                dynamic_body_count: raw.values[0],
                unique_edge_count: raw.values[1],
                hook_attempts: raw.values[2],
                hook_successes: raw.values[3],
                shortcut_writes: raw.values[4],
                last_changed_phase: raw.values[5],
                component_count: raw.values[6],
                largest_component_size: raw.values[7],
                invalid_root_count: raw.values[8],
                cross_label_edge_count: raw.values[9],
                invalid_edge_count: raw.values[10],
                phase_bound: raw.values[11],
                body_capacity: raw.values[12],
                point_capacity: raw.values[13],
            }
        }
    }

    #[derive(Clone, Debug, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct GpuContactComponentObservationV1 {
        pub version: &'static str,
        pub phase_bound_version: &'static str,
        pub adapter: AdapterDiagnostics,
        pub source_scenario_hash: String,
        pub body_count: u32,
        pub body_capacity: u32,
        pub completed_steps: u32,
        pub samples: u32,
        pub phase_bound: u32,
        pub observer_dispatches_per_sample: u32,
        pub observer_allocated_bytes: u64,
        pub product_dispatches_per_step: u32,
        pub product_allocated_bytes: u64,
        pub gpu_component_seconds: Option<Vec<f64>>,
        pub gpu_physics_seconds: Option<Vec<f64>>,
        pub gpu_end_to_end_seconds: Option<Vec<f64>>,
        pub host_completion_seconds: Vec<f64>,
        pub timestamp_period_nanoseconds: Option<f32>,
        pub labels: Vec<u32>,
        pub cpu_labels: Vec<u32>,
        pub edges: Vec<[u32; 2]>,
        pub stats: Vec<GpuContactComponentStatsV1>,
        pub replay_exact: bool,
        pub exact: bool,
    }

    struct ObserverPipelines {
        initialize: wgpu::ComputePipeline,
        hook: wgpu::ComputePipeline,
        shortcut: wgpu::ComputePipeline,
        finalize: wgpu::ComputePipeline,
        validate_edges: wgpu::ComputePipeline,
    }

    struct GpuContactComponentObserver {
        pipelines: ObserverPipelines,
        group: wgpu::BindGroup,
        parents: wgpu::Buffer,
        sizes: wgpu::Buffer,
        stats: wgpu::Buffer,
        _params: wgpu::Buffer,
        params_stride: u32,
        body_capacity: u32,
        phase_bound: u32,
    }

    impl GpuContactComponentObserver {
        fn new(
            device: &wgpu::Device,
            storage: &GpuBodyStorage,
            solver_points: &wgpu::Buffer,
            body_capacity: u32,
            point_capacity: u32,
        ) -> Result<Self, String> {
            let phase_bound = component_phase_bound(body_capacity)?;
            let params_stride = device.limits().min_uniform_buffer_offset_alignment.max(16);
            let params_count = phase_bound
                .checked_add(1)
                .ok_or_else(|| "contact-component parameter count overflowed".to_owned())?;
            let params_bytes = params_stride
                .checked_mul(params_count)
                .ok_or_else(|| "contact-component parameter bytes overflowed".to_owned())?;
            let mut parameter_data = vec![0_u8; params_bytes as usize];
            for phase in 0..=phase_bound {
                let value = ParamsRaw {
                    counts: [body_capacity, point_capacity, phase, phase_bound],
                };
                let start = phase as usize * params_stride as usize;
                parameter_data[start..start + size_of::<ParamsRaw>()]
                    .copy_from_slice(bytemuck::bytes_of(&value));
            }

            let storage_buffer = |label, elements: u32| -> Result<wgpu::Buffer, String> {
                let size = u64::from(elements)
                    .checked_mul(size_of::<u32>() as u64)
                    .ok_or_else(|| format!("{label} byte count overflowed"))?;
                Ok(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size,
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_SRC
                        | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }))
            };
            let parents = storage_buffer("physbox5_contact_component_parents", body_capacity)?;
            let sizes = storage_buffer("physbox5_contact_component_sizes", body_capacity)?;
            let stats = storage_buffer("physbox5_contact_component_stats", STAT_WORDS as u32)?;
            let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("physbox5_contact_component_params"),
                contents: &parameter_data,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("physbox5_contact_component_layout"),
                entries: &[
                    storage_layout_entry(0, true),
                    storage_layout_entry(1, true),
                    storage_layout_entry(2, false),
                    storage_layout_entry(3, false),
                    storage_layout_entry(4, false),
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: wgpu::BufferSize::new(size_of::<ParamsRaw>() as u64),
                        },
                        count: None,
                    },
                ],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_contact_component_pipeline_layout"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("physbox5_contact_component_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("contact_components.wgsl").into()),
            });
            let pipeline = |entry_point| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry_point),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some(entry_point),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                })
            };
            let pipelines = ObserverPipelines {
                initialize: pipeline("initialize"),
                hook: pipeline("hook"),
                shortcut: pipeline("shortcut"),
                finalize: pipeline("finalize"),
                validate_edges: pipeline("validate_edges"),
            };
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("physbox5_contact_component_group"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: storage.metadata_buffer().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: solver_points.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: parents.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: sizes.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: stats.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &params,
                            offset: 0,
                            size: wgpu::BufferSize::new(size_of::<ParamsRaw>() as u64),
                        }),
                    },
                ],
            });
            Ok(Self {
                pipelines,
                group,
                parents,
                sizes,
                stats,
                _params: params,
                params_stride,
                body_capacity,
                phase_bound,
            })
        }

        fn encode(
            &self,
            encoder: &mut wgpu::CommandEncoder,
            point_capacity_state: &wgpu::Buffer,
            timestamps: Option<(&wgpu::QuerySet, u32, u32)>,
        ) {
            encoder.clear_buffer(&self.sizes, 0, None);
            encoder.clear_buffer(&self.stats, 0, None);
            self.direct_pass(
                encoder,
                "physbox5_contact_component_initialize",
                &self.pipelines.initialize,
                0,
                timestamps.map(|(set, begin, _)| (set, Some(begin), None)),
            );
            for phase in 1..=self.phase_bound {
                self.indirect_pass(
                    encoder,
                    "physbox5_contact_component_hook",
                    &self.pipelines.hook,
                    phase,
                    point_capacity_state,
                    None,
                );
                self.direct_pass(
                    encoder,
                    "physbox5_contact_component_shortcut",
                    &self.pipelines.shortcut,
                    phase,
                    None,
                );
            }
            self.direct_pass(
                encoder,
                "physbox5_contact_component_finalize",
                &self.pipelines.finalize,
                0,
                None,
            );
            self.indirect_pass(
                encoder,
                "physbox5_contact_component_validate_edges",
                &self.pipelines.validate_edges,
                0,
                point_capacity_state,
                timestamps.map(|(set, _, end)| (set, None, Some(end))),
            );
        }

        fn direct_pass(
            &self,
            encoder: &mut wgpu::CommandEncoder,
            label: &'static str,
            pipeline: &wgpu::ComputePipeline,
            phase: u32,
            timestamp: Option<(&wgpu::QuerySet, Option<u32>, Option<u32>)>,
        ) {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: timestamp.map(|(query_set, beginning, end)| {
                    wgpu::ComputePassTimestampWrites {
                        query_set,
                        beginning_of_pass_write_index: beginning,
                        end_of_pass_write_index: end,
                    }
                }),
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.group, &[phase * self.params_stride]);
            pass.dispatch_workgroups(self.body_capacity.div_ceil(WORKGROUP_SIZE), 1, 1);
        }

        #[allow(clippy::too_many_arguments)]
        fn indirect_pass(
            &self,
            encoder: &mut wgpu::CommandEncoder,
            label: &'static str,
            pipeline: &wgpu::ComputePipeline,
            phase: u32,
            point_capacity_state: &wgpu::Buffer,
            timestamp: Option<(&wgpu::QuerySet, Option<u32>, Option<u32>)>,
        ) {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: timestamp.map(|(query_set, beginning, end)| {
                    wgpu::ComputePassTimestampWrites {
                        query_set,
                        beginning_of_pass_write_index: beginning,
                        end_of_pass_write_index: end,
                    }
                }),
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.group, &[phase * self.params_stride]);
            pass.dispatch_workgroups_indirect(point_capacity_state, POINT_DISPATCH_64_OFFSET);
        }

        fn dispatches_per_sample(&self) -> u32 {
            self.phase_bound * 2 + 3
        }

        fn allocated_bytes(&self) -> u64 {
            self.parents.size() + self.sizes.size() + self.stats.size() + self._params.size()
        }
    }

    fn storage_layout_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn observe_gpu_contact_components(
        scenario: &ScenarioV1,
        body_capacity: u32,
        completed_steps: u32,
        warmup_samples: u32,
        samples: u32,
    ) -> Result<GpuContactComponentObservationV1, String> {
        if completed_steps == 0 || completed_steps > scenario.requested_steps {
            return Err("component observation steps must lie inside the scenario".to_owned());
        }
        if samples == 0 {
            return Err("component observation requires at least one sample".to_owned());
        }
        scenario.validate().map_err(|error| error.to_string())?;
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "component observation body count exceeds u32".to_owned())?;
        if body_capacity < body_count {
            return Err(format!(
                "component observation capacity {body_capacity} cannot hold {body_count} bodies"
            ));
        }

        let instance = crate::gpu_context::create_instance();
        let adapter = crate::gpu_context::request_adapter(&instance, None).await?;
        let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let timestamps_available = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let required_features = if timestamps_available {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) =
            crate::gpu_context::request_device_with_features(&adapter, required_features).await?;
        let storage = GpuBodyStorage::new(
            &device,
            body_capacity,
            BODY_COMMAND_CAPACITY,
            &scenario.bodies,
        )?;
        let reset = crate::body_state::GpuBodyResetData::new(body_capacity, &scenario.bodies)?;
        let graph = GpuGeneratedContactGraph::new_with_topologies_execution_and_scheduling(
            &device,
            &storage,
            scenario,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::selected_for_body_capacity(body_capacity),
        )?;
        let (solver_points, point_capacity_state, point_capacity) =
            graph.contact_component_sources();
        let observer = GpuContactComponentObserver::new(
            &device,
            &storage,
            solver_points,
            body_capacity,
            point_capacity,
        )?;

        let query_count = samples
            .checked_mul(6)
            .ok_or_else(|| "component observation timestamp count overflowed".to_owned())?;
        let query_resources = timestamps_available.then(|| {
            let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("physbox5_contact_component_timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: query_count,
            });
            let bytes = u64::from(query_count) * size_of::<u64>() as u64;
            let resolve = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("physbox5_contact_component_timestamp_resolve"),
                size: bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let readback = readback_buffer(
                &device,
                "physbox5_contact_component_timestamp_readback",
                bytes,
            );
            (query_set, resolve, readback)
        });
        let label_stride = u64::from(body_capacity) * size_of::<u32>() as u64;
        let stats_stride = size_of::<StatsRaw>() as u64;
        let labels_readback = readback_buffer(
            &device,
            "physbox5_contact_component_label_readback",
            label_stride * u64::from(samples),
        );
        let stats_readback = readback_buffer(
            &device,
            "physbox5_contact_component_stats_readback",
            stats_stride * u64::from(samples),
        );
        let points_readback = readback_buffer(
            &device,
            "physbox5_contact_component_point_readback",
            solver_points.size(),
        );
        let capacity_readback = readback_buffer(
            &device,
            "physbox5_contact_component_capacity_readback",
            point_capacity_state.size(),
        );

        for _ in 0..warmup_samples {
            encode_observation_sample(
                &device,
                &queue,
                &storage,
                &reset,
                &graph,
                &observer,
                point_capacity_state,
                completed_steps,
                None,
                None,
            )?;
        }

        let mut host_completion_seconds = Vec::with_capacity(samples as usize);
        for sample in 0..samples {
            let started = Instant::now();
            let timestamps = query_resources
                .as_ref()
                .map(|(set, _, _)| (set, sample * 6));
            let copies = SampleCopies {
                labels: (&labels_readback, u64::from(sample) * label_stride),
                stats: (&stats_readback, u64::from(sample) * stats_stride),
                points: (sample + 1 == samples).then_some(&points_readback),
                capacity: (sample + 1 == samples).then_some(&capacity_readback),
            };
            encode_observation_sample(
                &device,
                &queue,
                &storage,
                &reset,
                &graph,
                &observer,
                point_capacity_state,
                completed_steps,
                timestamps,
                Some(copies),
            )?;
            host_completion_seconds.push(started.elapsed().as_secs_f64());
        }

        let all_labels = crate::collision_candidates::map_values::<u32>(
            &device,
            &labels_readback,
            body_capacity as usize * samples as usize,
        )?;
        let raw_stats = crate::collision_candidates::map_values::<StatsRaw>(
            &device,
            &stats_readback,
            samples as usize,
        )?;
        let stats = raw_stats
            .into_iter()
            .map(GpuContactComponentStatsV1::from)
            .collect::<Vec<_>>();
        let labels = all_labels[all_labels.len() - body_capacity as usize..].to_vec();
        let replay_exact = all_labels
            .chunks_exact(body_capacity as usize)
            .all(|sample_labels| sample_labels == labels)
            && stats
                .iter()
                .all(|sample_stats| structurally_equal(sample_stats, &stats[0]));

        let capacity_words = crate::collision_candidates::map_values::<u32>(
            &device,
            &capacity_readback,
            (point_capacity_state.size() / size_of::<u32>() as u64) as usize,
        )?;
        let point_span = capacity_words
            .get(2)
            .copied()
            .ok_or_else(|| {
                "component observation point capacity omitted committed count".to_owned()
            })?
            .saturating_mul(2)
            .min(point_capacity);
        let point_values = crate::collision_candidates::map_values::<PointInputRaw>(
            &device,
            &points_readback,
            point_capacity as usize,
        )?;
        let live_dynamic = scenario
            .bodies
            .iter()
            .map(|body| body.inverse_mass.get() > 0.0)
            .chain(std::iter::repeat(false))
            .take(body_capacity as usize)
            .collect::<Vec<_>>();
        let generations = scenario
            .bodies
            .iter()
            .map(|body| body.id.generation)
            .chain(std::iter::repeat(0))
            .take(body_capacity as usize)
            .collect::<Vec<_>>();
        let mut edges = Vec::new();
        for point in &point_values[..point_span as usize] {
            let ids = point.component_ids();
            if ids[3] != 0 || ids[0] == INVALID_LABEL {
                continue;
            }
            if ids[0] >= body_capacity || ids[1] >= body_capacity || ids[0] == ids[1] {
                return Err(format!(
                    "component observation found malformed edge ids {ids:?}"
                ));
            }
            let point_generations = point.component_generations();
            if generations[ids[0] as usize] != point_generations[0]
                || generations[ids[1] as usize] != point_generations[1]
            {
                return Err(format!(
                    "component observation found stale edge ids {ids:?}"
                ));
            }
            if live_dynamic[ids[0] as usize] && live_dynamic[ids[1] as usize] {
                edges.push([ids[0], ids[1]]);
            }
        }
        edges.sort_unstable();
        if edges.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("component observation projected duplicate manifold edges".to_owned());
        }
        let scalar_edges = edges
            .iter()
            .map(|edge| (edge[0], edge[1]))
            .collect::<Vec<_>>();
        let cpu_labels = reference_component_labels(&live_dynamic, &scalar_edges)?;
        let expected_components = cpu_labels
            .iter()
            .enumerate()
            .filter(|(slot, label)| **label == *slot as u32)
            .count() as u32;
        let mut component_sizes = vec![0_u32; body_capacity as usize];
        for &label in &cpu_labels {
            if label != INVALID_LABEL {
                component_sizes[label as usize] += 1;
            }
        }
        let expected_largest = component_sizes.into_iter().max().unwrap_or(0);
        let final_stats = stats
            .last()
            .ok_or_else(|| "component observation returned no statistics".to_owned())?;
        let exact = replay_exact
            && labels == cpu_labels
            && final_stats.dynamic_body_count
                == live_dynamic.iter().filter(|&&live| live).count() as u32
            && final_stats.unique_edge_count == edges.len() as u32
            && final_stats.component_count == expected_components
            && final_stats.largest_component_size == expected_largest
            && final_stats.invalid_root_count == 0
            && final_stats.cross_label_edge_count == 0
            && final_stats.invalid_edge_count == 0
            && final_stats.phase_bound == observer.phase_bound
            && final_stats.body_capacity == body_capacity
            && final_stats.point_capacity == point_capacity
            && final_stats.last_changed_phase <= observer.phase_bound;
        if !exact {
            let first_label_difference = labels
                .iter()
                .zip(&cpu_labels)
                .position(|(gpu, cpu)| gpu != cpu);
            return Err(format!(
                "contact-component exactness gate failed; first label difference {first_label_difference:?}, stats {final_stats:?}"
            ));
        }

        let (gpu_component_seconds, gpu_physics_seconds, gpu_end_to_end_seconds, period) =
            if let Some((query_set, resolve, readback)) = &query_resources {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("physbox5_contact_component_timestamp_copy"),
                });
                encoder.resolve_query_set(query_set, 0..query_count, resolve, 0);
                encoder.copy_buffer_to_buffer(
                    resolve,
                    0,
                    readback,
                    0,
                    u64::from(query_count) * size_of::<u64>() as u64,
                );
                queue.submit([encoder.finish()]);
                let values = crate::collision_candidates::map_values::<u64>(
                    &device,
                    readback,
                    query_count as usize,
                )?;
                let period = queue.get_timestamp_period();
                let seconds = |begin: usize, end: usize| {
                    values
                        .chunks_exact(6)
                        .map(|sample| {
                            sample[end].saturating_sub(sample[begin]) as f64
                                * f64::from(period)
                                * 1.0e-9
                        })
                        .collect::<Vec<_>>()
                };
                (
                    Some(seconds(4, 5)),
                    Some(seconds(0, 3)),
                    Some(seconds(0, 5)),
                    Some(period),
                )
            } else {
                (None, None, None, None)
            };

        Ok(GpuContactComponentObservationV1 {
            version: OUTPUT_VERSION,
            phase_bound_version: CONTACT_COMPONENT_PHASE_BOUND_VERSION,
            adapter: diagnostics,
            source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
            body_count,
            body_capacity,
            completed_steps,
            samples,
            phase_bound: observer.phase_bound,
            observer_dispatches_per_sample: observer.dispatches_per_sample(),
            observer_allocated_bytes: observer.allocated_bytes(),
            product_dispatches_per_step: graph.dispatches_per_step(),
            product_allocated_bytes: graph.allocated_bytes(),
            gpu_component_seconds,
            gpu_physics_seconds,
            gpu_end_to_end_seconds,
            host_completion_seconds,
            timestamp_period_nanoseconds: period,
            labels,
            cpu_labels,
            edges,
            stats,
            replay_exact,
            exact,
        })
    }

    struct SampleCopies<'a> {
        labels: (&'a wgpu::Buffer, u64),
        stats: (&'a wgpu::Buffer, u64),
        points: Option<&'a wgpu::Buffer>,
        capacity: Option<&'a wgpu::Buffer>,
    }

    fn structurally_equal(
        left: &GpuContactComponentStatsV1,
        right: &GpuContactComponentStatsV1,
    ) -> bool {
        left.dynamic_body_count == right.dynamic_body_count
            && left.unique_edge_count == right.unique_edge_count
            && left.component_count == right.component_count
            && left.largest_component_size == right.largest_component_size
            && left.invalid_root_count == right.invalid_root_count
            && left.cross_label_edge_count == right.cross_label_edge_count
            && left.invalid_edge_count == right.invalid_edge_count
            && left.phase_bound == right.phase_bound
            && left.body_capacity == right.body_capacity
            && left.point_capacity == right.point_capacity
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_observation_sample(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        storage: &GpuBodyStorage,
        reset: &crate::body_state::GpuBodyResetData,
        graph: &GpuGeneratedContactGraph,
        observer: &GpuContactComponentObserver,
        point_capacity_state: &wgpu::Buffer,
        completed_steps: u32,
        timestamps: Option<(&wgpu::QuerySet, u32)>,
        copies: Option<SampleCopies<'_>>,
    ) -> Result<(), String> {
        storage.upload_reset_data(queue, reset);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll component observation reset: {error}"))?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_contact_component_sample"),
        });
        for step in 1..=completed_steps {
            let final_step = step == completed_steps;
            graph.encode_step_with_timestamps(
                &mut encoder,
                step == 1,
                timestamps.filter(|_| final_step),
            );
        }
        observer.encode(
            &mut encoder,
            point_capacity_state,
            timestamps.map(|(set, base)| (set, base + 4, base + 5)),
        );
        if let Some(copies) = copies {
            encoder.copy_buffer_to_buffer(
                &observer.parents,
                0,
                copies.labels.0,
                copies.labels.1,
                observer.parents.size(),
            );
            encoder.copy_buffer_to_buffer(
                &observer.stats,
                0,
                copies.stats.0,
                copies.stats.1,
                observer.stats.size(),
            );
            if let Some(points) = copies.points {
                let (source, _, _) = graph.contact_component_sources();
                encoder.copy_buffer_to_buffer(source, 0, points, 0, source.size());
            }
            if let Some(capacity) = copies.capacity {
                encoder.copy_buffer_to_buffer(
                    point_capacity_state,
                    0,
                    capacity,
                    0,
                    point_capacity_state.size(),
                );
            }
        }
        queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll component observation sample: {error}"))?;
        Ok(())
    }

    fn readback_buffer(device: &wgpu::Device, label: &'static str, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn gpu_observer_matches_cpu_on_a_dynamic_stack_and_ignores_the_floor() {
            let scenario = crate::p5_2_centered_rectangle_stack_scenario(2, 1).unwrap();
            let output =
                pollster::block_on(observe_gpu_contact_components(&scenario, 64, 1, 1, 2)).unwrap();
            assert!(output.exact);
            assert!(output.replay_exact);
            assert_eq!(output.labels[0], INVALID_LABEL);
            assert_eq!(output.labels[1], 1);
            assert_eq!(output.labels[2], 1);
            assert_eq!(output.stats[0].dynamic_body_count, 2);
            assert_eq!(output.stats[0].component_count, 1);
            assert_eq!(output.stats[0].largest_component_size, 2);
            assert_eq!(output.stats[0].unique_edge_count, 1);
        }

        #[test]
        fn gpu_observer_matches_shared_split_bridge_and_pile_fixtures() {
            let cases = [
                (
                    crate::s26_isolated_components_scenario(1).unwrap(),
                    64,
                    2,
                    1,
                ),
                (
                    crate::s26_bridge_components_scenario(false, 1).unwrap(),
                    64,
                    2,
                    2,
                ),
                (
                    crate::s26_bridge_components_scenario(true, 1).unwrap(),
                    64,
                    1,
                    5,
                ),
                (
                    crate::s26_packed_pile_scenario(100, 1).unwrap(),
                    256,
                    1,
                    100,
                ),
                (
                    crate::s26_packed_pile_scenario(1_000, 1).unwrap(),
                    1_024,
                    1,
                    1_000,
                ),
            ];
            for (scenario, capacity, component_count, largest_component) in cases {
                let output = pollster::block_on(observe_gpu_contact_components(
                    &scenario, capacity, 1, 0, 2,
                ))
                .unwrap();
                assert!(output.exact, "{}", scenario.id);
                assert!(output.replay_exact, "{}", scenario.id);
                assert_eq!(
                    output.stats[0].component_count, component_count,
                    "{}",
                    scenario.id
                );
                assert_eq!(
                    output.stats[0].largest_component_size, largest_component,
                    "{}",
                    scenario.id
                );
            }
        }
    }
}

#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use gpu::{
    GpuContactComponentObservationV1, GpuContactComponentStatsV1, observe_gpu_contact_components,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_matches_reference(live: &[bool], edges: &[(u32, u32)]) {
        let observed = observe_components_scalar(live, edges).unwrap();
        assert!(observed.converged, "did not converge for {edges:?}");
        assert_eq!(
            observed.labels,
            reference_component_labels(live, edges).unwrap()
        );
        assert_eq!(
            observed.phases_executed,
            component_phase_bound(live.len() as u32).unwrap()
        );
    }

    #[test]
    fn phase_bound_is_monotonic_and_capacity_derived() {
        assert_eq!(component_phase_bound(1), Ok(1));
        assert_eq!(component_phase_bound(2), Ok(3));
        assert_eq!(component_phase_bound(3), Ok(5));
        assert_eq!(component_phase_bound(4), Ok(5));
        assert_eq!(component_phase_bound(1_024), Ok(21));
        assert_eq!(component_phase_bound(100_000), Ok(35));
        assert!(component_phase_bound(0).is_err());
    }

    #[test]
    fn every_graph_through_six_vertices_matches_independent_traversal() {
        for vertex_count in 1..=6u32 {
            let candidates = (0..vertex_count)
                .flat_map(|a| ((a + 1)..vertex_count).map(move |b| (a, b)))
                .collect::<Vec<_>>();
            for mask in 0u64..(1u64 << candidates.len()) {
                let edges = candidates
                    .iter()
                    .enumerate()
                    .filter_map(|(index, edge)| ((mask >> index) & 1 == 1).then_some(*edge))
                    .collect::<Vec<_>>();
                assert_matches_reference(&vec![true; vertex_count as usize], &edges);
            }
        }
    }

    #[test]
    fn inactive_slots_and_disconnected_components_keep_exact_minimum_labels() {
        let live = [true, true, false, true, true, true, false, true];
        let edges = [(5, 3), (4, 5), (1, 0)];
        assert_matches_reference(&live, &edges);
        assert_eq!(
            observe_components_scalar(&live, &edges).unwrap().labels,
            vec![0, 0, INVALID_LABEL, 3, 3, 3, INVALID_LABEL, 7]
        );
    }

    #[test]
    fn adversarial_long_paths_stars_and_bridges_converge_inside_the_bound() {
        for vertex_count in [2u32, 3, 17, 64, 257, 1_024] {
            let live = vec![true; vertex_count as usize];
            let forward = (1..vertex_count)
                .map(|body| (body - 1, body))
                .collect::<Vec<_>>();
            let reverse = forward.iter().rev().copied().collect::<Vec<_>>();
            let star = (1..vertex_count)
                .rev()
                .map(|body| (body, 0))
                .collect::<Vec<_>>();
            assert_matches_reference(&live, &forward);
            assert_matches_reference(&live, &reverse);
            assert_matches_reference(&live, &star);
        }

        let live = vec![true; 256];
        let mut bridge = (1..128).map(|body| (body - 1, body)).collect::<Vec<_>>();
        bridge.extend((129..256).map(|body| (body - 1, body)));
        bridge.push((127, 255));
        assert_matches_reference(&live, &bridge);
    }

    #[test]
    fn malformed_edge_projection_fails_closed() {
        assert!(observe_components_scalar(&[true], &[(0, 0)]).is_err());
        assert!(observe_components_scalar(&[true, false], &[(0, 1)]).is_err());
        assert!(observe_components_scalar(&[true], &[(0, 1)]).is_err());
    }
}
