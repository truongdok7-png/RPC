// config.rs - đọc/ghi config.json, chuẩn hoá (normalize) y hệt main.py.

use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::util::{as_f64, as_i64, as_str, regex_cache, truthy};

pub const LEGACY_KEYS: [&str; 4] = ["custom_sites", "manual", "presets", "paused"];
pub const LOCAL_KEYS: [&str; 4] = ["bg_image", "window_size", "seen_version", "mini_pos"];

pub const DEFAULT_FAVICON_TPL: &str = "https://www.google.com/s2/favicons?domain={domain}&sz=128";

// ---------------------------------------------------------------- đường dẫn

pub fn base_dir() -> &'static PathBuf {
    static BASE: OnceLock<PathBuf> = OnceLock::new();
    BASE.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

pub fn config_path() -> PathBuf {
    base_dir().join("config.json")
}

pub fn bot_dir() -> PathBuf {
    base_dir().join("bot")
}

pub fn resource_path(name: &str) -> PathBuf {
    let p = base_dir().join(name);
    if p.exists() {
        return p;
    }
    base_dir().join(name)
}

// ---------------------------------------------------------------- nút bấm

pub fn empty_buttons() -> Vec<Value> {
    vec![json!({"label": "", "url": ""}), json!({"label": "", "url": ""})]
}

/// Luôn trả về đúng 2 nút {label, url}.
pub fn norm_buttons(raw: Option<&Value>) -> Vec<Value> {
    let mut out = empty_buttons();
    if let Some(Value::Array(arr)) = raw {
        for (i, b) in arr.iter().take(2).enumerate() {
            if let Value::Object(_) = b {
                out[i] = json!({
                    "label": as_str(b.get("label")),
                    "url": as_str(b.get("url")),
                });
            }
        }
    }
    out
}

pub fn buttons_to_pairs(v: Option<&Value>) -> Vec<(String, String)> {
    norm_buttons(v)
        .into_iter()
        .map(|b| (as_str(b.get("label")), as_str(b.get("url"))))
        .collect()
}

pub fn pairs_to_buttons(pairs: &[(String, String)]) -> Value {
    Value::Array(
        pairs
            .iter()
            .map(|(l, u)| json!({"label": l.trim(), "url": u.trim()}))
            .collect(),
    )
}

// ---------------------------------------------------------------- site

pub fn new_site(
    m: &str,
    name: &str,
    kind: &str,
    enabled: bool,
    state: &str,
    details: &str,
    large_image: &str,
    timer: bool,
    domain: &str,
    favicon: bool,
    buttons: Option<&Value>,
    buttons_off: bool,
) -> Value {
    json!({
        "enabled": enabled,
        "match": m,
        "name": name,
        "type": kind,
        "state": state,
        "details": details,
        "large_image": large_image,
        "timer": timer,
        "domain": domain,
        "favicon": favicon,
        "buttons": Value::Array(norm_buttons(buttons)),
        "buttons_off": buttons_off,
    })
}

pub fn default_sites() -> Vec<Value> {
    vec![
        new_site("youtube", "YouTube", "watching", true, "", "", "", true, "youtube.com", true, None, false),
        new_site("netflix", "Netflix", "watching", true, "", "", "", true, "netflix.com", true, None, false),
        new_site("twitch", "Twitch", "watching", true, "", "", "", true, "twitch.tv", true, None, false),
        new_site("tiktok", "TikTok", "watching", true, "", "", "", true, "tiktok.com", true, None, false),
        new_site("bilibili", "Bilibili", "watching", true, "", "", "", true, "bilibili.com", true, None, false),
        new_site("animevietsub", "AnimeVietSub", "watching", true, "", "", "", true, "", true, None, false),
        new_site("spotify", "Spotify", "listening", true, "", "", "", true, "spotify.com", true, None, false),
        new_site("soundcloud", "SoundCloud", "listening", true, "", "", "", true, "soundcloud.com", true, None, false),
        new_site("zing mp3", "Zing MP3", "listening", true, "", "", "", true, "zingmp3.vn", true, None, false),
        new_site("nhaccuatui", "NhacCuaTui", "listening", true, "", "", "", true, "nhaccuatui.com", true, None, false),
    ]
}

