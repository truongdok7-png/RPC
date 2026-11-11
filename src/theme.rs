// theme.rs - bảng màu & kiểu dáng (port phần "GUI: màu sắc & kiểu dáng" của main.py).

use egui::{Color32, Context, FontData, FontDefinitions, FontFamily, Visuals};

pub const UI_FONT: &str = "Segoe UI";
pub const GLASS: Color32 = Color32::from_rgb(0x15, 0x0f, 0x1d); // thanh bên luôn tối cho dễ đọc trên mọi ảnh
pub const SIDE_TEXT: Color32 = Color32::from_rgb(0xf6, 0xf1, 0xf7);
pub const SIDE_MUTED: Color32 = Color32::from_rgb(0xb6, 0xab, 0xc2);
pub const TALLY: Color32 = Color32::from_rgb(0x24, 0x1c, 0x2e); // nền đặc của khối trạng thái ở đáy thanh bên
pub const DOT_ON: Color32 = Color32::from_rgb(0x5d, 0xd6, 0xa2);
pub const DOT_WARN: Color32 = Color32::from_rgb(0xff, 0xbf, 0x5e);
pub const DOT_OFF: Color32 = Color32::from_rgb(0x8b, 0x81, 0x97);

// Kích thước khung (port từ main.py)
pub const MARGIN: f32 = 14.0;
pub const SIDE_W: f32 = 212.0;
pub const NAV_H: f32 = 34.0;
pub const HEAD_H: f32 = 80.0;
pub const GAP: f32 = 10.0;
pub const RADIUS: f32 = 18.0;
pub const TALLY_H: f32 = 104.0;
pub const HINT_W: f32 = 540.0;
pub const CARD_PAD: f32 = 8.0; // khung nội dung nằm lọt vào trong mặt phẳng bo góc
pub const GLASS_ALPHA: f32 = 0.55; // độ đậm lớp kính (0 = trong suốt, 1 = đặc)

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub card: Color32,
    pub field: Color32,
    pub hover: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_hi: Color32,
    pub accent_lo: Color32,
    pub on_accent: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub bad: Color32,
    pub sel: Color32,
    pub track: Color32,
    pub knob_off: Color32,
    pub watch: Color32,
    pub listen: Color32,
    pub play: Color32,
    pub idle: Color32,
}

fn hex(s: &str) -> Color32 {
    let b = s.as_bytes();
    let v = |i: usize| u8::from_str_radix(std::str::from_utf8(&b[i..i + 2]).unwrap_or("00"), 16).unwrap_or(0);
    Color32::from_rgb(v(1), v(3), v(5))
}

pub fn palette(name: &str) -> Palette {
    if name == "light" {
        let mk = |vals: [&str; 20]| Palette {
            card: hex(vals[0]),
            field: hex(vals[1]),
            hover: hex(vals[2]),
            line: hex(vals[3]),
            text: hex(vals[4]),
            muted: hex(vals[5]),
            accent: hex(vals[6]),
            accent_hi: hex(vals[7]),
            accent_lo: hex(vals[8]),
            on_accent: hex(vals[9]),
            ok: hex(vals[10]),
            warn: hex(vals[11]),
            bad: hex(vals[12]),
            sel: hex(vals[13]),
            track: hex(vals[14]),
            knob_off: hex(vals[15]),
            watch: hex(vals[16]),
            listen: hex(vals[17]),
            play: hex(vals[18]),
            idle: hex(vals[19]),
        };
        mk([
            "#f7f2f5", "#ffffff", "#f0e7ed", "#e0d5dc", "#2a2230", "#76697f",
            "#d63a6c", "#e24f7e", "#b82d5a", "#ffffff",
            "#177a56", "#8f5a0c", "#c93636", "#fbdbe6",
            "#c4b6c0", "#ffffff",
            "#d63a6c", "#0f7f6c", "#9c5a05", "#7a5ad6",
        ])
    } else {
        // dark (mặc định)
        let mk = |vals: [&str; 20]| Palette {
            card: hex(vals[0]),
            field: hex(vals[1]),
            hover: hex(vals[2]),
            line: hex(vals[3]),
            text: hex(vals[4]),
            muted: hex(vals[5]),
            accent: hex(vals[6]),
            accent_hi: hex(vals[7]),
            accent_lo: hex(vals[8]),
            on_accent: hex(vals[9]),
            ok: hex(vals[10]),
            warn: hex(vals[11]),
            bad: hex(vals[12]),
            sel: hex(vals[13]),
            track: hex(vals[14]),
            knob_off: hex(vals[15]),
            watch: hex(vals[16]),
            listen: hex(vals[17]),
            play: hex(vals[18]),
            idle: hex(vals[19]),
        };
        mk([
            "#1f1a27", "#2a2433", "#352e41", "#40374d", "#f5f0f6", "#aaa0b6",
            "#ff6f91", "#ff89a6", "#e4577b", "#2b0f1b",
            "#5dd6a2", "#ffbf5e", "#ff7a7a", "#47293f",
            "#4a4157", "#d2c9db",
            "#ff6f91", "#4fd1b8", "#ffb454", "#b79cff",
        ])
    }
}

