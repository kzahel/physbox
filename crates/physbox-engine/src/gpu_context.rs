use std::fmt::{Display, Formatter};

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDiagnostics {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub max_buffer_size: u64,
    pub max_storage_buffer_binding_size: u32,
    pub max_compute_invocations_per_workgroup: u32,
}

impl AdapterDiagnostics {
    pub(crate) fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let info = adapter.get_info();
        let limits = adapter.limits();
        Self {
            name: info.name,
            backend: format!("{:?}", info.backend),
            device_type: format!("{:?}", info.device_type),
            driver: info.driver,
            driver_info: info.driver_info,
            max_buffer_size: limits.max_buffer_size,
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
            max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
        }
    }

    pub fn marker_json(&self) -> String {
        serde_json::to_string(self).expect("adapter diagnostics are serializable")
    }
}

impl Display for AdapterDiagnostics {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} ({}, {}, driver: {} {})",
            self.name, self.backend, self.device_type, self.driver, self.driver_info
        )
    }
}

pub fn create_instance() -> wgpu::Instance {
    #[cfg(target_arch = "wasm32")]
    let backends = wgpu::Backends::BROWSER_WEBGPU;
    #[cfg(not(target_arch = "wasm32"))]
    let backends = wgpu::Backends::PRIMARY;

    wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends,
        ..Default::default()
    })
}

pub(crate) async fn request_adapter(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|error| format!("failed to find a WebGPU adapter: {error}"))
}

pub(crate) async fn request_device(
    adapter: &wgpu::Adapter,
) -> Result<(wgpu::Device, wgpu::Queue), String> {
    request_device_with_features(adapter, wgpu::Features::empty()).await
}

pub(crate) async fn request_device_with_features(
    adapter: &wgpu::Adapter,
    required_features: wgpu::Features,
) -> Result<(wgpu::Device, wgpu::Queue), String> {
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("physbox5_device"),
            required_features,
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .map_err(|error| format!("failed to create the WebGPU device: {error}"))
}

pub(crate) fn surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| formats.first().copied())
}
