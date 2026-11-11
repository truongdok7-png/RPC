// util.rs - các hàm phụ chung (port từ main.py)

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub const APP_NAME: &str = "DiscordPresence";
pub const APP_VERSION: &str = "2.2.0";
pub const ICON_FILE: &str = "app.ico";
pub const BG_FILE: &str = "background.jpg";
pub const BOT_PORT: u16 = 3000;
pub const LOG_MAX: usize = 500;

// Bảng mã hoá giống urllib.parse.quote(..., safe=""): giữ chữ/số và _.-~
const PY_QUOTE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'_')
    .remove(b'-')
    .remove(b'.')
    .remove(b'~');

pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn timestamp_hms() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

pub fn quote_like(s: &str) -> String {
    utf8_percent_encode(s, PY_QUOTE).to_string()
}

// ---------------------------------------------------------------- kiểu/khoá activity

pub fn type_code(kind: &str) -> i64 {
    match kind {
        "watching" => 2,   // ActivityType.WATCHING
        "listening" => 1,  // ActivityType.LISTENING
        _ => 0,            // ActivityType.PLAYING
    }
}

pub fn kind_of_code(code: i64) -> &'static str {
    match code {
        2 => "watching",
        1 => "listening",
        _ => "playing",
    }
}

pub const TYPE_LABEL: [(&str, &str); 3] = [
    ("watching", "Đang xem"),
    ("listening", "Đang nghe"),
    ("playing", "Đang chơi"),
];

pub fn type_label(kind: &str) -> &'static str {
    for (k, l) in TYPE_LABEL {
        if k == kind {
            return l;
        }
    }
    "Đang xem"
}

pub fn label_type(label: &str) -> &'static str {
    for (k, l) in TYPE_LABEL {
        if l == label {
            return k;
        }
    }
    "watching"
}

// ---------------------------------------------------------------- đọc giá trị JSON kiểu Python

pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// str(x or "") của Python trên giá trị JSON.
pub fn as_str(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(true)) => "True".to_string(),
        Some(Value::Bool(false)) => "False".to_string(),
        Some(other) => other.to_string(),
    }
}

pub fn as_i64(v: Option<&Value>, default: i64) -> i64 {
    match v {
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i
            } else {
                n.as_f64().map(|f| f as i64).unwrap_or(default)
            }
        }
        Some(Value::String(s)) => {
            let t = s.trim();
            t.parse::<i64>().ok().or_else(|| t.parse::<f64>().ok().map(|f| f as i64)).unwrap_or(default)
        }
        _ => default,
    }
}

pub fn as_f64(v: Option<&Value>, default: f64) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
        Some(Value::String(s)) => s.trim().replace(',', ".").parse::<f64>().unwrap_or(default),
        _ => default,
    }
}

// ---------------------------------------------------------------- regex dùng chung (khởi tạo 1 lần)

fn re_once(cell: &OnceLock<Regex>, pattern: &str) -> Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("regex"))
        .clone()
}

pub fn regex_cache(pattern: &'static str) -> Regex {
    static MAP: OnceLock<std::sync::Mutex<std::collections::HashMap<&'static str, Regex>>> = OnceLock::new();
    let map = MAP.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut guard = map.lock().unwrap();
    guard.entry(pattern).or_insert_with(|| Regex::new(pattern).expect("regex")).clone()
}

// ---------------------------------------------------------------- định dạng

pub fn clip(text: &str) -> String {
    let mut text = text.trim().chars().take(128).collect::<String>();
    // Discord yêu cầu tối thiểu 2 ký tự
    if text.chars().count() == 1 {
        text.push('\u{200b}');
    }
    text
}

pub fn fmt_clock(sec: i64) -> String {
    let sec = sec.max(0);
    let m = sec / 60;
    let s = sec % 60;
    let h = m / 60;
    let m = m % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

pub fn fmt_dur(sec: i64) -> String {
    let sec = sec.max(0);
    let h = sec / 3600;
    let m = (sec % 3600) / 60;
    if h > 0 {
        return format!("{} giờ {} phút", h, m);
    }
    if m > 0 {
        return format!("{} phút", m);
    }
    format!("{} giây", sec)
}

/// Điền placeholder {x} vào mẫu.
pub fn fill(template: &str, vals: &Map<String, Value>) -> String {
    let mut out = template.to_string();
    for (k, v) in vals {
        let needle = format!("{{{}}}", k);
        if out.contains(&needle) {
            out = out.replace(&needle, &as_str(Some(v)));
        }
    }
    out
}

/// Điền placeholder; {x_q} = bản đã mã hoá URL.
pub fn fill_url(template: &str, vals: &Map<String, Value>) -> String {
    let mut out = template.to_string();
    for (k, v) in vals {
        let needle = format!("{{{}_q}}", k);
        if out.contains(&needle) {
            out = out.replace(&needle, &quote_like(&as_str(Some(v))));
        }
        let needle = format!("{{{}}}", k);
        if out.contains(&needle) {
            out = out.replace(&needle, &as_str(Some(v)));
        }
    }
    out
}

pub fn common_vals(title: &str, artist: &str, site: &str, browser: &str, url: &str, domain: &str) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("title".into(), Value::String(title.to_string()));
    m.insert("artist".into(), Value::String(artist.to_string()));
    m.insert("site".into(), Value::String(site.to_string()));
    m.insert("browser".into(), Value::String(browser.to_string()));
    m.insert("url".into(), Value::String(url.to_string()));
    m.insert("domain".into(), Value::String(domain.to_string()));
    m
}

/// Chuẩn hoá log bot: bỏ mã màu ANSI và emoji ngoài BMP (giống Tk của bản Python).
pub fn bot_clean(line: &str) -> String {
    let re = regex_cache("\\x1b\\[[0-9;]*[A-Za-z]");
    let line = re.replace_all(line, "");
    let line: String = line
        .chars()
        .filter(|c| (*c as u32) < 0x10000)
        .collect();
    line.trim_end().to_string()
}

pub fn bot_line_is_error(line: &str) -> bool {
    regex_cache("(?i)lỗi|error|❌|exception|failed|unhandled").is_match(line)
}

pub fn norm(s: &str) -> String {
    regex_cache("\\W+").replace_all(&s.to_lowercase(), " ").trim().to_string()
}

/// Kéo cửa sổ/nội dung ra khỏi vùng nhìn thấy.
pub fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}
