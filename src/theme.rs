#![allow(dead_code)]
use egui::{Color32, Context, Rounding, Stroke, Visuals, FontFamily, RichText, Vec2, LayerId, pos2};

// ── Crimson Cyber Dark Palette (#780606 top to #4E0707 bottom) ────
pub const BG_BASE: Color32       = Color32::from_rgb(18, 18, 20);     // Deep dark base (#121214)
pub const BG_SURFACE: Color32    = Color32::from_rgb(22, 24, 32);     // Dark panel / card (#161820)
pub const BG_RAISED: Color32     = Color32::from_rgb(28, 34, 50);     // Input / button fill (#1c2232)
pub const BG_OVERLAY: Color32    = Color32::from_rgb(6, 52, 100);     // Active hover fill (#063464)

// Accent colors
pub const ACCENT_BLUE: Color32   = Color32::from_rgb(96, 165, 250);   // #60a5fa blue-400
pub const ACCENT_CYAN: Color32   = Color32::from_rgb(56, 189, 248);   // #38bdf8 cyan-400
pub const ACCENT_TEAL: Color32   = Color32::from_rgb(45, 212, 191);   // #2dd4bf teal-400
pub const ACCENT_GREEN: Color32  = Color32::from_rgb(74, 222, 128);   // #4ade80 green-400
pub const ACCENT_AMBER: Color32  = Color32::from_rgb(251, 191, 36);   // #fbbf24 amber-400
pub const ACCENT_ORANGE: Color32 = Color32::from_rgb(251, 146, 60);   // #fb923c orange-400
pub const ACCENT_RED: Color32    = Color32::from_rgb(248, 113, 113);  // #f87171 red-400
pub const ACCENT_VIOLET: Color32 = Color32::from_rgb(192, 132, 252);  // #c084fc violet-400

// Text hierarchy - High contrast
pub const TEXT_0: Color32        = Color32::from_rgb(248, 250, 252);  // #f8fafc slate-50 (Crisp White)
pub const TEXT_1: Color32        = Color32::from_rgb(226, 232, 240);  // #e2e8f0 slate-200 (Secondary Text)
pub const TEXT_2: Color32        = Color32::from_rgb(160, 174, 192);  // #a0aec0 slate-400 (Muted Text)

// Syntax Highlighting Colors
pub const SYNTAX_KEY: Color32    = Color32::from_rgb(251, 191, 36);   // Amber (#fbbf24) for Header keys
pub const SYNTAX_VAL: Color32    = Color32::from_rgb(248, 250, 252);  // Crisp white for Header values
pub const SYNTAX_METHOD: Color32 = Color32::from_rgb(56, 189, 248);   // Cyan for HTTP Methods
pub const SYNTAX_STRING: Color32 = Color32::from_rgb(74, 222, 128);   // Green for JSON/HTML Strings
pub const SYNTAX_NUMBER: Color32 = Color32::from_rgb(251, 146, 60);   // Orange for Numbers

// Borders
pub const BORDER: Color32       = Color32::from_rgb(2, 80, 150);      // Ocean blue border (#025096)
pub const BORDER_DIM: Color32   = Color32::from_rgb(2, 50, 100);      // Dim blue border (#023264)

// Semantic row states
pub const ROW_SELECTED: Color32 = Color32::from_rgba_premultiplied(96, 165, 250, 40);
pub const ROW_HOVER: Color32    = Color32::from_rgba_premultiplied(255, 255, 255, 12);

// ── HTTP Method Colors ────────────────────────────────────────────
pub fn method_color(m: &str) -> Color32 {
    match m {
        "GET"     => ACCENT_CYAN,
        "POST"    => ACCENT_BLUE,
        "PUT"     => ACCENT_AMBER,
        "PATCH"   => ACCENT_VIOLET,
        "DELETE"  => ACCENT_RED,
        "HEAD"    => TEXT_1,
        "OPTIONS" => TEXT_2,
        _         => TEXT_1,
    }
}

// ── Status Code Colors ────────────────────────────────────────────
pub fn status_color(c: u16) -> Color32 {
    match c {
        200..=299 => ACCENT_GREEN,
        300..=399 => ACCENT_BLUE,
        400..=499 => ACCENT_ORANGE,
        500..=599 => ACCENT_RED,
        _         => TEXT_2,
    }
}

