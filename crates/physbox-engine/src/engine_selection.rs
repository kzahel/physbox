#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ComparisonEngine {
    #[default]
    PhysBox,
    #[cfg(feature = "box2d-comparator")]
    Box2d,
}

impl ComparisonEngine {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::PhysBox => "physbox",
            #[cfg(feature = "box2d-comparator")]
            Self::Box2d => "box2d",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::PhysBox => "PHYSBOX",
            #[cfg(feature = "box2d-comparator")]
            Self::Box2d => "BOX2D",
        }
    }
}
