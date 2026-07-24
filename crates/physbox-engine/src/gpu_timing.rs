use std::mem::size_of;
use std::sync::{Arc, Mutex};

use crate::FrameTimingReport;

pub(crate) const FRAME_TIMING_SAMPLE_INTERVAL_STEPS: u32 = 30;
const TIMESTAMP_COUNT: u32 = 7;
const TIMESTAMP_SLOT_COUNT: u32 = 3;
const TIMESTAMP_BYTES: u64 = TIMESTAMP_COUNT as u64 * size_of::<u64>() as u64;

#[derive(Clone, Copy)]
pub(crate) struct GpuFrameTimestampPlan<'a> {
    query_set: &'a wgpu::QuerySet,
    query_base: u32,
}

impl<'a> GpuFrameTimestampPlan<'a> {
    const fn marker(self, offset: u32) -> GpuTimestampMarker<'a> {
        GpuTimestampMarker {
            query_set: self.query_set,
            query_index: self.query_base + offset,
        }
    }

    pub(crate) const fn frame_start(self) -> GpuTimestampMarker<'a> {
        self.marker(0)
    }

    pub(crate) const fn rigid_end(self) -> GpuTimestampMarker<'a> {
        self.marker(1)
    }

    pub(crate) const fn liquid_end(self) -> GpuTimestampMarker<'a> {
        self.marker(2)
    }

    pub(crate) const fn body_render_end(self) -> GpuTimestampMarker<'a> {
        self.marker(3)
    }

    pub(crate) const fn particle_render_end(self) -> GpuTimestampMarker<'a> {
        self.marker(4)
    }

    pub(crate) const fn ui_render_end(self) -> GpuTimestampMarker<'a> {
        self.marker(6)
    }

    pub(crate) const fn composite_render_end(self) -> GpuTimestampMarker<'a> {
        self.marker(5)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GpuTimestampMarker<'a> {
    query_set: &'a wgpu::QuerySet,
    query_index: u32,
}

pub(crate) fn write_timestamp_marker(
    encoder: &mut wgpu::CommandEncoder,
    marker: Option<GpuTimestampMarker<'_>>,
    label: &'static str,
) {
    let Some(marker) = marker else {
        return;
    };
    let _pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
            query_set: marker.query_set,
            beginning_of_pass_write_index: Some(marker.query_index),
            end_of_pass_write_index: None,
        }),
    });
}

struct PendingSample {
    surface_frame_serial: u64,
    frame_index: u64,
    fixed_steps: u32,
    mapping_started: bool,
}

struct TimestampSlot {
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    mapping_result: Arc<Mutex<Option<Result<(), String>>>>,
    pending: Option<PendingSample>,
}

impl TimestampSlot {
    fn new(device: &wgpu::Device, index: u32) -> Self {
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_frame_timing_resolve"),
            size: TIMESTAMP_BYTES,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_frame_timing_readback"),
            size: TIMESTAMP_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let _ = index;
        Self {
            resolve,
            readback,
            mapping_result: Arc::new(Mutex::new(None)),
            pending: None,
        }
    }

    fn start_mapping(&mut self) {
        let pending = self
            .pending
            .as_mut()
            .expect("encoded timing slot has pending sample metadata");
        assert!(!pending.mapping_started, "timing slot maps once per sample");
        pending.mapping_started = true;
        let callback_result = Arc::clone(&self.mapping_result);
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |mapped| {
                *callback_result.lock().expect("frame timing callback lock") =
                    Some(mapped.map_err(|error| error.to_string()));
            });
    }
}

#[derive(Clone)]
struct ResolvedSample {
    surface_frame_serial: u64,
    frame_index: u64,
    fixed_steps: u32,
    rigid_compute_ms: f64,
    liquid_compute_ms: f64,
    body_render_ms: f64,
    particle_render_ms: f64,
    composite_render_ms: f64,
    ui_render_ms: f64,
    total_gpu_ms: f64,
}

