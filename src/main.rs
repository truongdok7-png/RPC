// main.rs - điểm vào chương trình (port hàm main() của main.py).
//
// Chạy: cargo run --release -- --minimized  (mở sẵn dưới khay, giống lệnh autostart)

// Ẩn cửa sổ terminal khi chạy (app GUI). Bỏ dòng này nếu muốn xem log khi debug.
#![windows_subsystem = "windows"]

mod activity;
mod app;
mod bg;
mod bot;
mod browser;
mod config;
mod discord;
mod engine;
mod icons;
mod media;
mod pages;
mod theme;
mod util;
mod win;

use eframe::egui;

/// "960x680" -> (960.0, 680.0); sai định dạng thì dùng mặc định (giống root.geometry của Tk).
fn parse_window_size(s: &str) -> [f32; 2] {
    let mut parts = s.splitn(2, 'x');
    let w = parts.next().unwrap_or("").trim().parse::<f32>().ok();
    let h = parts.next().unwrap_or("").trim().parse::<f32>().ok();
    match (w, h) {
        (Some(w), Some(h)) if w >= 500.0 && h >= 500.0 && w < 8000.0 && h < 8000.0 => [w, h],
        _ => [960.0, 680.0],
    }
}

fn main() {
    let start_hidden = std::env::args().any(|a| a == "--minimized");

    // một bản chạy duy nhất (port single_instance); handle phải giữ sống suốt chương trình
    let _mutex = win::single_instance("Local\\DiscordPresenceGUI");
    if _mutex.is_none() {
        win::message_box(
            "Discord Presence",
            "App đang chạy rồi. Hãy tìm biểu tượng dưới khay hệ thống và bấm đúp để mở.",
        );
        return;
    }

    // đọc config trước để lấy kích thước cửa sổ đã lưu
    let cfg = config::load_config();
    let size = parse_window_size(&cfg.get_str("window_size"));

    // biểu tượng app: file app.ico cạnh exe (không có thì hình vẽ mặc định)
    let (iw, ih, rgba) = app::make_icon_rgba();
    let icon = egui::IconData { rgba, width: iw, height: ih };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Discord Presence {}", util::APP_VERSION))
            .with_inner_size(size)
            .with_min_inner_size([900.0, 680.0])
            .with_icon(icon),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Discord Presence",
        options,
        Box::new(move |cc| Box::new(app::App::new(cc, cfg, start_hidden))),
    ) {
        win::message_box("Discord Presence", &format!("Không thể khởi động giao diện: {e}"));
    }
}
