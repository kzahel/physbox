//! Unstable first-party bridge to private mechanical GPU resources.
//!
//! This module exists only for the PhysBox 5 engine during the library
//! extraction. It is not part of the consumer contract. Resources borrow the
//! simulation world and are invalidated by mutable world advancement.

pub const ENGINE_RESOURCE_BRIDGE_VERSION: u32 = 1;

#[derive(Clone, Copy)]
pub struct EngineParticleResources<'a> {
    positions: &'a wgpu::Buffer,
    velocities: &'a wgpu::Buffer,
    density_states: &'a wgpu::Buffer,
    metadata: &'a wgpu::Buffer,
    capacity: u32,
    radius: f32,
    support_radius: f32,
    rest_density: f32,
}

impl<'a> EngineParticleResources<'a> {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub const fn from_raw_parts(
        positions: &'a wgpu::Buffer,
        velocities: &'a wgpu::Buffer,
        density_states: &'a wgpu::Buffer,
        metadata: &'a wgpu::Buffer,
        capacity: u32,
        radius: f32,
        support_radius: f32,
        rest_density: f32,
    ) -> Self {
        Self {
            positions,
            velocities,
            density_states,
            metadata,
            capacity,
            radius,
            support_radius,
            rest_density,
        }
    }

    pub const fn positions(&self) -> &'a wgpu::Buffer {
        self.positions
    }

    pub const fn velocities(&self) -> &'a wgpu::Buffer {
        self.velocities
    }

    pub const fn density_states(&self) -> &'a wgpu::Buffer {
        self.density_states
    }

    pub const fn metadata(&self) -> &'a wgpu::Buffer {
        self.metadata
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub const fn radius(&self) -> f32 {
        self.radius
    }

    pub const fn support_radius(&self) -> f32 {
        self.support_radius
    }

    pub const fn rest_density(&self) -> f32 {
        self.rest_density
    }
}

#[derive(Clone, Copy)]
pub struct EngineBodyResources<'a> {
    poses: &'a wgpu::Buffer,
    properties: &'a wgpu::Buffer,
    metadata: &'a wgpu::Buffer,
    active_bodies: &'a wgpu::Buffer,
    sleep_states: &'a wgpu::Buffer,
    grab_state: &'a wgpu::Buffer,
    active_draw_offset: u64,
    kill_zone_y: f32,
    particles: Option<EngineParticleResources<'a>>,
}

impl<'a> EngineBodyResources<'a> {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub const fn from_raw_parts(
        poses: &'a wgpu::Buffer,
        properties: &'a wgpu::Buffer,
        metadata: &'a wgpu::Buffer,
        active_bodies: &'a wgpu::Buffer,
        sleep_states: &'a wgpu::Buffer,
        grab_state: &'a wgpu::Buffer,
        active_draw_offset: u64,
        kill_zone_y: f32,
        particles: Option<EngineParticleResources<'a>>,
    ) -> Self {
        Self {
            poses,
            properties,
            metadata,
            active_bodies,
            sleep_states,
            grab_state,
            active_draw_offset,
            kill_zone_y,
            particles,
        }
    }

    pub const fn version(&self) -> u32 {
        ENGINE_RESOURCE_BRIDGE_VERSION
    }

    pub const fn poses(&self) -> &'a wgpu::Buffer {
        self.poses
    }

    pub const fn properties(&self) -> &'a wgpu::Buffer {
        self.properties
    }

    pub const fn metadata(&self) -> &'a wgpu::Buffer {
        self.metadata
    }

    pub const fn active_bodies(&self) -> &'a wgpu::Buffer {
        self.active_bodies
    }

    pub const fn sleep_states(&self) -> &'a wgpu::Buffer {
        self.sleep_states
    }

    pub const fn grab_state(&self) -> &'a wgpu::Buffer {
        self.grab_state
    }

    pub const fn active_draw_offset(&self) -> u64 {
        self.active_draw_offset
    }

    pub const fn kill_zone_y(&self) -> f32 {
        self.kill_zone_y
    }

    pub const fn particles(&self) -> Option<EngineParticleResources<'a>> {
        self.particles
    }
}