pub(crate) struct SurfaceFrameTiming {
    query_set: Option<wgpu::QuerySet>,
    slots: Vec<TimestampSlot>,
    timestamp_period_nanoseconds: f64,
    surface_frame_serial: u64,
    last_cpu_encode_ms: Option<f64>,
    latest: Option<ResolvedSample>,
    error: Option<String>,
}

impl SurfaceFrameTiming {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        timestamp_queries_supported: bool,
    ) -> Self {
        let query_set = timestamp_queries_supported.then(|| {
            device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("physbox5_frame_timing_queries"),
                ty: wgpu::QueryType::Timestamp,
                count: TIMESTAMP_COUNT * TIMESTAMP_SLOT_COUNT,
            })
        });
        let slots = if timestamp_queries_supported {
            (0..TIMESTAMP_SLOT_COUNT)
                .map(|index| TimestampSlot::new(device, index))
                .collect()
        } else {
            Vec::new()
        };
        Self {
            query_set,
            slots,
            timestamp_period_nanoseconds: f64::from(queue.get_timestamp_period()),
            surface_frame_serial: 0,
            last_cpu_encode_ms: None,
            latest: None,
            error: None,
        }
    }

    pub(crate) fn candidate(&self) -> Option<(usize, GpuFrameTimestampPlan<'_>)> {
        if self.error.is_some() {
            return None;
        }
        let query_set = self.query_set.as_ref()?;
        self.slots
            .iter()
            .position(|slot| slot.pending.is_none())
            .map(|slot| {
                (
                    slot,
                    GpuFrameTimestampPlan {
                        query_set,
                        query_base: slot as u32 * TIMESTAMP_COUNT,
                    },
                )
            })
    }

    pub(crate) fn encode_resolve(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        slot_index: usize,
        frame_index: u64,
        fixed_steps: u32,
    ) {
        let query_set = self
            .query_set
            .as_ref()
            .expect("timed surface frames own a timestamp query set");
        let slot = &mut self.slots[slot_index];
        assert!(slot.pending.is_none(), "timing candidate slot remains idle");
        let query_base = slot_index as u32 * TIMESTAMP_COUNT;
        encoder.resolve_query_set(
            query_set,
            query_base..query_base + TIMESTAMP_COUNT,
            &slot.resolve,
            0,
        );
        encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.readback, 0, TIMESTAMP_BYTES);
        slot.pending = Some(PendingSample {
            surface_frame_serial: self.surface_frame_serial,
            frame_index,
            fixed_steps,
            mapping_started: false,
        });
    }

    pub(crate) fn start_mapping(&mut self, slot_index: usize) {
        self.slots[slot_index].start_mapping();
    }

    pub(crate) fn resolve_ready(&mut self) {
        for slot in &mut self.slots {
            let ready = slot
                .mapping_result
                .lock()
                .expect("frame timing readiness lock")
                .is_some();
            if !ready {
                continue;
            }
            let mapping_result = slot
                .mapping_result
                .lock()
                .expect("frame timing result lock")
                .take()
                .expect("ready frame timing result exists");
            let pending = slot
                .pending
                .take()
                .expect("mapped timing slot retains sample metadata");
            let decoded = match mapping_result {
                Ok(()) => {
                    let mapped = slot.readback.slice(..).get_mapped_range();
                    let decoded =
                        decode_timestamps(&mapped, self.timestamp_period_nanoseconds, &pending);
                    drop(mapped);
                    slot.readback.unmap();
                    decoded
                }
                Err(error) => Err(format!("map frame timing readback: {error}")),
            };
            match decoded {
                Ok(sample) => self.latest = Some(sample),
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }
    }

    pub(crate) fn record_cpu_encode(&mut self, milliseconds: f64) {
        self.last_cpu_encode_ms = milliseconds.is_finite().then_some(milliseconds);
    }

    pub(crate) fn finish_surface_frame(&mut self) {
        self.surface_frame_serial = self.surface_frame_serial.saturating_add(1);
    }

    pub(crate) fn report(&self) -> FrameTimingReport {
        let latest = self.latest.as_ref();
        FrameTimingReport {
            source: if self.error.is_some() {
                "timestamp-query-error"
            } else if self.query_set.is_some() {
                "surface-timestamp-query"
            } else {
                "adapter-timestamp-query-unavailable"
            },
            timestamp_queries_supported: self.query_set.is_some(),
            sample_interval_steps: FRAME_TIMING_SAMPLE_INTERVAL_STEPS,
            sample_submitted_this_frame: false,
            sample_frame_index: latest.map(|sample| sample.frame_index),
            sample_fixed_steps: latest.map(|sample| sample.fixed_steps),
            age_frames: latest.map(|sample| {
                self.surface_frame_serial
                    .saturating_sub(sample.surface_frame_serial)
            }),
            pending_samples: self
                .slots
                .iter()
                .filter(|slot| slot.pending.is_some())
                .count() as u32,
            cpu_encode_ms: self.last_cpu_encode_ms,
            rigid_compute_ms: latest.map(|sample| sample.rigid_compute_ms),
            liquid_compute_ms: latest.map(|sample| sample.liquid_compute_ms),
            body_render_ms: latest.map(|sample| sample.body_render_ms),
            particle_render_ms: latest.map(|sample| sample.particle_render_ms),
            composite_render_ms: latest.map(|sample| sample.composite_render_ms),
            ui_render_ms: latest.map(|sample| sample.ui_render_ms),
            total_gpu_ms: latest.map(|sample| sample.total_gpu_ms),
            blocking_readback_for_advancement: false,
            error: self.error.clone(),
        }
    }
}

