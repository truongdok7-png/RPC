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
mod diag;
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
    // ghi log + bắt panic ra file (app chạy không có console nên lỗi sẽ hoàn toàn im lặng)
    diag::init();

    let start_hidden = std::env::args().any(|a| a == "--minimized");
    diag::note(&format!("chế độ: {}", if start_hidden { "--minimized" } else { "bình thường" }));

    // một bản chạy duy nhất (port single_instance); handle phải giữ sống suốt chương trình
    diag::note("tạo mutex một bản chạy duy nhất");
    let _mutex = win::single_instance("Local\\DiscordPresenceGUI");
    if _mutex.is_none() {
        diag::note("đã có bản khác đang chạy -> thử đưa cửa sổ cũ lên trước");
        win::show_existing_main_window();
        win::message_box(
            "Discord Presence",
            "App đang chạy rồi. Hãy tìm biểu tượng dưới khay hệ thống và bấm đúp để mở.",
        );
        return;
    }
    diag::note("đã giữ mutex một bản chạy duy nhất");

    // đọc config trước để lấy kích thước cửa sổ đã lưu
    let cfg = config::load_config();
    let size = parse_window_size(&cfg.get_str("window_size"));
    diag::note(&format!("đã đọc config, window_size = {:?}", size));

    // biểu tượng app: file app.ico cạnh exe (không có thì hình vẽ mặc định)
    let (iw, ih, rgba) = app::make_icon_rgba();
    let icon = egui::IconData { rgba, width: iw, height: ih };
    diag::note(&format!("biểu tượng: {}x{}", iw, ih));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Discord Presence {}", util::APP_VERSION))
            .with_inner_size(size)
            .with_min_inner_size([900.0, 680.0])
            .with_icon(icon),
        ..Default::default()
    };

    // canh gác: nếu sau 8 giây vẫn chưa vẽ xong khung hình đầu thì ghi lại để biết kẹt ở đâu,
    // đồng thời hiện cửa sổ bằng WinAPI và báo bằng hộp thoại (khỏi phải đi tìm file log).
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(8));
        if !diag::first_frame_reported() {
            let step = diag::last_step();
            let state = win::main_window_state();
            diag::log(&format!(
                "CẢNH BÁO: sau 8 giây chưa vẽ xong khung hình đầu -> bước gần nhất: {step} | cửa sổ: {state}"
            ));
            if !start_hidden {
                diag::log("canh gác: gọi ShowWindow để hiện cửa sổ dù chưa vẽ xong");
                win::show_own_main_window();
                win::message_box(
                    "Discord Presence - chẩn đoán",
                    &format!(
                        "Ứng dụng chưa vẽ xong khung hình đầu sau 8 giây.\n\n\
                         Bước gần nhất:\n{step}\n\n\
                         Cửa sổ:\n{state}\n\n\
                         Chi tiết đã ghi vào:\n{}\n\n\
                         Đây là thông tin chẩn đoán, hãy gửi file log ở trên.",
                        diag::log_path().display()
                    ),
                );
            }
        }
    });

    diag::note("gọi eframe::run_native");
    let result = eframe::run_native(
        "Discord Presence",
        options,
        Box::new(move |cc| {
            diag::note("eframe: đang tạo App");
            Box::new(app::App::new(cc, cfg, start_hidden))
        }),
    );
    match &result {
        Ok(()) => diag::log("eframe::run_native kết thúc bình thường"),
        Err(e) => diag::log(&format!("eframe::run_native LỖI: {e}")),
    }
    if let Err(e) = result {
        win::message_box("Discord Presence", &format!("Không thể khởi động giao diện: {e}"));
    }
    diag::log("thoát chương trình");
}
