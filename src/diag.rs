// diag.rs - ghi log chẩn đoán + bắt panic.
//
// App chạy ở chế độ GUI (#![windows_subsystem = "windows"]) nên KHÔNG có cửa sổ console:
// nếu có lỗi thì Windows không hiện gì cả. Vì vậy mọi bước khởi động và mọi panic đều
// được ghi ra file "DiscordPresence.log" nằm cạnh exe (cùng chỗ với config.json).

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// File log nằm cạnh exe (cùng thư mục với config.json).
pub fn log_path() -> PathBuf {
    crate::config::base_dir().join("DiscordPresence.log")
}

/// Ghi một dòng vào file log. Mở/ghi/đóng mỗi lần nên gọi được từ nhiều luồng.
pub fn log(msg: &str) {
    let line = format!("[{}] {}\n", crate::util::timestamp_hms(), msg);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}

static MAIN_THREAD: OnceLock<std::thread::ThreadId> = OnceLock::new();
static FIRST_FRAME_STARTED: AtomicBool = AtomicBool::new(false);
static FIRST_FRAME_DONE: AtomicBool = AtomicBool::new(false);

/// Bước gần nhất đã chạy tới (để hộp thoại chẩn đoán biết app kẹt ở đâu).
static LAST_STEP: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Ghi log + ghi nhớ đây là bước gần nhất.
pub fn note(msg: &str) {
    if let Ok(mut g) = LAST_STEP.lock() {
        *g = msg.to_string();
    }
    log(msg);
}

/// Tên bước gần nhất.
pub fn last_step() -> String {
    LAST_STEP.lock().map(|g| g.clone()).unwrap_or_default()
}

/// Ghi log đúng một lần khi khung hình đầu tiên bắt đầu.
pub fn first_frame_started() {
    if !FIRST_FRAME_STARTED.swap(true, Ordering::Relaxed) {
        note("khung hình đầu tiên: bắt đầu vẽ");
    }
}

/// Ghi log đúng một lần khi khung hình đầu tiên vẽ xong (eframe chỉ hiện cửa sổ sau bước này).
pub fn first_frame_done() {
    if !FIRST_FRAME_DONE.swap(true, Ordering::Relaxed) {
        note("khung hình đầu tiên: vẽ xong -> cửa sổ sẽ hiện");
    }
}

/// Đã vẽ xong khung hình đầu tiên chưa (dùng cho luồng canh gác).
pub fn first_frame_reported() -> bool {
    FIRST_FRAME_DONE.load(Ordering::Relaxed)
}

static STAGE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Đánh dấu mốc trong khung hình đầu tiên (chỉ ghi mỗi mốc một lần) để biết app kẹt ở đâu.
pub fn stage(n: usize, name: &str) {
    if FIRST_FRAME_DONE.load(Ordering::Relaxed) {
        return;
    }
    if n > STAGE.load(Ordering::Relaxed) {
        STAGE.store(n, Ordering::Relaxed);
        note(&format!("khung hình đầu tiên: {name}"));
    }
}

/// Cài panic hook: ghi lỗi ra file; nếu lỗi ở luồng giao diện thì hiện hộp thoại
/// (thay cho màn hình "không có gì xảy ra").
pub fn init() {
    let _ = MAIN_THREAD.set(std::thread::current().id());

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "(không rõ nội dung)".to_string()
        };
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        log(&format!("PANIC: {payload} @ {loc}"));

        let is_main = MAIN_THREAD
            .get()
            .map(|id| *id == std::thread::current().id())
            .unwrap_or(false);
        if is_main {
            crate::win::message_box(
                "Discord Presence - lỗi",
                &format!(
                    "Ứng dụng gặp lỗi và phải dừng:\n\n{payload}\n\n{loc}\n\nChi tiết đã ghi vào:\n{}",
                    log_path().display()
                ),
            );
        }
        default_hook(info);
    }));

    log("==================================================");
    log(&format!("khởi động Discord Presence {}", crate::util::APP_VERSION));
    log(&format!(
        "exe: {}",
        std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()
    ));
    log(&format!("tham số: {:?}", std::env::args().collect::<Vec<String>>()));
}
