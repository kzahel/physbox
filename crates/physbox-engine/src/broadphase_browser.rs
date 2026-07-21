use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::{
    AdapterDiagnostics,
    broadphase_grid::{GRID_DISPATCHES, GpuGridConfigurationV1, GridCase, GridPipelines},
    broadphase_sap::{SAP_DISPATCHES, SapCase, SapPipelines},
    gpu_context::{create_instance, request_adapter, request_device},
    scenario::{P5_3BroadphasePattern, p5_3_broadphase_snapshot},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuBroadphaseBrowserProbeV1 {
    pub adapter: AdapterDiagnostics,
    pub pattern: &'static str,
    pub proxy_count: u32,
    pub seed: u64,
    pub sap_candidate_capacity: u32,
    pub grid_configuration: GpuGridConfigurationV1,
    pub sap_dispatches: u32,
    pub grid_dispatches: u32,
    pub completion_method: &'static str,
    pub qualification_scope: &'static str,
}

pub struct GpuBroadphaseBrowserSession {
    device: wgpu::Device,
    queue: wgpu::Queue,
    sap_pipelines: SapPipelines,
    sap_case: SapCase,
    grid_pipelines: GridPipelines,
    grid_case: GridCase,
    probe: GpuBroadphaseBrowserProbeV1,
}

impl GpuBroadphaseBrowserSession {
    pub async fn new(
        pattern: P5_3BroadphasePattern,
        proxy_count: u32,
        seed: u64,
    ) -> Result<Self, String> {
        let snapshot = p5_3_broadphase_snapshot(pattern, proxy_count, seed)
            .map_err(|error| error.to_string())?;
        let mut candidate_capacity = scaled_capacity(proxy_count, 16)?;
        let mut pair_emission_capacity = scaled_capacity(proxy_count, 64)?;
        if pattern == P5_3BroadphasePattern::CompleteOverlap {
            let all_pairs = u64::from(proxy_count) * u64::from(proxy_count - 1) / 2;
            candidate_capacity =
                candidate_capacity.max(u32::try_from(all_pairs).map_err(|_| {
                    "browser complete-overlap candidate count exceeds u32".to_owned()
                })?);
            pair_emission_capacity =
                pair_emission_capacity.max(u32::try_from(all_pairs * 9).map_err(|_| {
                    "browser complete-overlap emission count exceeds u32".to_owned()
                })?);
        }
        let grid_configuration = GpuGridConfigurationV1 {
            cell_size: 1.0,
            max_cells_per_fixed: 16,
            cell_record_capacity: scaled_capacity(proxy_count, 16)?,
            global_static_capacity: 2_048,
            pair_emission_capacity,
            candidate_capacity,
        };
        let instance = create_instance();
        let adapter = request_adapter(&instance, None).await?;
        let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let (device, queue) = request_device(&adapter).await?;
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let sap_pipelines = SapPipelines::new(&device);
        let sap_case = SapCase::new(&device, &sap_pipelines, &snapshot, candidate_capacity)?;
        let grid_pipelines = GridPipelines::new(&device);
        let grid_case = GridCase::new(&device, &grid_pipelines, &snapshot, grid_configuration)?;
        if let Some(error) = device.pop_error_scope().await {
            return Err(format!(
                "browser broadphase pipeline or resource validation failed: {error}"
            ));
        }
        Ok(Self {
            device,
            queue,
            sap_pipelines,
            sap_case,
            grid_pipelines,
            grid_case,
            probe: GpuBroadphaseBrowserProbeV1 {
                adapter: diagnostics,
                pattern: pattern.name(),
                proxy_count,
                seed,
                sap_candidate_capacity: candidate_capacity,
                grid_configuration,
                sap_dispatches: SAP_DISPATCHES,
                grid_dispatches: GRID_DISPATCHES,
                completion_method: "mapped-candidate-capacity-readback",
                qualification_scope: "same-wgsl-browser-portability-execution",
            },
        })
    }

    pub fn probe(&self) -> &GpuBroadphaseBrowserProbeV1 {
        &self.probe
    }

    pub async fn submit_sap(&self) -> Result<GpuBroadphaseBrowserCompletion, String> {
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let readback = completion_readback(&self.device, "sap", 16);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_browser_sap_probe"),
            });
        self.sap_case
            .encode(&mut encoder, &self.sap_pipelines, None);
        self.sap_case.copy_capacity_to(&mut encoder, &readback);
        self.queue.submit([encoder.finish()]);
        if let Some(error) = self.device.pop_error_scope().await {
            return Err(format!("browser SAP submission validation failed: {error}"));
        }
        Ok(GpuBroadphaseBrowserCompletion::new(readback, 1, "SAP"))
    }

    pub async fn submit_grid(&self) -> Result<GpuBroadphaseBrowserCompletion, String> {
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let readback = completion_readback(&self.device, "grid", 64);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_browser_grid_probe"),
            });
        self.grid_case
            .encode(&mut encoder, &self.grid_pipelines, None);
        self.grid_case.copy_capacity_to(&mut encoder, &readback);
        self.queue.submit([encoder.finish()]);
        if let Some(error) = self.device.pop_error_scope().await {
            return Err(format!(
                "browser grid submission validation failed: {error}"
            ));
        }
        Ok(GpuBroadphaseBrowserCompletion::new(readback, 4, "grid"))
    }

    pub fn poll(&self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Poll)
            .map(|_| ())
            .map_err(|error| format!("poll browser broadphase probe: {error}"))
    }
}

