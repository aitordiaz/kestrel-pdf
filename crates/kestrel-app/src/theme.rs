use egui::Color32;

/// Warm Salmon & Light Ochre Professional Design System Tokens for Kestrel-PDF.
#[derive(Debug, Clone, Copy)]
pub struct Theme;

impl Theme {
    /// Primary CTA accent (Warm Salmon / Terracotta): #D97757
    pub const ACCENT_SALMON: Color32 = Color32::from_rgb(217, 119, 87);
    /// Primary CTA hover state: #E58B6D
    pub const ACCENT_SALMON_HOVER: Color32 = Color32::from_rgb(229, 139, 109);
    /// Primary CTA pressed/active state: #BF6243
    pub const ACCENT_SALMON_ACTIVE: Color32 = Color32::from_rgb(191, 98, 67);

    /// Secondary accent (Warm Ochre / Amber): #E28743
    pub const ACCENT_OCHRE: Color32 = Color32::from_rgb(226, 135, 67);
    /// Secondary accent hover: #EFA062
    pub const ACCENT_OCHRE_HOVER: Color32 = Color32::from_rgb(239, 160, 98);

    /// Base panel dark surface: Slate 900 (#0F172A)
    pub const PANEL_DARK: Color32 = Color32::from_rgb(15, 23, 42);
    /// Secondary panel container: Slate 800 (#1E293B)
    pub const PANEL_SURFACE: Color32 = Color32::from_rgb(30, 41, 59);
    /// Subtle high-contrast border: Slate 700 (#334155)
    pub const BORDER_DARK: Color32 = Color32::from_rgb(51, 65, 85);

    /// High contrast text on dark panels: Slate 50 (#F8FAFC)
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(248, 250, 252);
    /// Secondary muted text: Slate 400 (#94A3B8)
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184);

    /// Viewport canvas surround backdrop: Slate 700 (#334155)
    pub const CANVAS_BACKDROP: Color32 = Color32::from_rgb(51, 65, 85);
}