// ---------------------------------------------------------------- config

pub fn default_config() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("client_id".into(), json!(""));
    m.insert("interval_seconds".into(), json!(5));
    m.insert("large_image".into(), json!("logo"));
    m.insert("show_media".into(), json!(true));
    m.insert("show_browsing".into(), json!(true));
    m.insert("browsing_requires_focus".into(), json!(false));
    m.insert("browsing_state".into(), json!("Đang duyệt web ({browser})"));
    m.insert("show_large_text".into(), json!(true));
    m.insert("blacklist_keywords".into(), json!([]));
    m.insert("sites".into(), Value::Null);
    m.insert("auto_start_presence".into(), json!(true));
    m.insert("minimize_to_tray".into(), json!(true));
    m.insert("start_with_windows".into(), json!(false));
    m.insert("favicon_mode".into(), json!("small"));
    m.insert("favicon_url_template".into(), json!(DEFAULT_FAVICON_TPL));
    m.insert("read_address_bar".into(), json!(true));
    m.insert("buttons_enabled".into(), json!(true));
    m.insert("buttons".into(), Value::Array(empty_buttons()));
    m.insert("idle_minutes".into(), json!(0));
    m.insert("show_idle".into(), json!(true));
    m.insert("idle_details".into(), json!("Đang rảnh"));
    m.insert("quiet_enabled".into(), json!(false));
    m.insert("quiet_from".into(), json!("23:00"));
    m.insert("quiet_to".into(), json!("07:00"));
    m.insert("theme".into(), json!("dark"));
    m.insert("bg_image".into(), json!(""));
    m.insert("bg_dim".into(), json!(35));
    m.insert("bg_blur".into(), json!(0));
    m.insert("window_size".into(), json!(""));
    m.insert("seen_version".into(), json!(""));
    m.insert("toast_enabled".into(), json!(true));
    m.insert("mini_pos".into(), json!(""));
    m.insert("idle_custom_enabled".into(), json!(false));
    m.insert("idle_custom_client_id".into(), json!(""));
    m.insert("idle_custom_type".into(), json!("listening"));
    m.insert("idle_custom_details".into(), json!("Đang rảnh"));
    m.insert("idle_custom_state".into(), json!(""));
    m.insert("idle_custom_large_image".into(), json!(""));
    m.insert("idle_custom_large_text".into(), json!(""));
    m.insert("idle_custom_timer".into(), json!(false));
    m.insert("idle_custom_buttons".into(), Value::Array(empty_buttons()));
    m.insert("bot_auto_start".into(), json!(false));
    // kiểu hiển thị cho web KHÔNG nằm trong danh sách Trang web
    m.insert("web_default_type".into(), json!("playing"));
    m
}

pub fn is_valid_type(t: &str) -> bool {
    t == "watching" || t == "listening" || t == "playing"
}

pub fn parse_hhmm(s: &str) -> Option<i64> {
    // '23:30' -> số phút trong ngày; sai định dạng -> None
    let t = s.trim();
    let parts: Vec<&str> = t.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let h = parts[0].trim().parse::<i64>().ok()?;
    let mi = parts[1].trim().parse::<i64>().ok()?;
    if h < 24 && mi < 60 {
        Some(h * 60 + mi)
    } else {
        None
    }
}

/// Config là wrapper quanh Map JSON để giữ 100% khoá (kể cả khoá lạ) như bản Python.
#[derive(Clone, Default)]
pub struct Config(pub Map<String, Value>);

