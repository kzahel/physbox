pub type Color = [f32; 4];

/// Neutral visual tokens consumed by canvas widgets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub scrim: Color,
    pub panel: Color,
    pub card: Color,
    pub control: Color,
    pub control_selected: Color,
    pub border: Color,
    pub border_selected: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub corner_radius: f32,
    pub control_radius: f32,
    pub border_width: f32,
}

impl Theme {
    /// A low-saturation dark theme suitable for overlaying colorful content.
    pub const DARK: Self = Self {
        scrim: [0.012, 0.017, 0.032, 0.80],
        panel: [0.040, 0.052, 0.091, 0.98],
        card: [0.055, 0.071, 0.118, 0.98],
        control: [0.055, 0.071, 0.130, 0.96],
        control_selected: [0.120, 0.200, 0.380, 0.98],
        border: [0.180, 0.240, 0.380, 0.85],
        border_selected: [0.320, 0.780, 1.000, 1.00],
        text: [0.820, 0.860, 0.940, 1.00],
        text_muted: [0.560, 0.620, 0.740, 1.00],
        accent: [0.340, 0.760, 1.000, 1.00],
        corner_radius: 12.0,
        control_radius: 8.0,
        border_width: 1.0,
    };
}

impl Default for Theme {
    fn default() -> Self {
        Self::DARK
    }
}
