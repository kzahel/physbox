use std::{
    ffi::{CStr, c_char, c_void},
    ptr::NonNull,
};

use physbox_contracts::{BodyId, BodyStateV1, ShapeV1, Vec2};

use crate::{
    BODY_AWAKE, BODY_FIXED, BodySnapshot, GrabTuning, SHAPE_CIRCLE, SHAPE_RECTANGLE, WorldSettings,
};

#[repr(C)]
struct RawWorldDef {
    gravity_x: f32,
    gravity_y: f32,
    restitution_threshold: f32,
    capacity: u32,
    sleeping_enabled: u32,
    sleep_speed_threshold: f32,
}

#[repr(C)]
struct RawBody {
    slot: u32,
    generation: u32,
    shape_kind: u32,
    flags: u32,
    position_x: f32,
    position_y: f32,
    angle: f32,
    angular_velocity: f32,
    velocity_x: f32,
    velocity_y: f32,
    half_extent_x: f32,
    half_extent_y: f32,
    inverse_mass: f32,
    inverse_inertia: f32,
    friction: f32,
    restitution: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawSnapshot {
    slot: u32,
    generation: u32,
    shape_kind: u32,
    flags: u32,
    position_x: f32,
    position_y: f32,
    angle: f32,
    angular_velocity: f32,
    velocity_x: f32,
    velocity_y: f32,
    half_extent_x: f32,
    half_extent_y: f32,
}

unsafe extern "C" {
    fn pb5_box2d_world_create(definition: *const RawWorldDef) -> *mut c_void;
    fn pb5_box2d_world_destroy(world: *mut c_void);
    fn pb5_box2d_world_add_body(world: *mut c_void, body: *const RawBody) -> bool;
    fn pb5_box2d_world_remove_body(world: *mut c_void, slot: u32, generation: u32) -> bool;
    fn pb5_box2d_world_apply_impulse(
        world: *mut c_void,
        slot: u32,
        generation: u32,
        impulse_x: f32,
        impulse_y: f32,
        point_x: f32,
        point_y: f32,
    ) -> bool;
    fn pb5_box2d_world_reset_body(
        world: *mut c_void,
        slot: u32,
        generation: u32,
        position_x: f32,
        position_y: f32,
        angle: f32,
        velocity_x: f32,
        velocity_y: f32,
        angular_velocity: f32,
    ) -> bool;
    fn pb5_box2d_world_begin_grab(
        world: *mut c_void,
        slot: u32,
        generation: u32,
        anchor_x: f32,
        anchor_y: f32,
        target_x: f32,
        target_y: f32,
        spring_acceleration: f32,
        damping_rate: f32,
        max_acceleration: f32,
    ) -> bool;
    fn pb5_box2d_world_update_grab(world: *mut c_void, target_x: f32, target_y: f32) -> bool;
    fn pb5_box2d_world_end_grab(world: *mut c_void);
    fn pb5_box2d_world_set_gravity(world: *mut c_void, x: f32, y: f32);
    fn pb5_box2d_world_step(world: *mut c_void, dt: f32, substeps: i32);
    fn pb5_box2d_world_snapshot(
        world: *const c_void,
        output: *mut RawSnapshot,
        capacity: u32,
    ) -> u32;
    fn pb5_box2d_world_body_sleep_threshold(
        world: *const c_void,
        slot: u32,
        generation: u32,
    ) -> f32;
    fn pb5_box2d_world_last_error(world: *const c_void) -> *const c_char;
}

pub(crate) struct World {
    raw: NonNull<c_void>,
}

impl World {
    pub(crate) fn new(settings: WorldSettings) -> Result<Self, String> {
        let definition = RawWorldDef {
            gravity_x: settings.gravity.x,
            gravity_y: settings.gravity.y,
            restitution_threshold: settings.restitution_threshold,
            capacity: settings.capacity,
            sleeping_enabled: u32::from(settings.sleeping_enabled),
            sleep_speed_threshold: settings.sleep_speed_threshold,
        };
        // SAFETY: definition is fully initialized and borrowed for the duration of the call.
        let raw = unsafe { pb5_box2d_world_create(&raw const definition) };
        let raw = NonNull::new(raw)
            .ok_or_else(|| "failed to allocate the Box2D comparator world".to_owned())?;
        Ok(Self { raw })
    }

