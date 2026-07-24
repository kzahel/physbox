#[derive(Clone, Copy)]
pub struct GpuFrameTimestampPlan<'a> {
    pub query_set: &'a wgpu::QuerySet,
    pub query_base: u32,
}

impl<'a> GpuFrameTimestampPlan<'a> {
    const fn marker(self, offset: u32) -> GpuTimestampMarker<'a> {
        GpuTimestampMarker {
            query_set: self.query_set,
            query_index: self.query_base + offset,
        }
    }

    pub const fn frame_start(self) -> GpuTimestampMarker<'a> {
        self.marker(0)
    }

    pub const fn rigid_end(self) -> GpuTimestampMarker<'a> {
        self.marker(1)
    }

    pub const fn liquid_end(self) -> GpuTimestampMarker<'a> {
        self.marker(2)
    }

    pub const fn body_render_end(self) -> GpuTimestampMarker<'a> {
        self.marker(3)
    }

    pub const fn particle_render_end(self) -> GpuTimestampMarker<'a> {
        self.marker(4)
    }

    pub const fn composite_render_end(self) -> GpuTimestampMarker<'a> {
        self.marker(5)
    }

    pub const fn ui_render_end(self) -> GpuTimestampMarker<'a> {
        self.marker(6)
    }
}

#[derive(Clone, Copy)]
pub struct GpuTimestampMarker<'a> {
    pub query_set: &'a wgpu::QuerySet,
    pub query_index: u32,
}

pub fn write_timestamp_marker(
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
