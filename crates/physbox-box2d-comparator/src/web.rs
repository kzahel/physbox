use std::cell::RefCell;

use js_sys::{Array, Function, Reflect, Uint8Array};
use physbox_contracts::{BodyId, BodyStateV1, ShapeV1, Vec2};
use wasm_bindgen::{JsCast, JsValue};

use crate::{
    BODY_AWAKE, BODY_FIXED, BodySnapshot, GrabTuning, SHAPE_CIRCLE, SHAPE_RECTANGLE, WorldSettings,
};

const WORLD_DEFINITION_BYTES: usize = 20;
const BODY_BYTES: usize = 64;
const SNAPSHOT_BYTES: usize = 48;

thread_local! {
    static WEB_RUNTIME: RefCell<Option<JsValue>> = const { RefCell::new(None) };
}

pub(crate) fn install(runtime: JsValue) -> Result<(), String> {
    if runtime.is_null() || runtime.is_undefined() {
        return Err("Box2D comparator WebAssembly runtime is unavailable".to_owned());
    }
    for name in [
        "_malloc",
        "_free",
        "_pb5_box2d_world_create",
        "_pb5_box2d_world_destroy",
        "_pb5_box2d_world_add_body",
        "_pb5_box2d_world_reset_body",
        "_pb5_box2d_world_step",
        "_pb5_box2d_world_snapshot",
    ] {
        function(&runtime, name)?;
    }
    heap(&runtime)?;
    WEB_RUNTIME.with(|slot| *slot.borrow_mut() = Some(runtime));
    Ok(())
}

pub(crate) struct World {
    runtime: JsValue,
    raw: f64,
}

impl World {
    pub(crate) fn new(settings: WorldSettings) -> Result<Self, String> {
        let runtime = WEB_RUNTIME
            .with(|slot| slot.borrow().clone())
            .ok_or_else(|| {
                "Box2D comparator WebAssembly runtime has not been installed".to_owned()
            })?;
        let mut definition = Vec::with_capacity(WORLD_DEFINITION_BYTES);
        push_f32(&mut definition, settings.gravity.x);
        push_f32(&mut definition, settings.gravity.y);
        push_f32(&mut definition, settings.restitution_threshold);
        push_u32(&mut definition, settings.capacity);
        push_u32(&mut definition, u32::from(settings.sleeping_enabled));
        let pointer = allocate_and_write(&runtime, &definition)?;
        let raw = call(
            &runtime,
            "_pb5_box2d_world_create",
            &[JsValue::from(pointer)],
        )?;
        call(&runtime, "_free", &[JsValue::from(pointer)])?;
        if raw == 0.0 {
            return Err("failed to allocate the Box2D comparator WebAssembly world".to_owned());
        }
        Ok(Self { runtime, raw })
    }

