//! Portable GPU-canvas user-interface foundations.
//!
//! This crate owns neutral geometry, layout, theming, widget identity, and
//! pointer interaction. Application meaning belongs to its caller. Window
//! lifecycle, persistence, and platform adapters deliberately remain outside
//! this boundary.

mod geometry;
mod input;
mod layout;
mod scene;
mod theme;

pub use geometry::{Insets, Point, Rect, Size};
pub use input::{
    HitTarget, InteractionState, PointerButton, PointerEvent, PointerId, PointerPhase, RouteReport,
    WidgetEvent, WidgetEventKind, WidgetId, WidgetRole,
};
pub use layout::{Axis, FlowLayout, LinearLayout, ResponsiveScale};
pub use scene::{
    BatchCapacities, CanvasScene, CanvasSceneBuilder, IconId, Primitive, RenderReport,
};
pub use theme::{Color, Theme};