// ── Smooth Strip-Based Background Gradient (#0172B0 Top -> #023C85 Mid -> #121214 Bottom) ──
pub fn paint_background_gradient(ctx: &Context) {
    let rect = ctx.screen_rect();
    let painter = ctx.layer_painter(LayerId::background());

    let steps = 60;
    let h = rect.height() / (steps as f32);

    // 3-stop gradient: Top #0172B0 → Mid #023C85 → Bottom #121214
    let top_r = 1.0_f32;   let top_g = 114.0_f32; let top_b = 176.0_f32;  // #0172B0
    let mid_r = 2.0_f32;   let mid_g = 60.0_f32;  let mid_b = 133.0_f32;  // #023C85
    let bot_r = 18.0_f32;  let bot_g = 18.0_f32;  let bot_b = 20.0_f32;   // #121214

    for i in 0..steps {
        let t = (i as f32) / (steps as f32);

        // Two-phase interpolation: top→mid (0..0.4), mid→bottom (0.4..1.0)
        let (r, g, b) = if t < 0.4 {
            let p = t / 0.4;
            (
                (top_r + (mid_r - top_r) * p) as u8,
                (top_g + (mid_g - top_g) * p) as u8,
                (top_b + (mid_b - top_b) * p) as u8,
            )
        } else {
            let p = (t - 0.4) / 0.6;
            (
                (mid_r + (bot_r - mid_r) * p) as u8,
                (mid_g + (bot_g - mid_g) * p) as u8,
                (mid_b + (bot_b - mid_b) * p) as u8,
            )
        };

        let y0 = rect.top() + (i as f32) * h;
        let y1 = if i == steps - 1 { rect.bottom() } else { y0 + h + 0.5 };

        let strip_rect = egui::Rect::from_min_max(
            pos2(rect.left(), y0),
            pos2(rect.right(), y1),
        );

        painter.rect_filled(strip_rect, Rounding::ZERO, Color32::from_rgb(r, g, b));
    }
}

// ── Apply Theme ───────────────────────────────────────────────────
pub fn apply_theme(ctx: &Context) {
    let mut v = Visuals::dark();

    v.panel_fill = Color32::from_rgba_premultiplied(18, 18, 22, 180);
    v.window_fill = BG_SURFACE;
    v.extreme_bg_color = Color32::from_rgb(12, 12, 16);
    v.faint_bg_color = BG_RAISED;

    v.selection.bg_fill = Color32::from_rgba_premultiplied(96, 165, 250, 60);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT_BLUE);

    v.widgets.noninteractive.bg_fill = BG_SURFACE;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_1);
    v.widgets.noninteractive.bg_stroke = Stroke::new(0.5_f32, BORDER_DIM);
    v.widgets.noninteractive.rounding = Rounding::same(3.0);

    v.widgets.inactive.bg_fill = BG_RAISED;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_0);
    v.widgets.inactive.bg_stroke = Stroke::new(0.5_f32, BORDER);
    v.widgets.inactive.rounding = Rounding::same(3.0);

    v.widgets.hovered.bg_fill = BG_OVERLAY;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_0);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_BLUE);
    v.widgets.hovered.rounding = Rounding::same(3.0);

    v.widgets.active.bg_fill = BG_OVERLAY;
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT_0);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_BLUE);
    v.widgets.active.rounding = Rounding::same(3.0);

    v.widgets.open.bg_fill = BG_OVERLAY;
    v.widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT_0);

    v.window_rounding = Rounding::same(4.0);
    v.window_stroke = Stroke::new(1.0_f32, BORDER);
    v.override_text_color = Some(TEXT_0);

    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 5.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    ctx.set_style(style);
}

// ── Helpers ───────────────────────────────────────────────────────
pub fn color_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), a)
}

pub fn section_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(BG_SURFACE)
        .rounding(Rounding::same(4.0))
        .stroke(Stroke::new(1.0_f32, BORDER))
        .inner_margin(10.0)
}

pub fn mono(text: &str) -> RichText {
    RichText::new(text).family(FontFamily::Monospace).size(12.0).color(TEXT_0)
}
