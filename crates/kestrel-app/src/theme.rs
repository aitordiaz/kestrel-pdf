use egui::{epaint::Shadow, Color32, RichText, Rounding, Stroke, Vec2};

/// Warm Salmon & Light Ochre Professional Design System Tokens and Component Helpers for Kestrel-PDF.
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
    /// Hover surface for panels, cards, and buttons: (#2D3D54)
    pub const PANEL_SURFACE_HOVER: Color32 = Color32::from_rgb(45, 61, 84);
    /// Subtle high-contrast border: Slate 700 (#334155)
    pub const BORDER_DARK: Color32 = Color32::from_rgb(51, 65, 85);
    /// Border subtle highlight: Slate 600 (#475569)
    pub const BORDER_LIGHT: Color32 = Color32::from_rgb(71, 85, 105);

    /// High contrast text on dark panels: Slate 50 (#F8FAFC)
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(248, 250, 252);
    /// Secondary high-readability text: Slate 300 (#CBD5E1)
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(203, 213, 225);
    /// Secondary muted text: Slate 400 (#94A3B8)
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184);

    /// Viewport canvas surround backdrop: Slate 700 (#334155)
    pub const CANVAS_BACKDROP: Color32 = Color32::from_rgb(51, 65, 85);

    /// Apply the unified Kestrel-PDF visuals, color palette, spacing, and icon fonts to an `egui::Context`.
    pub fn apply(ctx: &egui::Context) {
        // Register Phosphor vector icon font once per Context to avoid font atlas churn
        let already_loaded = ctx.data(|d| {
            d.get_temp::<bool>(egui::Id::new("kestrel_theme_fonts_loaded"))
                .unwrap_or(false)
        });
        if !already_loaded {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            if let Some(mono_keys) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                mono_keys.push("phosphor".into());
            }
            ctx.set_fonts(fonts);
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("kestrel_theme_fonts_loaded"), true);
            });
        }

        let mut visuals = egui::Visuals::dark();

        visuals.panel_fill = Self::PANEL_DARK;
        visuals.window_fill = Self::PANEL_SURFACE;
        visuals.window_stroke = Stroke::new(1.0_f32, Self::BORDER_DARK);
        visuals.window_rounding = Rounding::same(10.0);
        visuals.window_shadow = Shadow {
            offset: Vec2::new(0.0, 4.0),
            blur: 16.0,
            spread: 0.0,
            color: Color32::from_black_alpha(120),
        };

        visuals.extreme_bg_color = Self::PANEL_DARK;
        visuals.faint_bg_color = Self::PANEL_SURFACE;
        visuals.code_bg_color = Self::PANEL_DARK;

        visuals.selection.bg_fill = Self::ACCENT_SALMON;
        visuals.selection.stroke = Stroke::new(1.0_f32, Self::ACCENT_SALMON_HOVER);

        visuals.hyperlink_color = Self::ACCENT_OCHRE;
        visuals.warn_fg_color = Self::ACCENT_OCHRE;

        // Non-interactive widgets (labels, disabled elements, static frames)
        visuals.widgets.noninteractive.bg_fill = Self::PANEL_DARK;
        visuals.widgets.noninteractive.weak_bg_fill = Self::PANEL_DARK;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
        visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

        // Inactive widgets (buttons, checkboxes, unselected tabs in idle state) - borderless & airy
        visuals.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(30, 41, 59, 140);
        visuals.widgets.inactive.weak_bg_fill = Color32::from_rgba_unmultiplied(30, 41, 59, 140);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        visuals.widgets.inactive.rounding = Rounding::same(6.0);

        // Hovered widgets (interactive hover feedback with soft ochre border)
        visuals.widgets.hovered.bg_fill = Self::PANEL_SURFACE_HOVER;
        visuals.widgets.hovered.weak_bg_fill = Self::PANEL_SURFACE_HOVER;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.hovered.bg_stroke =
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(226, 135, 67, 180));
        visuals.widgets.hovered.rounding = Rounding::same(6.0);

        // Active widgets (pressed state, checked checkboxes, active selection with salmon fill)
        visuals.widgets.active.bg_fill = Self::ACCENT_SALMON;
        visuals.widgets.active.weak_bg_fill = Self::ACCENT_SALMON;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.active.bg_stroke = Stroke::NONE;
        visuals.widgets.active.rounding = Rounding::same(6.0);

        // Open widgets (open dropdown menus, active combo popups)
        visuals.widgets.open.bg_fill = Self::PANEL_SURFACE;
        visuals.widgets.open.weak_bg_fill = Self::PANEL_SURFACE;
        visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, Self::ACCENT_OCHRE);
        visuals.widgets.open.rounding = Rounding::same(6.0);

        ctx.set_visuals(visuals);

        // Global spacing and margins - comfortable breathing room
        ctx.style_mut(|style| {
            style.spacing.button_padding = Vec2::new(8.0, 5.0);
            style.spacing.item_spacing = Vec2::new(4.0, 4.0);
            style.spacing.window_margin = egui::Margin::same(12.0);
        });
    }

    /// Primary CTA button constructor (Salmon background, bold white text, comfortable hit area).
    pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
        egui::Button::new(
            RichText::new(text.into())
                .color(Color32::WHITE)
                .strong()
                .size(13.0),
        )
        .fill(Self::ACCENT_SALMON)
        .rounding(Rounding::same(6.0))
        .min_size(Vec2::new(0.0, 28.0))
    }

    /// Secondary button constructor (Borderless slate surface, soft hover feedback, crisp text).
    pub fn secondary_button(text: impl Into<String>) -> egui::Button<'static> {
        egui::Button::new(
            RichText::new(text.into())
                .color(Self::TEXT_PRIMARY)
                .size(12.0),
        )
        .fill(Color32::from_rgba_unmultiplied(30, 41, 59, 140))
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(6.0))
        .min_size(Vec2::new(0.0, 26.0))
    }

    /// Borderless ghost button constructor (Transparent background, soft hover).
    pub fn ghost_button(text: impl Into<String>) -> egui::Button<'static> {
        egui::Button::new(
            RichText::new(text.into())
                .color(Self::TEXT_PRIMARY)
                .size(12.0),
        )
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(6.0))
        .min_size(Vec2::new(0.0, 24.0))
    }

    /// Accent button constructor (Warm Ochre background, bold white text).
    pub fn accent_button(text: impl Into<String>) -> egui::Button<'static> {
        egui::Button::new(
            RichText::new(text.into())
                .color(Color32::WHITE)
                .strong()
                .size(12.0),
        )
        .fill(Self::ACCENT_OCHRE)
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(6.0))
        .min_size(Vec2::new(0.0, 28.0))
    }

    /// Danger / Destructive action button constructor (Redaction, permanent deletion).
    pub fn danger_button(text: impl Into<String>) -> egui::Button<'static> {
        egui::Button::new(
            RichText::new(text.into())
                .color(Color32::WHITE)
                .strong()
                .size(12.0),
        )
        .fill(Color32::from_rgb(185, 28, 28)) // Red 700
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(6.0))
        .min_size(Vec2::new(0.0, 26.0))
    }

    /// Frame container for pill-shaped tool clusters, page navigators, and search groups.
    pub fn pill_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(Color32::from_rgba_unmultiplied(15, 23, 42, 190))
            .stroke(Stroke::NONE)
            .rounding(Rounding::same(7.0))
            .inner_margin(egui::Margin::symmetric(4.0, 2.0))
    }

    /// Frame container for content cards, form groups, and sidebar items.
    pub fn card_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(Self::PANEL_SURFACE)
            .stroke(Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(51, 65, 85, 120),
            ))
            .rounding(Rounding::same(8.0))
            .inner_margin(egui::Margin::same(10.0))
    }

    /// Frame container for Tier 1 Header bar.
    pub fn header_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(Self::PANEL_DARK)
            .stroke(Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(12.0, 7.0))
    }

    /// Frame container for Tier 2 Action ribbon (subtle soft bottom divider line).
    pub fn ribbon_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(Self::PANEL_SURFACE)
            .stroke(Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(51, 65, 85, 90),
            ))
            .inner_margin(egui::Margin::symmetric(10.0, 5.0))
    }

    /// Render a subtle, low-opacity vertical divider between toolbar control clusters.
    pub fn vertical_divider(ui: &mut egui::Ui) {
        ui.add_space(2.0);
        let height = 18.0;
        let (rect, _resp) = ui.allocate_exact_size(Vec2::new(1.0, height), egui::Sense::hover());
        ui.painter().vline(
            rect.center().x,
            rect.y_range(),
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(148, 163, 184, 45)),
        );
        ui.add_space(2.0);
    }

    /// Frame container for bottom status bar.
    pub fn status_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(Self::PANEL_DARK)
            .stroke(Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(12.0, 6.0))
    }

    /// Segmented tool button helper (Active: solid Salmon; Inactive: clean borderless icon/label).
    pub fn segmented_tool_button(
        ui: &mut egui::Ui,
        is_active: bool,
        label: impl Into<String>,
    ) -> egui::Response {
        let (bg, stroke, text_color) = if is_active {
            (Self::ACCENT_SALMON, Stroke::NONE, Color32::WHITE)
        } else {
            (Color32::TRANSPARENT, Stroke::NONE, Self::TEXT_PRIMARY)
        };

        let btn = egui::Button::new(RichText::new(label.into()).color(text_color).size(12.0))
            .fill(bg)
            .stroke(stroke)
            .rounding(Rounding::same(6.0))
            .min_size(Vec2::new(0.0, 24.0));

        ui.add(btn)
    }
}
