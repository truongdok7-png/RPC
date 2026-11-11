// win.rs - các hàm WinAPI (port từ phần "Windows helpers" của main.py)

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Mutex;

use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    CreateMutexW, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetSystemMetrics, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible,
};

use crate::util::regex_cache;

pub const BROWSERS: [(&str, &str); 6] = [
    ("chrome.exe", "Google Chrome"),
    ("msedge.exe", "Microsoft Edge"),
    ("firefox.exe", "Mozilla Firefox"),
    ("brave.exe", "Brave"),
    ("opera.exe", "Opera"),
    ("vivaldi.exe", "Vivaldi"),
];

// ---------------------------------------------------------------- tiện ích chuỗi UTF-16

pub fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

pub fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

// ---------------------------------------------------------------- quy trình / exe

fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let res = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut size);
        let _ = CloseHandle(handle);
        if res.is_err() {
            return None;
        }
        let full = from_wide(&buf[..size as usize]);
        PathBuf::from(&full)
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
    }
}

/// (tiêu đề sạch, tên trình duyệt, là_tab_riêng_tư, hwnd) nếu hwnd là cửa sổ trình duyệt, ngược lại None.
pub fn browser_window_info(hwnd: HWND) -> Option<(String, String, bool, isize)> {
    let mut pid: u32 = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return None;
    }
    let exe = process_name(pid)?;
    let browser = BROWSERS.iter().find(|(e, _)| *e == exe)?.1.to_string();

    let mut buf = [0u16; 1024];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if n <= 0 {
        return None;
    }
    let mut title = from_wide(&buf[..(n as usize).min(buf.len())]);
    if title.is_empty() {
        return None;
    }

    // 1. Xoá đuôi tên trình duyệt gốc (vd: - Google Chrome)
    title = regex_cache("(?i)\\s[-\u{2013}\u{2014}]\\s(Google Chrome|Microsoft.? Edge|Mozilla Firefox|Brave|Opera|Vivaldi).*$")
        .replace_all(&title, "")
        .trim()
        .to_string();

    // 2. Xoá chữ "và X trang nữa" cùng mọi thứ phía sau (bao gồm Profile dính kèm)
    title = regex_cache("(?i)\\s+(và|and)\\s+\\d+\\s+(trang nữa|more pages|more tabs).*$")
        .replace_all(&title, "")
        .trim()
        .to_string();

    // 3. Xoá tên Profile ở cuối cửa sổ khi không có chữ nhóm tab
    title = regex_cache("(?i)\\s+[-\u{2013}\u{2014}]\\s+(Profile\\s*\\d+|Cá nhân|Personal|Công việc|Work)$")
        .replace_all(&title, "")
        .trim()
        .to_string();

    let private = regex_cache("(?i)inprivate|incognito|private browsing|ẩn danh").is_match(&title);
    Some((title, browser, private, hwnd.0 as isize))
}

/// (tiêu đề, tên trình duyệt, hwnd) của cửa sổ đang focus, nếu là trình duyệt.
pub fn foreground_browser_title() -> Option<(String, String, isize)> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0 == 0 {
        return None;
    }
    match browser_window_info(hwnd) {
        Some((t, b, private, _)) if !private => Some((t, b, hwnd.0 as isize)),
        _ => None, // không bao giờ lộ tab riêng tư
    }
}

// ---------------------------------------------------------------- EnumWindows

static FOUND: Mutex<Vec<(String, String, bool, isize)>> = Mutex::new(Vec::new());

unsafe extern "system" fn top_enum_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let mut found = FOUND.lock().unwrap();
    if !found.is_empty() {
        return BOOL(1); // đã tìm thấy -> dừng sớm
    }
    if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
        return BOOL(1);
    }
    if let Some(info) = browser_window_info(hwnd) {
        found.push(info);
    }
    BOOL(1)
}

/// Cửa sổ trình duyệt nằm TRÊN CÙNG (không cần đang focus). Trả về (tiêu đề, trình duyệt, hwnd).
pub fn top_browser_title() -> Option<(String, String, isize)> {
    FOUND.lock().unwrap().clear();
    unsafe {
        let _ = EnumWindows(Some(top_enum_proc), LPARAM(0));
    }
    let found = FOUND.lock().unwrap();
    if found.is_empty() || found[0].2 {
        return None;
    }
    Some((found[0].0.clone(), found[0].1.clone(), found[0].3))
}

