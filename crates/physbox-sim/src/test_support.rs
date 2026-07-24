use physbox_contracts::{
    AuthoredContactPointV1, AuthoredManifoldV1, BodyId, BodyStateV1, FeatureIdV1, FiniteF32,
    MaterialV1, SCHEMA_VERSION, ScenarioV1, ShapeV1, SolverSettingsV1, Vec2,
};

pub(crate) fn p5_1_rectangle_floor_scenario(requested_steps: u32) -> Result<ScenarioV1, String> {
    if requested_steps == 0 {
        return Err("rectangle-floor test fixture requires at least one step".to_owned());
    }
    let dynamic_id = BodyId {
        slot: 0,
        generation: 1,
    };
    let floor_id = BodyId {
        slot: 1,
        generation: 1,
    };
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.0),
    };
    let dynamic_position = Vec2::new(0.0, 0.65);
    let floor_position = Vec2::new(0.0, -0.1);
    let dynamic_half_extents = Vec2::new(0.35, 0.2);
    let floor_half_extents = Vec2::new(8.0, 0.1);
    let contact_y = 0.0;
    let separation = dynamic_position.y - dynamic_half_extents.y - contact_y;
    Ok(ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: "physbox-sim-rectangle-floor-test".to_owned(),
        seed: 0,
        requested_steps,
        settings: SolverSettingsV1 {
            fixed_dt: finite(1.0 / 60.0),
            gravity: Vec2::new(0.0, -10.0),
            substeps: 4,
            iterations: 1,
            bias_rate: finite(0.2),
            mass_coefficient: finite(1.0),
            impulse_coefficient: finite(0.0),
            contact_softness: None,
            restitution_threshold: finite(1.0),
            contact_policy: None,
        },
        bodies: vec![
            BodyStateV1 {
                id: dynamic_id,
                shape: ShapeV1::Rectangle {
                    half_extents: dynamic_half_extents,
                },
                position: dynamic_position,
                angle: finite(0.0),
                linear_velocity: Vec2::ZERO,
                angular_velocity: finite(0.0),
                inverse_mass: finite(1.0),
                inverse_inertia: finite(18.461_538),
                material,
                sleeping: false,
            },
            BodyStateV1 {
                id: floor_id,
                shape: ShapeV1::Rectangle {
                    half_extents: floor_half_extents,
                },
                position: floor_position,
                angle: finite(0.0),
                linear_velocity: Vec2::ZERO,
                angular_velocity: finite(0.0),
                inverse_mass: finite(0.0),
                inverse_inertia: finite(0.0),
                material,
                sleeping: false,
            },
        ],
        manifolds: vec![AuthoredManifoldV1 {
            id: 0,
            body_a: dynamic_id,
            body_b: floor_id,
            normal: Vec2::new(0.0, -1.0),
            material,
            points: [-dynamic_half_extents.x, dynamic_half_extents.x]
                .into_iter()
                .enumerate()
                .map(|(index, x)| AuthoredContactPointV1 {
                    local_anchor_a: Vec2::new(x, -dynamic_half_extents.y),
                    local_anchor_b: Vec2::new(x, floor_half_extents.y),
                    world_point: Vec2::new(x, contact_y),
                    separation: finite(separation),
                    feature_a: FeatureIdV1 {
                        feature_type: 1,
                        feature_index: 2,
                    },
                    feature_b: FeatureIdV1 {
                        feature_type: 1,
                        feature_index: index as u32,
                    },
                })
                .collect(),
        }],
        contact_cache: Vec::new(),
        actions: Vec::new(),
    })
}

pub(crate) fn create_instance() -> wgpu::Instance {
    wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    })
}

pub(crate) mod gpu_context {
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
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("physbox_sim_test_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|error| format!("failed to create a WebGPU device: {error}"))
    }
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("test fixture constants are finite")
}