    pub(crate) fn add_body(&mut self, body: &BodyStateV1) -> Result<(), String> {
        let (shape_kind, half_extent_x, half_extent_y) = match body.shape {
            ShapeV1::Circle { radius } => (SHAPE_CIRCLE, radius.get(), radius.get()),
            ShapeV1::Rectangle { half_extents } => {
                (SHAPE_RECTANGLE, half_extents.x, half_extents.y)
            }
        };
        let fixed = body.inverse_mass.get() == 0.0;
        let mut raw = Vec::with_capacity(BODY_BYTES);
        for value in [
            body.id.slot,
            body.id.generation,
            shape_kind,
            (if fixed { BODY_FIXED } else { 0 }) | if body.sleeping { 0 } else { BODY_AWAKE },
        ] {
            push_u32(&mut raw, value);
        }
        for value in [
            body.position.x,
            body.position.y,
            body.angle.get(),
            body.angular_velocity.get(),
            body.linear_velocity.x,
            body.linear_velocity.y,
            half_extent_x,
            half_extent_y,
            body.inverse_mass.get(),
            body.inverse_inertia.get(),
            body.material.friction.get(),
            body.material.restitution.get(),
        ] {
            push_f32(&mut raw, value);
        }
        let pointer = allocate_and_write(&self.runtime, &raw)?;
        let result = call(
            &self.runtime,
            "_pb5_box2d_world_add_body",
            &[JsValue::from(self.raw), JsValue::from(pointer)],
        );
        call(&self.runtime, "_free", &[JsValue::from(pointer)])?;
        if result? != 0.0 {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    pub(crate) fn remove_body(&mut self, body: BodyId) -> Result<(), String> {
        self.truthy(
            "_pb5_box2d_world_remove_body",
            &[
                JsValue::from(self.raw),
                JsValue::from(body.slot),
                JsValue::from(body.generation),
            ],
        )
    }

    pub(crate) fn apply_impulse(
        &mut self,
        body: BodyId,
        impulse: Vec2,
        world_point: Vec2,
    ) -> Result<(), String> {
        self.truthy(
            "_pb5_box2d_world_apply_impulse",
            &[
                JsValue::from(self.raw),
                JsValue::from(body.slot),
                JsValue::from(body.generation),
                JsValue::from(impulse.x),
                JsValue::from(impulse.y),
                JsValue::from(world_point.x),
                JsValue::from(world_point.y),
            ],
        )
    }

    pub(crate) fn reset_body(
        &mut self,
        body: BodyId,
        position: Vec2,
        angle: f32,
        linear_velocity: Vec2,
        angular_velocity: f32,
    ) -> Result<(), String> {
        self.truthy(
            "_pb5_box2d_world_reset_body",
            &[
                JsValue::from(self.raw),
                JsValue::from(body.slot),
                JsValue::from(body.generation),
                JsValue::from(position.x),
                JsValue::from(position.y),
                JsValue::from(angle),
                JsValue::from(linear_velocity.x),
                JsValue::from(linear_velocity.y),
                JsValue::from(angular_velocity),
            ],
        )
    }

    pub(crate) fn begin_grab(
        &mut self,
        body: BodyId,
        world_anchor: Vec2,
        world_target: Vec2,
        tuning: GrabTuning,
    ) -> Result<(), String> {
        self.truthy(
            "_pb5_box2d_world_begin_grab",
            &[
                JsValue::from(self.raw),
                JsValue::from(body.slot),
                JsValue::from(body.generation),
                JsValue::from(world_anchor.x),
                JsValue::from(world_anchor.y),
                JsValue::from(world_target.x),
                JsValue::from(world_target.y),
                JsValue::from(tuning.spring_acceleration),
                JsValue::from(tuning.damping_rate),
                JsValue::from(tuning.max_acceleration),
            ],
        )
    }

    pub(crate) fn update_grab(&mut self, target: Vec2) -> Result<(), String> {
        self.truthy(
            "_pb5_box2d_world_update_grab",
            &[
                JsValue::from(self.raw),
                JsValue::from(target.x),
                JsValue::from(target.y),
            ],
        )
    }

    pub(crate) fn end_grab(&mut self) {
        let _ = call(
            &self.runtime,
            "_pb5_box2d_world_end_grab",
            &[JsValue::from(self.raw)],
        );
    }

    pub(crate) fn set_gravity(&mut self, gravity: Vec2) {
        let _ = call(
            &self.runtime,
            "_pb5_box2d_world_set_gravity",
            &[
                JsValue::from(self.raw),
                JsValue::from(gravity.x),
                JsValue::from(gravity.y),
            ],
        );
    }

    pub(crate) fn step(&mut self, dt: f32, substeps: u32) -> Result<(), String> {
        call(
            &self.runtime,
            "_pb5_box2d_world_step",
            &[
                JsValue::from(self.raw),
                JsValue::from(dt),
                JsValue::from(substeps),
            ],
        )?;
        Ok(())
    }

    pub(crate) fn snapshots(&self, capacity: u32) -> Result<Vec<BodySnapshot>, String> {
        let byte_capacity = capacity as usize * SNAPSHOT_BYTES;
        let pointer = allocate(&self.runtime, byte_capacity)?;
        let result = call(
            &self.runtime,
            "_pb5_box2d_world_snapshot",
            &[
                JsValue::from(self.raw),
                JsValue::from(pointer),
                JsValue::from(capacity),
            ],
        );
        let count = result? as u32;
        if count > capacity {
            call(&self.runtime, "_free", &[JsValue::from(pointer)])?;
            return Err("Box2D comparator returned an invalid snapshot count".to_owned());
        }
        let bytes = read(&self.runtime, pointer, count as usize * SNAPSHOT_BYTES)?;
        call(&self.runtime, "_free", &[JsValue::from(pointer)])?;
        (0..count as usize)
            .map(|index| snapshot_from_bytes(&bytes[index * SNAPSHOT_BYTES..][..SNAPSHOT_BYTES]))
            .collect()
    }

    fn truthy(&self, name: &str, arguments: &[JsValue]) -> Result<(), String> {
        if call(&self.runtime, name, arguments)? != 0.0 {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    fn last_error(&self) -> String {
        let Ok(pointer) = call(
            &self.runtime,
            "_pb5_box2d_world_last_error",
            &[JsValue::from(self.raw)],
        ) else {
            return "Box2D comparator WebAssembly call failed".to_owned();
        };
        let Ok(bytes) = read(&self.runtime, pointer as u32, 160) else {
            return "Box2D comparator WebAssembly error was unreadable".to_owned();
        };
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = call(
            &self.runtime,
            "_pb5_box2d_world_destroy",
            &[JsValue::from(self.raw)],
        );
    }
}

fn snapshot_from_bytes(bytes: &[u8]) -> Result<BodySnapshot, String> {
    let slot = read_u32(bytes, 0)?;
    let generation = read_u32(bytes, 4)?;
    let shape_kind = read_u32(bytes, 8)?;
    let flags = read_u32(bytes, 12)?;
    let half_extent_x = read_f32(bytes, 40)?;
    let half_extent_y = read_f32(bytes, 44)?;
    let shape = match shape_kind {
        SHAPE_CIRCLE => ShapeV1::Circle {
            radius: physbox_contracts::FiniteF32::new(half_extent_x)
                .map_err(|error| error.to_string())?,
        },
        SHAPE_RECTANGLE => ShapeV1::Rectangle {
            half_extents: Vec2::new(half_extent_x, half_extent_y),
        },
        other => return Err(format!("unknown Box2D comparator shape {other}")),
    };
    Ok(BodySnapshot {
        id: BodyId { slot, generation },
        shape,
        position: Vec2::new(read_f32(bytes, 16)?, read_f32(bytes, 20)?),
        angle: read_f32(bytes, 24)?,
        angular_velocity: read_f32(bytes, 28)?,
        linear_velocity: Vec2::new(read_f32(bytes, 32)?, read_f32(bytes, 36)?),
        fixed: (flags & BODY_FIXED) != 0,
        awake: (flags & BODY_AWAKE) != 0,
    })
}

fn function(runtime: &JsValue, name: &str) -> Result<Function, String> {
    Reflect::get(runtime, &JsValue::from_str(name))
        .map_err(js_error)?
        .dyn_into::<Function>()
        .map_err(|_| format!("Box2D comparator runtime is missing {name}"))
}

fn call(runtime: &JsValue, name: &str, arguments: &[JsValue]) -> Result<f64, String> {
    let values = Array::new();
    for argument in arguments {
        values.push(argument);
    }
    Reflect::apply(&function(runtime, name)?, runtime, &values)
        .map_err(js_error)?
        .as_f64()
        .ok_or_else(|| format!("Box2D comparator function {name} returned a non-number"))
}

fn heap(runtime: &JsValue) -> Result<Uint8Array, String> {
    Reflect::get(runtime, &JsValue::from_str("HEAPU8"))
        .map_err(js_error)?
        .dyn_into::<Uint8Array>()
        .map_err(|_| "Box2D comparator runtime is missing HEAPU8".to_owned())
}

fn allocate(runtime: &JsValue, length: usize) -> Result<u32, String> {
    let pointer = call(runtime, "_malloc", &[JsValue::from(length as u32)])? as u32;
    if pointer == 0 {
        Err(format!(
            "Box2D comparator WebAssembly allocation of {length} bytes failed"
        ))
    } else {
        Ok(pointer)
    }
}

fn allocate_and_write(runtime: &JsValue, bytes: &[u8]) -> Result<u32, String> {
    let pointer = allocate(runtime, bytes.len())?;
    heap(runtime)?
        .subarray(pointer, pointer + bytes.len() as u32)
        .copy_from(bytes);
    Ok(pointer)
}

fn read(runtime: &JsValue, pointer: u32, length: usize) -> Result<Vec<u8>, String> {
    let mut bytes = vec![0; length];
    heap(runtime)?
        .subarray(pointer, pointer + length as u32)
        .copy_to(&mut bytes);
    Ok(bytes)
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "Box2D comparator snapshot u32 is truncated".to_owned())?;
    Ok(u32::from_le_bytes(raw.try_into().expect("four bytes")))
}

fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "Box2D comparator snapshot f32 is truncated".to_owned())?;
    let value = f32::from_le_bytes(raw.try_into().expect("four bytes"));
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| "Box2D comparator snapshot contains a non-finite value".to_owned())
}

fn js_error(value: JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}