    pub(crate) fn add_body(&mut self, body: &BodyStateV1) -> Result<(), String> {
        let (shape_kind, half_extent_x, half_extent_y) = match body.shape {
            ShapeV1::Circle { radius } => (SHAPE_CIRCLE, radius.get(), radius.get()),
            ShapeV1::Rectangle { half_extents } => {
                (SHAPE_RECTANGLE, half_extents.x, half_extents.y)
            }
        };
        let fixed = body.inverse_mass.get() == 0.0;
        let raw = RawBody {
            slot: body.id.slot,
            generation: body.id.generation,
            shape_kind,
            flags: (if fixed { BODY_FIXED } else { 0 })
                | if body.sleeping { 0 } else { BODY_AWAKE },
            position_x: body.position.x,
            position_y: body.position.y,
            angle: body.angle.get(),
            angular_velocity: body.angular_velocity.get(),
            velocity_x: body.linear_velocity.x,
            velocity_y: body.linear_velocity.y,
            half_extent_x,
            half_extent_y,
            inverse_mass: body.inverse_mass.get(),
            inverse_inertia: body.inverse_inertia.get(),
            friction: body.material.friction.get(),
            restitution: body.material.restitution.get(),
        };
        // SAFETY: the world is exclusively owned and raw is valid for the call.
        if unsafe { pb5_box2d_world_add_body(self.raw.as_ptr(), &raw const raw) } {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    pub(crate) fn remove_body(&mut self, body: BodyId) -> Result<(), String> {
        // SAFETY: the world is exclusively owned.
        if unsafe { pb5_box2d_world_remove_body(self.raw.as_ptr(), body.slot, body.generation) } {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    pub(crate) fn apply_impulse(
        &mut self,
        body: BodyId,
        impulse: Vec2,
        world_point: Vec2,
    ) -> Result<(), String> {
        // SAFETY: the world is exclusively owned and scalar arguments are finite by contract.
        if unsafe {
            pb5_box2d_world_apply_impulse(
                self.raw.as_ptr(),
                body.slot,
                body.generation,
                impulse.x,
                impulse.y,
                world_point.x,
                world_point.y,
            )
        } {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    pub(crate) fn reset_body(
        &mut self,
        body: BodyId,
        position: Vec2,
        angle: f32,
        linear_velocity: Vec2,
        angular_velocity: f32,
    ) -> Result<(), String> {
        // SAFETY: the world is exclusively owned and scalar arguments are finite by contract.
        if unsafe {
            pb5_box2d_world_reset_body(
                self.raw.as_ptr(),
                body.slot,
                body.generation,
                position.x,
                position.y,
                angle,
                linear_velocity.x,
                linear_velocity.y,
                angular_velocity,
            )
        } {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    pub(crate) fn begin_grab(
        &mut self,
        body: BodyId,
        world_anchor: Vec2,
        world_target: Vec2,
        tuning: GrabTuning,
    ) -> Result<(), String> {
        // SAFETY: the world is exclusively owned and scalar arguments are finite by contract.
        if unsafe {
            pb5_box2d_world_begin_grab(
                self.raw.as_ptr(),
                body.slot,
                body.generation,
                world_anchor.x,
                world_anchor.y,
                world_target.x,
                world_target.y,
                tuning.spring_acceleration,
                tuning.damping_rate,
                tuning.max_acceleration,
            )
        } {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    pub(crate) fn update_grab(&mut self, target: Vec2) -> Result<(), String> {
        // SAFETY: the world is exclusively owned.
        if unsafe { pb5_box2d_world_update_grab(self.raw.as_ptr(), target.x, target.y) } {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    pub(crate) fn end_grab(&mut self) {
        // SAFETY: the world is exclusively owned.
        unsafe { pb5_box2d_world_end_grab(self.raw.as_ptr()) };
    }

    pub(crate) fn set_gravity(&mut self, gravity: Vec2) {
        // SAFETY: the world is exclusively owned.
        unsafe { pb5_box2d_world_set_gravity(self.raw.as_ptr(), gravity.x, gravity.y) };
    }

    pub(crate) fn step(&mut self, dt: f32, substeps: u32) -> Result<(), String> {
        // SAFETY: the caller validates the scalar range and the world is exclusively owned.
        unsafe { pb5_box2d_world_step(self.raw.as_ptr(), dt, substeps as i32) };
        Ok(())
    }

    pub(crate) fn snapshots(&self, capacity: u32) -> Result<Vec<BodySnapshot>, String> {
        let mut raw = vec![RawSnapshot::default(); capacity as usize];
        // SAFETY: output has capacity entries and the world is alive.
        let count =
            unsafe { pb5_box2d_world_snapshot(self.raw.as_ptr(), raw.as_mut_ptr(), capacity) };
        if count > capacity {
            return Err("Box2D comparator returned an invalid snapshot count".to_owned());
        }
        raw.truncate(count as usize);
        raw.into_iter()
            .map(|body| {
                let shape = match body.shape_kind {
                    SHAPE_CIRCLE => ShapeV1::Circle {
                        radius: physbox_contracts::FiniteF32::new(body.half_extent_x)
                            .map_err(|error| error.to_string())?,
                    },
                    SHAPE_RECTANGLE => ShapeV1::Rectangle {
                        half_extents: Vec2::new(body.half_extent_x, body.half_extent_y),
                    },
                    other => return Err(format!("unknown Box2D comparator shape {other}")),
                };
                Ok(BodySnapshot {
                    id: BodyId {
                        slot: body.slot,
                        generation: body.generation,
                    },
                    shape,
                    position: Vec2::new(body.position_x, body.position_y),
                    angle: body.angle,
                    linear_velocity: Vec2::new(body.velocity_x, body.velocity_y),
                    angular_velocity: body.angular_velocity,
                    fixed: (body.flags & BODY_FIXED) != 0,
                    awake: (body.flags & BODY_AWAKE) != 0,
                })
            })
            .collect()
    }

    pub(crate) fn body_sleep_threshold(&self, body: BodyId) -> Result<f32, String> {
        // SAFETY: the world is exclusively owned and the identity is a scalar value.
        let threshold = unsafe {
            pb5_box2d_world_body_sleep_threshold(self.raw.as_ptr(), body.slot, body.generation)
        };
        if threshold.is_finite() && threshold >= 0.0 {
            Ok(threshold)
        } else {
            Err(self.last_error())
        }
    }

    fn last_error(&self) -> String {
        // SAFETY: the world owns a stable, nul-terminated error buffer.
        let raw = unsafe { pb5_box2d_world_last_error(self.raw.as_ptr()) };
        if raw.is_null() {
            return "Box2D comparator failed without an error message".to_owned();
        }
        // SAFETY: the shim returns a valid nul-terminated string for the world's lifetime.
        unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner and destruction happens exactly once.
        unsafe { pb5_box2d_world_destroy(self.raw.as_ptr()) };
    }
}