unsafe extern "system" fn any_enum_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let mut found = FOUND.lock().unwrap();
    if !found.is_empty() {
        return BOOL(1);
    }
    if IsWindowVisible(hwnd).as_bool() {
        if let Some(info) = browser_window_info(hwnd) {
            found.push(info);
        }
    }
    BOOL(1)
}

/// Còn cửa sổ trình duyệt nào đang mở không (kể cả đang thu nhỏ).
pub fn any_browser_window() -> bool {
    FOUND.lock().unwrap().clear();
    unsafe {
        let _ = EnumWindows(Some(any_enum_proc), LPARAM(0));
    }
    // lỗi thì coi như còn mở, tránh reset nhầm
    !FOUND.lock().unwrap().is_empty()
}

// ---------------------------------------------------------------- idle (chuột / bàn phím)

/// Số giây kể từ lần cuối bạn động vào chuột/bàn phím.
pub fn idle_seconds() -> f64 {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            return ((GetTickCount().wrapping_sub(info.dwTime)) & 0xFFFF_FFFF) as f64 / 1000.0;
        }
        0.0
    }
}

// ---------------------------------------------------------------- màn hình

/// (x, y, w, h) của vùng làm việc chính (SM_XVIRTUALSCREEN..SM_CYVIRTUALSCREEN = 76..79).
pub fn virtual_screen() -> (i32, i32, i32, i32) {
    unsafe {
        (
            GetSystemMetrics(76),
            GetSystemMetrics(77),
            GetSystemMetrics(78),
            GetSystemMetrics(79),
        )
    }
}

pub fn screen_size() -> (i32, i32) {
    unsafe { (GetSystemMetrics(0), GetSystemMetrics(1)) } // SM_CXSCREEN, SM_CYSCREEN
}

// ---------------------------------------------------------------- hộp thoại hệ thống & mở thư mục

pub fn message_box(title: &str, text: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
    let tw = to_wide(title);
    let tx = to_wide(text);
    unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR(tx.as_ptr()),
            PCWSTR(tw.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// os.startfile(path) của Python.
pub fn startfile(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer")
        .arg(path.as_os_str())
        .spawn();
}

// ---------------------------------------------------------------- chạy một bản duy nhất

/// Port single_instance: tạo named mutex "Local\\DiscordPresenceGUI".
/// Trả về Some(handle) nếu đây là bản đầu tiên (handle giữ sống đến khi app thoát),
/// None nếu app đã chạy rồi.
pub fn single_instance(name: &str) -> Option<HANDLE> {
    let wide = to_wide(name);
    let handle = unsafe { CreateMutexW(None, BOOL(0), PCWSTR(wide.as_ptr())) };
    match handle {
        Ok(h) => {
            if h.is_invalid() {
                None
            } else if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                None
            } else {
                Some(h)
            }
        }
        Err(_) => None,
    }
}

// ---------------------------------------------------------------- khởi động cùng Windows

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// `"<exe>" --minimized`
pub fn autostart_cmd() -> String {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    format!("\"{}\" --minimized", exe)
}

pub fn set_autostart(enable: bool) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
        .map_err(|e| e.to_string())?;
    if enable {
        key.set_value("DiscordPresence", &autostart_cmd()).map_err(|e| e.to_string())?;
    } else {
        match key.delete_value("DiscordPresence") {
            Ok(()) => {}
            Err(winreg::RegError::NotFound(_, _)) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- single instance

pub enum MutexGuard {
    Owned(HANDLE),
    None,
}

/// Tạo mutex "Local\\DiscordPresenceGUI"; trả về None nếu app đã chạy.
pub fn single_instance() -> MutexGuard {
    use windows::Win32::System::Threading::CreateMutexW;
    unsafe {
        match CreateMutexW(None, false, w!("Local\\DiscordPresenceGUI")) {
            Ok(h) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    MutexGuard::None
                } else {
                    MutexGuard::Owned(h)
                }
            }
            Err(_) => MutexGuard::None,
        }
    }
}