fn decode_timestamps(
    bytes: &[u8],
    period_nanoseconds: f64,
    pending: &PendingSample,
) -> Result<ResolvedSample, String> {
    if bytes.len() != TIMESTAMP_BYTES as usize {
        return Err(format!(
            "frame timing readback contained {} bytes instead of {TIMESTAMP_BYTES}",
            bytes.len()
        ));
    }
    let values = bytes
        .chunks_exact(size_of::<u64>())
        .map(|chunk| u64::from_ne_bytes(chunk.try_into().expect("timestamp chunk is eight bytes")))
        .collect::<Vec<_>>();
    if let Some(index) = values.windows(2).position(|pair| pair[1] < pair[0]) {
        return Err(format!(
            "frame GPU timestamps ran backwards at boundary {index}: {} > {}",
            values[index],
            values[index + 1]
        ));
    }
    let milliseconds = |start: usize, end: usize| {
        (values[end] - values[start]) as f64 * period_nanoseconds * 1.0e-6
    };
    Ok(ResolvedSample {
        surface_frame_serial: pending.surface_frame_serial,
        frame_index: pending.frame_index,
        fixed_steps: pending.fixed_steps,
        rigid_compute_ms: milliseconds(0, 1),
        liquid_compute_ms: milliseconds(1, 2),
        body_render_ms: milliseconds(2, 3),
        particle_render_ms: milliseconds(3, 4),
        composite_render_ms: milliseconds(4, 5),
        ui_render_ms: milliseconds(5, 6),
        total_gpu_ms: milliseconds(0, 6),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_boundaries_decode_into_debug_buckets() {
        let values = [10_u64, 20, 50, 70, 90, 95, 110];
        let bytes = bytemuck::cast_slice(&values);
        let sample = decode_timestamps(
            bytes,
            100_000.0,
            &PendingSample {
                surface_frame_serial: 4,
                frame_index: 3,
                fixed_steps: 1,
                mapping_started: true,
            },
        )
        .unwrap();
        assert_eq!(sample.rigid_compute_ms, 1.0);
        assert_eq!(sample.liquid_compute_ms, 3.0);
        assert_eq!(sample.body_render_ms, 2.0);
        assert_eq!(sample.particle_render_ms, 2.0);
        assert_eq!(sample.composite_render_ms, 0.5);
        assert_eq!(sample.ui_render_ms, 1.5);
        assert_eq!(sample.total_gpu_ms, 10.0);
    }
}