impl Config {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }
    pub fn get_bool(&self, key: &str) -> bool {
        truthy(self.0.get(key))
    }
    pub fn get_bool_or(&self, key: &str, default: bool) -> bool {
        match self.0.get(key) {
            Some(Value::Bool(b)) => *b,
            _ => default,
        }
    }
    pub fn get_str(&self, key: &str) -> String {
        as_str(self.0.get(key))
    }
    pub fn get_f64(&self, key: &str) -> f64 {
        as_f64(self.0.get(key), 0.0)
    }
    pub fn get_i64(&self, key: &str) -> i64 {
        as_i64(self.0.get(key), 0)
    }
    pub fn set(&mut self, key: &str, v: Value) {
        self.0.insert(key.into(), v);
    }
    pub fn set_bool(&mut self, key: &str, v: bool) {
        self.0.insert(key.into(), json!(v));
    }
    pub fn set_str(&mut self, key: &str, v: &str) {
        self.0.insert(key.into(), json!(v));
    }
    pub fn sites(&self) -> Vec<Value> {
        match self.0.get("sites") {
            Some(Value::Array(a)) => a.clone(),
            _ => Vec::new(),
        }
    }
    pub fn set_sites(&mut self, sites: Vec<Value>) {
        self.0.insert("sites".into(), Value::Array(sites));
    }
    pub fn blacklist(&self) -> Vec<String> {
        match self.0.get("blacklist_keywords") {
            Some(Value::Array(a)) => a.iter().map(|v| as_str(Some(v))).collect(),
            _ => Vec::new(),
        }
    }
}

