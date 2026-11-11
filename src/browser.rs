// browser.rs - đọc địa chỉ trang từ thanh địa chỉ trình duyệt (UI Automation) + hàm URL/tên miền.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use once_cell::sync::Lazy;
use serde_json::Value;

use crate::util::regex_cache;

// ---------------------------------------------------------------- regex dùng cho URL

pub fn private_re_test(s: &str) -> bool {
    regex_cache("(?i)inprivate|incognito|private browsing|ẩn danh").is_match(s)
}

fn domain_ok(host: &str) -> bool {
    regex_cache("^[\\w-]+(\\.[\\w-]+)+$").is_match(host)
}

fn scheme_re_test(val: &str) -> bool {
    regex_cache("(?i)^[a-z][a-z0-9+.\\-]*://").is_match(val)
}

/// Chuẩn hoá chuỗi trong thanh địa chỉ thành URL http(s); trả về None nếu không phải URL.
pub fn normalize_url(val: &str) -> Option<String> {
    let mut val = val.trim().to_string();
    if val.is_empty() || val.split_whitespace().count() > 1 {
        return None;
    }
    if !scheme_re_test(&val) {
        val = format!("https://{}", val); // Chrome/Edge ẩn "https://" trong thanh địa chỉ
    }
    let host = host_of(&val);
    // lấy scheme
    let scheme_end = val.find("://")?;
    let scheme = val[..scheme_end].to_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    if host.is_empty() || !domain_ok(&host) {
        return None;
    }
    Some(val)
}

/// hostname của URL, bỏ "www." ở đầu (giống urlparse(url).hostname).
pub fn host_of(url: &str) -> String {
    let u = url.trim();
    let rest = match u.find("://") {
        Some(i) => &u[i + 3..],
        None => return String::new(),
    };
    // cắt phần sau dấu / ? #
    let end = rest
        .find(|c| c == '/' || c == '?' || c == '#')
        .unwrap_or(rest.len());
    let mut host = &rest[..end];
    // bỏ userinfo (user:pass@)
    if let Some(at) = host.rfind('@') {
        host = &host[at + 1..];
    }
    // bỏ cổng
    let mut h = host.split(':').next().unwrap_or("").to_lowercase();
    if let Some(stripped) = h.strip_prefix("www.") {
        h = stripped.to_string();
    }
    h
}

pub fn clean_domain(d: &str) -> String {
    let d = d.trim().to_lowercase();
    let d = regex_cache("(?i)^[a-z][a-z0-9+.\\-]*://").replace(&d, "").to_string();
    let d = d.split('/').next().unwrap_or("").split(':').next().unwrap_or("").to_string();
    if let Some(stripped) = d.strip_prefix("www.") {
        stripped.to_string()
    } else {
        d
    }
}

pub fn known_domain_for(m: &str) -> String {
    KNOWN_DOMAINS
        .iter()
        .find(|(k, _)| *k == m.trim().to_lowercase())
        .map(|(_, v)| v.to_string())
        .unwrap_or_default()
}

pub const KNOWN_DOMAINS: [(&str, &str); 9] = [
    ("youtube", "youtube.com"),
    ("netflix", "netflix.com"),
    ("twitch", "twitch.tv"),
    ("tiktok", "tiktok.com"),
    ("bilibili", "bilibili.com"),
    ("spotify", "spotify.com"),
    ("soundcloud", "soundcloud.com"),
    ("zing mp3", "zingmp3.vn"),
    ("nhaccuatui", "nhaccuatui.com"),
];

/// Tên miền suy ra từ cấu hình của site (ô 'Tên miền' > từ khoá dạng tên miền > danh sách có sẵn).
pub fn guess_domain(site: Option<&Value>) -> String {
    let site = match site {
        Some(s) => s,
        None => return String::new(),
    };
    let d = clean_domain(&site_str(site, "domain"));
    if !d.is_empty() {
        return d;
    }
    let m = clean_domain(&site_str(site, "match"));
    if domain_ok(&m) {
        return m;
    }
    known_domain_for(&m)
}

pub fn site_str(site: &Value, key: &str) -> String {
    crate::util::as_str(site.get(key))
}

/// Ưu tiên: tên miền khai báo cho site > tên miền của URL đọc được > đoán từ cấu hình.
pub fn resolve_domain(site: Option<&Value>, url: Option<&str>) -> String {
    if let Some(s) = site {
        let d = clean_domain(&site_str(s, "domain"));
        if !d.is_empty() {
            return d;
        }
    }
    if let Some(u) = url {
        return host_of(u);
    }
    guess_domain(site)
}

// ---------------------------------------------------------------- đọc thanh địa chỉ (UIA)

type CacheEntry = (Option<String>, f64);

static URL_CACHE: Lazy<Mutex<HashMap<(isize, String), CacheEntry>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Hàm này chạy trong luồng riêng để UI Automation không đụng COM của luồng engine (winrt).
fn uia_read(hwnd: isize, out: mpsc::Sender<String>) {
    use uiautomation::controls::ControlType;
    use uiautomation::patterns::UIValuePattern;
    use uiautomation::types::Handle;
    let result = (|| -> Result<String, uiautomation::Error> {
        let automation = uiautomation::UIAutomation::new()?;
        let win = automation.element_from_handle(Handle::from(hwnd))?;
        // ô Edit đầu tiên của cửa sổ = thanh địa chỉ
        let edit = automation
            .create_matcher()
            .from(win)
            .control_type(ControlType::Edit)
            .depth(16)
            .timeout(1500)
            .find_first()?;
        let vp: UIValuePattern = edit.get_pattern()?;
        Ok(vp.get_value()?)
    })();
    if let Ok(value) = result {
        let _ = out.send(value);
    }
}

/// Đọc URL từ thanh địa chỉ của cửa sổ trình duyệt. Có cache 20 giây, chờ tối đa 4 giây.
pub fn read_address_bar(hwnd: isize, title: &str) -> Option<String> {
    let now = crate::util::now();
    {
        let cache = URL_CACHE.lock().unwrap();
        if let Some((hit, at)) = cache.get(&(hwnd, title.to_string())) {
            if now - at < 20.0 {
                return hit.clone();
            }
        }
    }
    let (tx, rx) = mpsc::channel::<String>();
    let _t = std::thread::Builder::new()
        .name("uia-read".into())
        .spawn(move || uia_read(hwnd, tx));
    let raw = rx.recv_timeout(Duration::from_secs(4)).ok();
    let url = raw.as_deref().and_then(normalize_url);
    {
        let mut cache = URL_CACHE.lock().unwrap();
        if cache.len() > 40 {
            cache.clear();
        }
        cache.insert((hwnd, title.to_string()), (url.clone(), now));
    }
    url
}

pub fn clear_url_cache() {
    URL_CACHE.lock().unwrap().clear();
}
