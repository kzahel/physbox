#![forbid(unsafe_code)]

//! Optional same-device rendering for a [`physbox_sim::RenderView`].
//!
//! This crate owns render pipelines only. The caller owns the device, queue,
//! command encoder, target texture, composition, and presentation.

mod renderer;

#[doc(hidden)]
pub const BODY_SHADER_SOURCE: &str = include_str!("body_render.wgsl");
#[doc(hidden)]
pub const PARTICLE_SHADER_SOURCE: &str = include_str!("particle_render.wgsl");

pub use renderer::{
    Background, BodyRenderTimestamp, PhysicsRenderer, RenderError, RenderOptions, RenderTarget,
    SurfacePreview, ViewTransform,
};