/// Gộp với mặc định và làm sạch. Trả về (cfg, dirty); dirty = cần ghi lại file.
pub fn normalize_config(raw: &Map<String, Value>) -> (Config, bool) {
    let mut dirty = false;
    let mut cfg = default_config();
    for (k, v) in raw {
        cfg.insert(k.clone(), v.clone());
    }

    // Nâng cấp từ bản cũ: custom_sites -> sites (gộp chung với danh sách mặc định)
    let sites_ok = matches!(cfg.get("sites"), Some(Value::Array(_)));
    if !sites_ok {
        let mut custom = Vec::new();
        if let Some(Value::Array(arr)) = raw.get("custom_sites") {
            for s in arr {
                if !s.is_object() {
                    continue;
                }
                let m = as_str(s.get("match"));
                if m.is_empty() {
                    continue;
                }
                let name = {
                    let n = as_str(s.get("name"));
                    if n.is_empty() {
                        m.clone()
                    } else {
                        n
                    }
                };
                let kind = {
                    let k = as_str(s.get("type")).to_lowercase();
                    if is_valid_type(&k) {
                        k
                    } else {
                        "watching".into()
                    }
                };
                custom.push(new_site(&m, &name, &kind, true, "", "", "", true, "", true, None, false));
            }
        }
        custom.extend(default_sites());
        cfg.insert("sites".into(), Value::Array(custom));
        dirty = true;
    }
    for k in LEGACY_KEYS {
        if cfg.remove(k).is_some() {
            dirty = true;
        }
    }

    let mut clean = Vec::new();
    if let Some(Value::Array(arr)) = cfg.get("sites") {
        for s in arr {
            if !s.is_object() || as_str(s.get("match")).is_empty() {
                continue;
            }
            let m = as_str(s.get("match"));
            let mut base = new_site(&m, "", "watching", true, "", "", "", true, "", true, None, false);
            // base.update({k: s[k] for k in base if k in s})
            let name = {
                let n = as_str(s.get("name"));
                if n.is_empty() {
                    m.clone()
                } else {
                    n
                }
            };
            base["name"] = json!(name);
            for key in [
                "enabled",
                "match",
                "name",
                "type",
                "state",
                "details",
                "large_image",
                "timer",
                "domain",
                "favicon",
                "buttons",
                "buttons_off",
            ] {
                if let Some(v) = s.get(key) {
                    base[key] = match key {
                        // giữ đúng giá trị gốc cho bool để config tương thích ngược
                        "enabled" | "timer" | "favicon" | "buttons_off" => v.clone(),
                        "type" => json!(v.as_str().unwrap_or("watching")),
                        "domain" => json!(v.as_str().unwrap_or("")),
                        _ => v.clone(),
                    };
                }
            }
            // ép kiểu các trường chuỗi (mục tiêu: giống base khi thiếu trường)
            for key in ["match", "name", "type", "state", "details", "large_image", "domain"] {
                base[key] = json!(as_str(Some(&base[key])));
            }
            if !is_valid_type(base["type"].as_str().unwrap_or("")) {
                base["type"] = json!("watching");
            }
            base["buttons"] = Value::Array(norm_buttons(Some(&base["buttons"])));
            clean.push(base);
        }
    }
    cfg.insert("sites".into(), Value::Array(clean));

    cfg.insert("buttons".into(), Value::Array(norm_buttons(cfg.get("buttons"))));
    match cfg.get("favicon_mode").and_then(|v| v.as_str()) {
        Some("off") | Some("small") | Some("large") => {}
        _ => {
            cfg.insert("favicon_mode".into(), json!("small"));
        }
    }
    match cfg.get("theme").and_then(|v| v.as_str()) {
        Some("dark") | Some("light") => {}
        _ => {
            cfg.insert("theme".into(), json!("dark"));
        }
    }
    for (key, lo, hi) in [("bg_dim", 0i64, 85i64), ("bg_blur", 0, 20)] {
        let cur = cfg.get(key).cloned();
        let val = match cur {
            Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
            Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
            _ => None,
        };
        match val {
            Some(v) => {
                cfg.insert(key.into(), json!(v.max(lo).min(hi)));
            }
            None => {
                cfg.insert(key.into(), default_config()[key].clone());
            }
        }
    }
    for key in ["quiet_from", "quiet_to"] {
        if parse_hhmm(&as_str(cfg.get(key))).is_none() {
            cfg.insert(key.into(), default_config()[key].clone());
        }
    }
    {
        let ws = as_str(cfg.get("window_size"));
        if !regex_cache("^\\d{3,4}x\\d{3,4}$").is_match(&ws) {
            cfg.insert("window_size".into(), json!(""));
        }
    }
    {
        let pos = as_str(cfg.get("mini_pos"));
        if !regex_cache("^\\+-?\\d{1,5}\\+-?\\d{1,5}$").is_match(&pos) {
            cfg.insert("mini_pos".into(), json!(""));
        }
    }
    cfg.insert("toast_enabled".into(), json!(truthy(cfg.get("toast_enabled"))));
    if !matches!(cfg.get("blacklist_keywords"), Some(Value::Array(_))) {
        cfg.insert("blacklist_keywords".into(), json!([]));
    }
    cfg.insert("idle_custom_enabled".into(), json!(truthy(cfg.get("idle_custom_enabled"))));
    {
        let cid = as_str(cfg.get("idle_custom_client_id")).trim().to_string();
        cfg.insert("idle_custom_client_id".into(), json!(cid));
    }
    if !is_valid_type(&as_str(cfg.get("idle_custom_type"))) {
        cfg.insert("idle_custom_type".into(), json!("listening"));
    }
    for key in ["idle_custom_details", "idle_custom_state", "idle_custom_large_image", "idle_custom_large_text"] {
        cfg.insert(key.into(), json!(as_str(cfg.get(key))));
    }
    cfg.insert("idle_custom_timer".into(), json!(truthy(cfg.get("idle_custom_timer"))));
    cfg.insert(
        "idle_custom_buttons".into(),
        Value::Array(norm_buttons(cfg.get("idle_custom_buttons"))),
    );
    cfg.insert("bot_auto_start".into(), json!(truthy(cfg.get("bot_auto_start"))));
    if !is_valid_type(&as_str(cfg.get("web_default_type"))) {
        cfg.insert("web_default_type".into(), json!("playing"));
    }
    (Config(cfg), dirty)
}

pub fn load_config() -> Config {
    let mut raw: Map<String, Value> = Map::new();
    let mut dirty = false;
    let path = config_path();
    if path.exists() {
        match fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
            Some(Value::Object(m)) => raw = m,
            _ => {
                // hỏng -> chuyển sang .json.bak
                let bak = path.with_extension("json.bak");
                let _ = fs::rename(&path, &bak);
                dirty = true;
            }
        }
    } else {
        dirty = true;
    }
    let (cfg, d2) = normalize_config(&raw);
    let cfg = cfg;
    if dirty || d2 {
        save_config(&cfg);
    }
    cfg
}

pub fn save_config(cfg: &Config) {
    let text = serde_json::to_string_pretty(&cfg.0).unwrap_or_else(|_| "{}".to_string());
    let _ = fs::write(config_path(), text);
}