/// Pha hai màu, t = 0 giữ c1, t = 1 thành c2.
pub fn mix(c1: Color32, c2: Color32, t: f32) -> Color32 {
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgb(m(c1.r(), c2.r()), m(c1.g(), c2.g()), m(c1.b(), c2.b()))
}

/// Từ chữ ở nhãn loại hoạt động ("Đang xem", "Đang nghe · rảnh"...) suy ra khoá màu.
pub fn kind_key(text: &str) -> Option<&'static str> {
    if text.contains("rảnh") {
        return Some("idle");
    }
    for (head, key) in [("Đang xem", "watch"), ("Đang nghe", "listen"), ("Đang chơi", "play")] {
        if text.starts_with(head) {
            return Some(key);
        }
    }
    None
}

pub fn kind_color(p: &Palette, text: &str) -> Color32 {
    match kind_key(text) {
        Some("watch") => p.watch,
        Some("listen") => p.listen,
        Some("play") => p.play,
        Some("idle") => p.idle,
        _ => p.muted,
    }
}

// ---------------------------------------------------------------- egui style

pub fn setup_fonts(cc: &eframe::CreationContext<'_>) {
    let mut fonts = FontDefinitions::default();
    let win_fonts = r"C:\Windows\Fonts";
    // Segoe UI (thường + đậm + nửa đậm + mảnh) với fallback nếu thiếu
    let candidates: [(&str, &str); 4] = [
        ("segoeui.ttf", "SegoeUI"),
        ("segoeuib.ttf", "SegoeUI-Bold"),
        ("segoeuisb.ttf", "SegoeUI-Semibold"),
        ("segoeuil.ttf", "SegoeUI-Light"),
    ];
    for (file, name) in candidates {
        let path = format!("{}\\{}", win_fonts, file);
        if let Ok(data) = std::fs::read(&path) {
            fonts.font_data.insert(name.to_string(), FontData::from_owned(data));
            fonts.families.get_mut(&FontFamily::Proportional).unwrap().push(name.to_string());
        }
    }
    // Consolas cho vùng log
    if let Ok(data) = std::fs::read(format!("{}\\consola.ttf", win_fonts)) {
        fonts.font_data.insert("Consolas".to_string(), FontData::from_owned(data));
        fonts
            .families
            .get_mut(&FontFamily::Monospace)
            .unwrap()
            .insert(0, "Consolas".to_string());
    }
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().dedup();
    cc.egui_ctx.set_fonts(fonts);
}

pub fn apply_style(ctx: &Context, p: &Palette) {
    let mut v = Visuals::default();
    v.panel_fill = p.card;
    v.window_fill = p.field;
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.hover;
    v.override_text_color = Some(p.text);
    v.widgets.noninteractive.bg_fill = p.field;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, p.muted);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, p.line);
    v.widgets.inactive.bg_fill = p.field;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, p.line);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, p.muted);
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, p.muted);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, p.text);
    v.widgets.active.bg_fill = p.line;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, p.text);
    v.widgets.open.bg_fill = p.field;
    v.selection.bg_fill = p.sel;
    v.selection.stroke = egui::Stroke::new(1.0_f32, p.text);
    v.hyperlink_color = p.accent;
    v.window_rounding = egui::Rounding::same(10.0);
    v.menu_rounding = egui::Rounding::same(6.0);
    v.window_stroke = egui::Stroke::new(1.0_f32, p.line);

    let mut style = (*ctx.style()).clone();
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.menu_margin = egui::Margin::same(6.0);
    style.spacing.interact_size = egui::vec2(64.0, 30.0);
    style.text_styles = std::collections::BTreeMap::from([
        (egui::TextStyle::Body, egui::FontId::proportional(13.5)),
        (egui::TextStyle::Button, egui::FontId::proportional(13.5)),
        (egui::TextStyle::Small, egui::FontId::proportional(11.0)),
        (egui::TextStyle::Heading, egui::FontId::proportional(18.0)),
        (egui::TextStyle::Monospace, egui::FontId::monospace(12.0)),
    ]);
    ctx.set_style(style);
}

pub fn accent(p: &Palette) -> Color32 {
    p.accent
}
