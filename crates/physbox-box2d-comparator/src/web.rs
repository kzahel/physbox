use physbox_contracts::{BodyId, BodyStateV1, Vec2};

use crate::{BodySnapshot, GrabTuning, WorldSettings};

pub(crate) struct World;

impl World {
    pub(crate) fn new(_settings: WorldSettings) -> Result<Self, String> {
        Err("Box2D comparator WebAssembly runtime has not been initialized".to_owned())
    }

    pub(crate) fn add_body(&mut self, _body: &BodyStateV1) -> Result<(), String> {
        Err("Box2D comparator WebAssembly runtime is unavailable".to_owned())
    }

    pub(crate) fn remove_body(&mut self, _body: BodyId) -> Result<(), String> {
        Err("Box2D comparator WebAssembly runtime is unavailable".to_owned())
    }

    pub(crate) fn apply_impulse(
        &mut self,
        _body: BodyId,
        _impulse: Vec2,
        _world_point: Vec2,
    ) -> Result<(), String> {
        Err("Box2D comparator WebAssembly runtime is unavailable".to_owned())
    }

    pub(crate) fn begin_grab(
        &mut self,
        _body: BodyId,
        _world_anchor: Vec2,
        _world_target: Vec2,
        _tuning: GrabTuning,
    ) -> Result<(), String> {
        Err("Box2D comparator WebAssembly runtime is unavailable".to_owned())
    }

    pub(crate) fn update_grab(&mut self, _target: Vec2) -> Result<(), String> {
        Err("Box2D comparator WebAssembly runtime is unavailable".to_owned())
    }

    pub(crate) fn end_grab(&mut self) {}

    pub(crate) fn set_gravity(&mut self, _gravity: Vec2) {}

    pub(crate) fn step(&mut self, _dt: f32, _substeps: u32) -> Result<(), String> {
        Err("Box2D comparator WebAssembly runtime is unavailable".to_owned())
    }

    pub(crate) fn snapshots(&self, _capacity: u32) -> Result<Vec<BodySnapshot>, String> {
        Err("Box2D comparator WebAssembly runtime is unavailable".to_owned())
    }
}