pub struct GpuBroadphaseBrowserCompletion {
    buffer: wgpu::Buffer,
    capacity_records: usize,
    algorithm: &'static str,
    result: Arc<Mutex<Option<Result<(), String>>>>,
}

impl GpuBroadphaseBrowserCompletion {
    fn new(buffer: wgpu::Buffer, capacity_records: usize, algorithm: &'static str) -> Self {
        let result = Arc::new(Mutex::new(None));
        let callback_result = Arc::clone(&result);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |mapped| {
                *callback_result
                    .lock()
                    .expect("browser broadphase fence callback lock") =
                    Some(mapped.map_err(|error| error.to_string()));
            });
        Self {
            buffer,
            capacity_records,
            algorithm,
            result,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.result
            .lock()
            .expect("browser broadphase fence readiness lock")
            .is_some()
    }

    pub fn finish(self) -> Result<(), String> {
        self.result
            .lock()
            .map_err(|_| "browser broadphase fence result lock poisoned".to_owned())?
            .take()
            .ok_or_else(|| "browser broadphase fence is not ready".to_owned())??;
        let view = self.buffer.slice(..).get_mapped_range();
        if view.len() != self.capacity_records * 16 {
            drop(view);
            self.buffer.unmap();
            return Err("browser broadphase fence size mismatch".to_owned());
        }
        let rejection = view
            .chunks_exact(16)
            .enumerate()
            .find_map(|(index, record)| {
                let capacity =
                    u32::from_le_bytes(record[0..4].try_into().expect("capacity word"));
                let attempted =
                    u32::from_le_bytes(record[4..8].try_into().expect("attempted word"));
                let committed =
                    u32::from_le_bytes(record[8..12].try_into().expect("committed word"));
                let overflowed =
                    u32::from_le_bytes(record[12..16].try_into().expect("overflow word"));
                (capacity == 0 || overflowed != 0 || committed != attempted).then(|| {
                    format!(
                        "browser {} broadphase capacity record {index} rejected {attempted} attempted elements at capacity {capacity}",
                        self.algorithm,
                    )
                })
            });
        drop(view);
        self.buffer.unmap();
        if let Some(rejection) = rejection {
            return Err(rejection);
        }
        Ok(())
    }
}

fn completion_readback(device: &wgpu::Device, algorithm: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(&format!(
            "physbox5_browser_{algorithm}_broadphase_capacity_readback"
        )),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

fn scaled_capacity(count: u32, multiplier: u32) -> Result<u32, String> {
    count
        .checked_mul(multiplier)
        .map(|capacity| capacity.max(64))
        .ok_or_else(|| format!("browser broadphase capacity {count} * {multiplier} overflowed"))
}
