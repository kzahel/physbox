#![cfg_attr(not(feature = "solver-lab"), allow(dead_code, unused_imports))]

#[cfg(not(target_arch = "wasm32"))]
use std::{mem::size_of, time::Instant};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuBroadphaseTimingV1 {
    pub warmup_steps: u32,
    pub sample_steps: u32,
    pub cpu_encode_submit_seconds: Vec<f64>,
    pub host_completion_seconds: Vec<f64>,
    pub gpu_step_seconds: Option<Vec<f64>>,
    pub timestamp_period_nanoseconds: Option<f32>,
}

impl GpuBroadphaseTimingV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.warmup_steps == 0
            || self.sample_steps == 0
            || self.cpu_encode_submit_seconds.len() != self.sample_steps as usize
            || self.host_completion_seconds.len() != self.sample_steps as usize
            || self
                .gpu_step_seconds
                .as_ref()
                .is_some_and(|samples| samples.len() != self.sample_steps as usize)
            || self.gpu_step_seconds.is_some() != self.timestamp_period_nanoseconds.is_some()
            || self
                .cpu_encode_submit_seconds
                .iter()
                .chain(&self.host_completion_seconds)
                .chain(self.gpu_step_seconds.iter().flatten())
                .any(|sample| !sample.is_finite() || *sample < 0.0)
        {
            return Err("GPU broadphase timing record is inconsistent".to_owned());
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn benchmark_gpu_case<F>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    timestamps_available: bool,
    warmup_steps: u32,
    sample_steps: u32,
    mut encode: F,
) -> Result<GpuBroadphaseTimingV1, String>
where
    F: FnMut(&mut wgpu::CommandEncoder, Option<(&wgpu::QuerySet, u32, u32)>),
{
    if warmup_steps == 0 || sample_steps == 0 {
        return Err(
            "GPU broadphase benchmark requires positive warmup and sample counts".to_owned(),
        );
    }
    for _ in 0..warmup_steps {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_broadphase_warmup_encoder"),
        });
        encode(&mut encoder, None);
        queue.submit([encoder.finish()]);
    }
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("GPU broadphase warmup poll failed: {error}"))?;

    let query_count = sample_steps
        .checked_mul(2)
        .ok_or_else(|| "GPU broadphase query count overflowed".to_owned())?;
    let queries = timestamps_available.then(|| {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("physbox5_broadphase_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: query_count,
        });
        let bytes = u64::from(query_count) * size_of::<u64>() as u64;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_broadphase_timestamp_resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_broadphase_timestamp_readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        (query_set, resolve, readback)
    });
    let mut cpu_encode_submit_seconds = Vec::with_capacity(sample_steps as usize);
    let mut host_completion_seconds = Vec::with_capacity(sample_steps as usize);
    for sample in 0..sample_steps {
        let host_start = Instant::now();
        let encode_start = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_broadphase_sample_encoder"),
        });
        let timestamp = queries
            .as_ref()
            .map(|(query_set, _, _)| (query_set, sample * 2, sample * 2 + 1));
        encode(&mut encoder, timestamp);
        queue.submit([encoder.finish()]);
        cpu_encode_submit_seconds.push(encode_start.elapsed().as_secs_f64());
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("GPU broadphase sample poll failed: {error}"))?;
        host_completion_seconds.push(host_start.elapsed().as_secs_f64());
    }

    let (gpu_step_seconds, timestamp_period_nanoseconds) =
        if let Some((query_set, resolve, readback)) = queries {
            let bytes = u64::from(query_count) * size_of::<u64>() as u64;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_broadphase_timestamp_encoder"),
            });
            encoder.resolve_query_set(&query_set, 0..query_count, &resolve, 0);
            encoder.copy_buffer_to_buffer(&resolve, 0, &readback, 0, bytes);
            queue.submit([encoder.finish()]);
            device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("GPU broadphase timestamp poll failed: {error}"))?;
            let ticks = crate::collision_candidates::map_values::<u64>(
                device,
                &readback,
                query_count as usize,
            )?;
            let period = queue.get_timestamp_period();
            let seconds = ticks
                .chunks_exact(2)
                .map(|pair| pair[1].wrapping_sub(pair[0]) as f64 * f64::from(period) * 1.0e-9)
                .collect::<Vec<_>>();
            (Some(seconds), Some(period))
        } else {
            (None, None)
        };
    let timing = GpuBroadphaseTimingV1 {
        warmup_steps,
        sample_steps,
        cpu_encode_submit_seconds,
        host_completion_seconds,
        gpu_step_seconds,
        timestamp_period_nanoseconds,
    };
    timing.validate()?;
    Ok(timing)
}
