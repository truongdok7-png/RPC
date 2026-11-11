// activity.rs - logic tạo activity (port build_activity / _build_activity và các hàm phụ của main.py).

use chrono::Timelike;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use once_cell::sync::Lazy;
use serde_json::{json, Map, Value};

use crate::browser;
use crate::config::Config;
use crate::icons::IconCache;
use crate::util::{as_str, clip, common_vals, fill, fill_url, norm, regex_cache, type_code};

// ---------------------------------------------------------------- trạng thái toàn cục

pub static FORCE_IDLE: AtomicBool = AtomicBool::new(false); // nút "Chuyển sang rảnh", không lưu vào config
static IDLE_SINCE: Lazy<Mutex<f64>> = Lazy::new(|| Mutex::new(0.0)); // thời điểm bắt đầu rảnh

// đồng hồ cộng dồn theo từng web (chỉ reset khi đóng hẳn trình duyệt)
struct SiteClock {
    acc: HashMap<String, f64>,
    cur: Option<(String, f64, f64)>, // (khoá đang xem, mốc bắt đầu, lần cuối còn thấy)
    seen: bool,
}

static SITE_CLOCK: Lazy<Mutex<SiteClock>> = Lazy::new(|| {
    Mutex::new(SiteClock {
        acc: HashMap::new(),
        cur: None,
        seen: false,
    })
});

fn site_close(end: Option<f64>) {
    let mut st = SITE_CLOCK.lock().unwrap();
    if let Some((key, since, _)) = st.cur.clone() {
        let e = end.unwrap_or_else(crate::util::now);
        *st.acc.entry(key).or_insert(0.0) += (e - since).max(0.0);
    }
    st.cur = None;
}

/// Mốc 'start' để Discord hiện đúng tổng thời gian đã xem web này (cộng dồn qua các lần quay lại).
pub fn site_start(key: &str) -> i64 {
    let now = crate::util::now();
    let mut st = SITE_CLOCK.lock().unwrap();
    if let Some((ckey, _since, last)) = st.cur.clone() {
        if ckey == key && now - last > 60.0 {
            // app bị tạm dừng / máy ngủ lâu -> không tính khoảng trống
            let e = last;
            if let Some((k, since, _)) = st.cur.clone() {
                *st.acc.entry(k).or_insert(0.0) += (e - since).max(0.0);
            }
            st.cur = None;
        }
    }
    let same = st.cur.as_ref().map(|(k, _, _)| k == key).unwrap_or(false);
    if !same {
        if st.cur.is_some() {
            if let Some((k, since, _)) = st.cur.clone() {
                *st.acc.entry(k).or_insert(0.0) += (now - since).max(0.0);
            }
        }
        st.cur = Some((key.to_string(), now, now));
    }
    if let Some((_, _, last)) = st.cur.as_mut() {
        *last = now;
    }
    st.seen = true;
    let acc = st.acc.get(key).copied().unwrap_or(0.0);
    let start = st.cur.as_ref().map(|(_, since, _)| *since - acc).unwrap_or(now);
    start as i64
}

fn site_tick_begin() {
    let mut st = SITE_CLOCK.lock().unwrap();
    st.seen = false;
    let has = !st.acc.is_empty() || st.cur.is_some();
    drop(st);
    if has && !crate::win::any_browser_window() {
        // đóng hẳn trình duyệt -> reset toàn bộ đồng hồ
        let mut st = SITE_CLOCK.lock().unwrap();
        st.acc.clear();
        st.cur = None;
    }
}

fn site_tick_end() {
    let mut st = SITE_CLOCK.lock().unwrap();
    if !st.seen {
        // tick này không ở trang nào có đồng hồ -> ngừng cộng dồn
        if let Some((key, since, _)) = st.cur.clone() {
            *st.acc.entry(key).or_insert(0.0) += (crate::util::now() - since).max(0.0);
        }
        st.cur = None;
    }
}

// ---------------------------------------------------------------- giờ yên lặng

pub fn in_quiet_hours(cfg: &Config) -> bool {
    if !cfg.get_bool("quiet_enabled") {
        return false;
    }
    let a = crate::config::parse_hhmm(&cfg.get_str("quiet_from"));
    let b = crate::config::parse_hhmm(&cfg.get_str("quiet_to"));
    let (a, b) = match (a, b) {
        (Some(a), Some(b)) => (a, b),
        _ => return false,
    };
    if a == b {
        return false;
    }
    let now = chrono::Local::now();
    let cur = now.hour() as i64 * 60 + now.minute() as i64;
    if a < b {
        a <= cur && cur < b
    } else {
        cur >= a || cur < b
    }
}

// ---------------------------------------------------------------- site / chặn từ khoá

/// Trả về mục site khớp đầu tiên (kể cả site đang tắt), hoặc None.
pub fn find_site(cfg: &Config, texts: &[&str]) -> Option<Value> {
    let blob = texts.join(" ").to_lowercase();
    for s in cfg.sites() {
        let m = as_str(s.get("match")).to_lowercase();
        if !m.is_empty() && blob.contains(&m) {
            return Some(s);
        }
    }
    None
}

pub fn blocked(cfg: &Config, texts: &[&str]) -> bool {
    let blob = texts.join(" ").to_lowercase();
    cfg.blacklist()
        .iter()
        .filter(|k| !k.trim().is_empty())
        .any(|k| blob.contains(&k.to_lowercase()))
}

// ---------------------------------------------------------------- favicon & nút

pub fn favicon_url(template: &str, domain: &str) -> String {
    (if template.is_empty() { crate::config::DEFAULT_FAVICON_TPL } else { template })
        .replace("{domain}", domain)
        .replace("{size}", "128")
}

fn favicon_fields(cfg: &Config, domain: &str) -> Map<String, Value> {
    let mut out = Map::new();
    let mode = cfg.get_str("favicon_mode");
    if mode == "off" || domain.is_empty() {
        return out;
    }
    let url = favicon_url(&cfg.get_str("favicon_url_template"), domain);
    let lower = url.to_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) || url.len() > 256 {
        return out;
    }
    if IconCache::global().fetch(&url).is_none() {
        // link chết -> không gửi, tránh Discord hiện ảnh lỗi
        return out;
    }
    if mode == "large" {
        out.insert("large_image".into(), json!(url));
    } else {
        out.insert("small_image".into(), json!(url));
        out.insert("small_text".into(), json!(domain));
    }
    out
}

/// Tạo tối đa 2 nút hợp lệ. Nút riêng (site / thủ công) ưu tiên hơn nút chung.
pub fn make_buttons(cfg: &Config, own: Option<&Value>, vals: &Map<String, Value>) -> Option<Value> {
    if !cfg.get_bool_or("buttons_enabled", true) {
        return None;
    }
    let filled = |v: Option<&Value>| -> Vec<Value> {
        match v {
            Some(Value::Array(arr)) => arr
                .iter()
                .filter(|b| !as_str(b.get("label")).trim().is_empty() && !as_str(b.get("url")).trim().is_empty())
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    };
    let own_filled = filled(own);
    let src = if !own_filled.is_empty() {
        own_filled
    } else {
        filled(cfg.get("buttons"))
    };
    // giá trị trống -> placeholder không điền được -> bỏ nút
    let uvals: Map<String, Value> = vals
        .iter()
        .filter(|(_, v)| !as_str(Some(v)).is_empty())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let mut out = Vec::new();
    for b in src.into_iter().take(2) {
        let label: String = fill(&as_str(b.get("label")), vals).trim().chars().take(32).collect();
        let url = fill_url(&as_str(b.get("url")).trim(), &uvals).trim().to_string();
        let placeholder = regex_cache("\\{\\w+\\}").is_match(&url);
        let lower = url.to_lowercase();
        if label.is_empty() || url.len() > 512 || placeholder || !(lower.starts_with("http://") || lower.starts_with("https://")) {
            continue;
        }
        out.push(json!({"label": label, "url": url}));
    }
    if out.is_empty() {
        None
    } else {
        Some(Value::Array(out))
    }
}

/// Phần ảnh favicon + nút bấm gắn thêm vào activity.
fn visuals(
    cfg: &Config,
    domain: &str,
    vals: &Map<String, Value>,
    own_buttons: Option<&Value>,
    favicon: bool,
    no_buttons: bool,
) -> Map<String, Value> {
    let mut out = Map::new();
    if favicon {
        out.extend(favicon_fields(cfg, domain));
    }
    if !no_buttons {
        if let Some(b) = make_buttons(cfg, own_buttons, vals) {
            out.insert("buttons".into(), b);
        }
    }
    out
}

fn site_kwargs(site: Option<&Value>) -> (Option<Value>, bool, bool) {
    match site {
        Some(s) => (s.get("buttons").cloned(), crate::util::truthy(s.get("favicon").or(Some(&json!(true)))), crate::util::truthy(s.get("buttons_off"))),
        None => (None, true, false),
    }
}

// ---------------------------------------------------------------- media khớp tab

fn same_media(tab_title: &str, media_title: &str) -> bool {
    let (a, b) = (norm(tab_title), norm(media_title));
    !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
}

// ---------------------------------------------------------------- TikTok

#[derive(Clone)]
pub struct TtInfo {
    pub caption: String,
    pub author: String,
    pub thumb: Option<String>,
}

static TT_CACHE: Lazy<Mutex<HashMap<String, (Option<TtInfo>, f64)>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Lấy caption / tác giả / thumbnail của video TikTok qua oEmbed (cần URL dạng .../@user/video/ID).
pub fn tiktok_info(domain: &str, url: Option<&str>) -> Option<TtInfo> {
    let url = url?;
    if !domain.ends_with("tiktok.com") || !regex_cache("/(?:video|photo)/\\d+").is_match(url) {
        return None;
    }
    let clean = url.split('?').next().unwrap_or("").split('#').next().unwrap_or("").to_string();
    let now = crate::util::now();
    {
        let cache = TT_CACHE.lock().unwrap();
        if let Some((hit, at)) = cache.get(&clean) {
            if hit.is_some() || now - *at < 300.0 {
                return hit.as_ref().map(|t| TtInfo {
                    caption: t.caption.clone(),
                    author: t.author.clone(),
                    thumb: t.thumb.clone(),
                });
            }
        }
    }
    let info = (|| -> Option<TtInfo> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(6))
            .timeout_read(Duration::from_secs(6))
            .build();
        let api = "https://www.tiktok.com/oembed?url=".to_string() + &crate::util::quote_like(&clean);
        let resp = agent
            .get(&api)
            .set("User-Agent", "Mozilla/5.0 DiscordPresence")
            .call()
            .ok()?;
        let data: Value = resp.into_json().ok()?;
        let thumb = as_str(data.get("thumbnail_url"));
        Some(TtInfo {
            caption: as_str(data.get("title"))
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            author: {
                let a = as_str(data.get("author_unique_id"));
                if a.is_empty() {
                    as_str(data.get("author_name"))
                } else {
                    a
                }
            },
            thumb: {
                let lower = thumb.to_lowercase();
                if lower.starts_with("http") && thumb.len() <= 256 {
                    Some(thumb)
                } else {
                    None
                }
            },
        })
    })();
    {
        let mut cache = TT_CACHE.lock().unwrap();
        if cache.len() > 100 {
            cache.clear();
        }
        cache.insert(clean, (info.clone(), now));
    }
    info
}

/// Ghi caption TikTok lên dòng 1, @tác giả lên dòng 2 (trừ khi trang đã tự chỉnh dòng đó).
fn apply_tiktok(act: &mut Map<String, Value>, site: Option<&Value>, info: Option<&TtInfo>) {
    let info = match info {
        Some(i) => i,
        None => return,
    };
    let site_details = site.map(|s| as_str(s.get("details"))).unwrap_or_default();
    let site_state = site.map(|s| as_str(s.get("state"))).unwrap_or_default();
    if !info.caption.is_empty() && site_details.is_empty() {
        act.insert("details".into(), json!(clip(&info.caption)));
    }
    if !info.author.is_empty() && site_state.is_empty() {
        let author = info.author.trim_start_matches('@');
        act.insert("state".into(), json!(clip(&format!("@{}", author))));
    }
}

// ---------------------------------------------------------------- tập phim

fn parse_episode(title: &str) -> String {
    let t = title;
    if let Some(m) = regex_cache("(?i)\\bS(?:eason)?\\s*(\\d{1,2})\\s*[,.\u{2013}\\-:|]?\\s*E(?:p(?:isode)?)?\\s*\\.?\\s*(\\d{1,4})\\b").captures(t) {
        let s: i64 = m.get(1).unwrap().as_str().parse().unwrap_or(0);
        let e: i64 = m.get(2).unwrap().as_str().parse().unwrap_or(0);
        return format!("Mùa {} · Tập {}", s, e);
    }
    let m = match regex_cache("(?i)\\b(?:T\u{1ead}p|Tap|Episode|Ep)\\.?\\s*#?(\\d{1,4})\\b").captures(t) {
        Some(m) => m,
        None => return String::new(),
    };
    let e: i64 = m.get(1).unwrap().as_str().parse().unwrap_or(0);
    let season = regex_cache("(?i)\\b(?:M\u{00f9}a|Season|Ph\u{1ea7}n)\\s*(\\d{1,2})\\b")
        .captures(t)
        .map(|s| format!("Mùa {} · ", s.get(1).unwrap().as_str().parse::<i64>().unwrap_or(0)))
        .unwrap_or_default();
    format!("{}Tập {}", season, e)
}

/// Đưa số tập / mùa vào dòng 2 (trừ khi trang đã tự chỉnh dòng 2, hoặc là trang nghe nhạc).
fn apply_episode(act: &mut Map<String, Value>, site: Option<&Value>, title: &str) {
    if let Some(s) = site {
        if !as_str(s.get("state")).is_empty() || as_str(s.get("type")) == "listening" {
            return;
        }
    }
    let ep = parse_episode(title);
    if !ep.is_empty() {
        act.insert("state".into(), json!(clip(&ep)));
    }
}

// ---------------------------------------------------------------- YouTube thumbnail

pub fn get_yt_thumb(url: Option<&str>) -> Option<String> {
    let url = url?;
    let m = regex_cache("(?:[?&]v=|youtu\\.be/|shorts/|/live/)([a-zA-Z0-9_-]{11})").captures(url)?;
    Some(format!("https://img.youtube.com/vi/{}/mqdefault.jpg", m.get(1).unwrap().as_str()))
}

/// Gắn favicon + nút bấm vào activity.
/// Đang xem video YouTube: thumbnail video làm ảnh lớn; favicon (nếu có) chuyển thành biểu tượng nhỏ.
fn apply_visuals(
    act: &mut Map<String, Value>,
    cfg: &Config,
    domain: &str,
    vals: &Map<String, Value>,
    url: Option<&str>,
    thumb: Option<String>,
    own_buttons: Option<&Value>,
    favicon: bool,
    no_buttons: bool,
) {
    let mut vis = visuals(cfg, domain, vals, own_buttons, favicon, no_buttons);
    let thumb = thumb.or_else(|| {
        if domain.ends_with("youtube.com") {
            get_yt_thumb(url)
        } else {
            None
        }
    });
    match thumb {
        Some(t) if IconCache::global().fetch(&t).is_some() => {
            let fav_large = vis.remove("large_image");
            if let Some(fl) = fav_large {
                if !vis.contains_key("small_image") {
                    vis.insert("small_image".into(), fl);
                    vis.insert("small_text".into(), json!(domain));
                }
            }
            act.extend(vis);
            act.insert("large_image".into(), json!(t));
        }
        _ => {
            act.extend(vis);
        }
    }
}

// ---------------------------------------------------------------- rảnh tuỳ chỉnh

pub struct CustomIdle {
    pub client_id: String,
    pub kind: String,
    pub details: String,
    pub state: String,
    pub large_image: String,
    pub large_text: String,
    pub timer: bool,
    pub buttons: Value,
}

/// Cấu hình rảnh tuỳ chỉnh nếu đang bật và có Client ID hợp lệ; ngược lại None (dùng rảnh mặc định).
pub fn custom_idle(cfg: &Config) -> Option<CustomIdle> {
    if !cfg.get_bool("idle_custom_enabled") {
        return None;
    }
    let cid = cfg.get_str("idle_custom_client_id").trim().to_string();
    if cid.is_empty() || !cid.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let kind = {
        let k = cfg.get_str("idle_custom_type");
        if crate::config::is_valid_type(&k) {
            k
        } else {
            "listening".into()
        }
    };
    Some(CustomIdle {
        client_id: cid,
        kind,
        details: cfg.get_str("idle_custom_details"),
        state: cfg.get_str("idle_custom_state"),
        large_image: cfg.get_str("idle_custom_large_image").trim().to_string(),
        large_text: cfg.get_str("idle_custom_large_text"),
        timer: cfg.get_bool("idle_custom_timer"),
        buttons: cfg.get("idle_custom_buttons").cloned().unwrap_or(Value::Null),
    })
}

// ---------------------------------------------------------------- build_activity

/// Trả về dict activity hoặc None (hàm bọc ngoài, port build_activity()).
pub fn build_activity(cfg: &Config) -> Option<Value> {
    let force = FORCE_IDLE.load(Ordering::Relaxed);
    // bỏ qua media / web, chỉ hiện trạng thái rảnh
    let show_media = if force { false } else { cfg.get_bool("show_media") };
    let show_browsing = if force { false } else { cfg.get_bool("show_browsing") };
    let show_idle = if force { true } else { cfg.get_bool_or("show_idle", true) };

    site_tick_begin();
    let act = if in_quiet_hours(cfg) {
        None
    } else {
        _build_activity(cfg, show_media, show_browsing, show_idle)
    };
    site_tick_end();

    let mut act = act?;
    if !cfg.get_bool_or("show_large_text", true) {
        act.as_object_mut().unwrap().remove("large_text"); // ẩn dòng chữ dưới cùng
    }
    let idle = crate::util::truthy(act.get("_idle"));
    if idle {
        let mut since = IDLE_SINCE.lock().unwrap();
        if *since == 0.0 {
            *since = crate::util::now();
        }
        let timer = crate::util::truthy(act.get("_idle_timer"));
        if timer {
            let obj = act.as_object_mut().unwrap();
            obj.remove("_idle_timer");
            obj.insert("start".into(), json!(*since as i64));
        }
    } else {
        *IDLE_SINCE.lock().unwrap() = 0.0;
    }
    Some(act)
}

fn _build_activity(cfg: &Config, show_media: bool, show_browsing: bool, show_idle: bool) -> Option<Value> {
    let now = crate::util::now();
    let large = cfg.get_str("large_image");
    let use_url = cfg.get_bool_or("read_address_bar", true);

    // 1) Media đang phát
    if show_media {
        if let Some(media) = crate::media::read_media() {
            if !media.title.is_empty() {
                let from_browser = crate::win::BROWSERS
                    .iter()
                    .any(|(exe, _)| media.app.contains(exe.trim_end_matches(".exe")));
                let bt = if from_browser { crate::win::top_browser_title() } else { None };
                let mut site = find_site(cfg, &[&media.app, &media.title, &media.artist]);

                // Tab trên cùng có đúng là tab đang phát media không? (nếu không thì KHÔNG được lấy
                // site/favicon/URL từ tab đó, nếu không sẽ ra tiêu đề YouTube + favicon của trang khác)
                let mut url: Option<String> = None;
                let mut other_tab = false;
                if from_browser {
                    let key = (media.app.clone(), norm(&media.title));
                    let tab_is_media = bt.as_ref().map(|(btitle, _, _)| {
                        same_media(btitle, &media.title)
                            || site
                                .as_ref()
                                .map(|s| find_site(cfg, &[btitle]).map(|f| as_str(f.get("match")).to_lowercase())
                                    == Some(as_str(s.get("match")).to_lowercase()))
                                .unwrap_or(false)
                    }).unwrap_or(false);
                    other_tab = bt.is_some() && !tab_is_media; // đang ở tab khác -> bỏ qua media, hiện trạng thái duyệt web
                    if tab_is_media {
                        if site.is_none() {
                            if let Some((btitle, _, _)) = &bt {
                                site = find_site(cfg, &[btitle]);
                            }
                        }
                        if use_url {
                            if let Some((btitle, _, bhwnd)) = &bt {
                                url = browser::read_address_bar(*bhwnd, btitle);
                                if url.is_some() && site.is_none() {
                                    site = find_site(cfg, &[&browser::host_of(url.as_deref().unwrap_or(""))]);
                                }
                            }
                        }
                        let mut ctx = MEDIA_CTX.lock().unwrap();
                        if ctx.len() > 20 {
                            ctx.clear();
                        }
                        ctx.insert(key.clone(), (site.clone(), url.clone()));
                    } else if let Some((c_site, c_url)) = MEDIA_CTX.lock().unwrap().get(&key).cloned() {
                        // đang ở tab khác: dùng lại thông tin đã biết của tab phát media (vd. YouTube)
                        site = site.or(c_site);
                        url = c_url;
                    }
                }
                if !other_tab {
                    if let Some(s) = &site {
                        if !crate::util::truthy(s.get("enabled")) {
                            return None; // site này đã bị tắt -> không hiển thị gì
                        }
                    }
                    // "artist" của video trên web đôi khi chỉ là tên miền CDN (stream.xxx.com) -> bỏ
                    let artist_strip = media.artist.trim().to_string();
                    let artist = if regex_cache("^[\\w-]+(\\.[\\w-]+)+$").is_match(&artist_strip) {
                        String::new()
                    } else {
                        media.artist.clone()
                    };
                    if !blocked(cfg, &[&media.title, &media.artist, url.as_deref().unwrap_or("")]) {
                        let domain = browser::resolve_domain(site.as_ref(), url.as_deref());
                        let site_name = site.as_ref().map(|s| as_str(s.get("name"))).unwrap_or_else(|| "Media".into());
                        let vals = common_vals(&media.title, &artist, &site_name, "", url.as_deref().unwrap_or(""), &domain);
                        let kind = if let Some(s) = &site { type_code(&as_str(s.get("type"))) } else { 1 };
                        let details = match &site {
                            Some(s) if !as_str(s.get("details")).is_empty() => fill(&as_str(s.get("details")), &vals),
                            _ => media.title.clone(),
                        };
                        let state = match &site {
                            Some(s) if !as_str(s.get("state")).is_empty() => {
                                if as_str(s.get("state")).trim() == "-" {
                                    String::new()
                                } else {
                                    fill(&as_str(s.get("state")), &vals)
                                }
                            }
                            _ => {
                                if !artist.is_empty() {
                                    artist.clone()
                                } else if let Some(s) = &site {
                                    as_str(s.get("name"))
                                } else {
                                    "Đang phát".to_string()
                                }
                            }
                        };

                        let mut act = Map::new();
                        act.insert("details".into(), json!(clip(&details)));
                        act.insert("state".into(), json!(clip(&state)));
                        let li = site
                            .as_ref()
                            .map(|s| as_str(s.get("large_image")))
                            .unwrap_or_default();
                        act.insert("large_image".into(), json!(if li.is_empty() { large.clone() } else { li }));
                        act.insert("large_text".into(), json!(site_name));
                        act.insert("activity_type".into(), json!(kind));

                        let timer = site
                            .as_ref()
                            .map(|s| crate::util::truthy(s.get("timer")))
                            .unwrap_or(true);
                        if timer {
                            act.insert("start".into(), json!((now - media.pos) as i64));
                            if media.dur > 0.0 {
                                act.insert("end".into(), json!((now - media.pos + media.dur) as i64));
                            }
                        }
                        let src = site
                            .as_ref()
                            .map(|s| as_str(s.get("name")))
                            .unwrap_or_else(|| if domain.is_empty() { "Media".into() } else { domain.clone() });
                        act.insert("_src".into(), json!(src));
                        let mut act = Value::Object(act);
                        let (own_buttons, favicon, no_buttons) = site_kwargs(site.as_ref());
                        apply_episode(act.as_object_mut().unwrap(), site.as_ref(), &media.title);
                        let tt = tiktok_info(&domain, url.as_deref());
                        apply_tiktok(act.as_object_mut().unwrap(), site.as_ref(), tt.as_ref());
                        apply_visuals(act.as_object_mut().unwrap(), cfg, &domain, &vals, url.as_deref(), tt.as_ref().and_then(|t| t.thumb.clone()), own_buttons.as_ref(), favicon, no_buttons);
                        return Some(act);
                    }
                }
            }
        }
    }

    // 2) Đang duyệt web
    if show_browsing {
        let idle_limit = cfg.get_f64("idle_minutes") * 60.0;
        if idle_limit > 0.0 && crate::win::idle_seconds() > idle_limit {
            return None; // vắng mặt quá lâu -> tạm ẩn
        }
        let fg = if cfg.get_bool("browsing_requires_focus") {
            crate::win::foreground_browser_title()
        } else {
            crate::win::top_browser_title()
        };
        if let Some((title, browser_name, hwnd)) = fg {
            if !title.is_empty() {
                let url = if use_url { browser::read_address_bar(hwnd, &title) } else { None };
                if !blocked(cfg, &[&title, url.as_deref().unwrap_or("")]) {
                    let site = match &url {
                        Some(u) => find_site(cfg, &[&title, &browser::host_of(u)]),
                        None => find_site(cfg, &[&title]),
                    };
                    if let Some(s) = &site {
                        if !crate::util::truthy(s.get("enabled")) {
                            return None;
                        }
                    }
                    let domain = browser::resolve_domain(site.as_ref(), url.as_deref());
                    let site_name = site.as_ref().map(|s| as_str(s.get("name"))).unwrap_or_else(|| browser_name.clone());
                    let vals = common_vals(&title, "", &site_name, &browser_name, url.as_deref().unwrap_or(""), &domain);
                    let kind = if let Some(s) = &site {
                        type_code(&as_str(s.get("type")))
                    } else {
                        type_code(&cfg.get_str("web_default_type"))
                    };
                    let details = match &site {
                        Some(s) if !as_str(s.get("details")).is_empty() => fill(&as_str(s.get("details")), &vals),
                        _ => title.clone(),
                    };
                    let state = match &site {
                        Some(s) if !as_str(s.get("state")).is_empty() => {
                            if as_str(s.get("state")).trim() == "-" {
                                String::new()
                            } else {
                                fill(&as_str(s.get("state")), &vals)
                            }
                        }
                        _ => {
                            let st = fill(&cfg.get_str("browsing_state"), &vals).trim().to_string();
                            if st == "-" {
                                String::new()
                            } else {
                                st
                            }
                        }
                    };

                    let mut act = Map::new();
                    act.insert("details".into(), json!(clip(&details)));
                    act.insert("state".into(), json!(clip(&state)));
                    let li = site.as_ref().map(|s| as_str(s.get("large_image"))).unwrap_or_default();
                    act.insert("large_image".into(), json!(if li.is_empty() { large.clone() } else { li }));
                    act.insert("large_text".into(), json!(site_name));
                    act.insert("activity_type".into(), json!(kind));

                    let timer = site.as_ref().map(|s| crate::util::truthy(s.get("timer"))).unwrap_or(true);
                    if timer {
                        let key = site
                            .as_ref()
                            .map(|s| as_str(s.get("match")).to_lowercase())
                            .unwrap_or_else(|| if domain.is_empty() { browser_name.clone() } else { domain.clone() });
                        act.insert("start".into(), json!(site_start(&key)));
                    }
                    let src = site
                        .as_ref()
                        .map(|s| as_str(s.get("name")))
                        .unwrap_or_else(|| if domain.is_empty() { browser_name.clone() } else { domain.clone() });
                    act.insert("_src".into(), json!(src));
                    let mut act = Value::Object(act);
                    let (own_buttons, favicon, no_buttons) = site_kwargs(site.as_ref());
                    apply_episode(act.as_object_mut().unwrap(), site.as_ref(), &title);
                    let tt = tiktok_info(&domain, url.as_deref());
                    apply_tiktok(act.as_object_mut().unwrap(), site.as_ref(), tt.as_ref());
                    apply_visuals(act.as_object_mut().unwrap(), cfg, &domain, &vals, url.as_deref(), tt.as_ref().and_then(|t| t.thumb.clone()), own_buttons.as_ref(), favicon, no_buttons);
                    return Some(act);
                }
            }
        }
    }

    // --- CHỨC NĂNG ĐANG RẢNH ---
    if show_idle {
        if let Some(ci) = custom_idle(cfg) {
            // rảnh tuỳ chỉnh: hiện bằng Client ID khác (Engine tự đổi kết nối theo "_cid")
            let mut act = Map::new();
            act.insert("details".into(), json!(clip(&ci.details)));
            act.insert("state".into(), json!(clip(&ci.state)));
            // ảnh phải có trong app của Client ID đó, nên không dùng ảnh mặc định
            act.insert("large_image".into(), json!(ci.large_image.clone()));
            act.insert("large_text".into(), json!(ci.large_text.clone()));
            act.insert("activity_type".into(), json!(type_code(&ci.kind)));
            act.insert("_cid".into(), json!(ci.client_id.clone()));
            act.insert("_idle".into(), json!(true));
            act.insert("_idle_timer".into(), json!(ci.timer));
            act.insert("_src".into(), json!("Đang rảnh"));
            if let Some(b) = make_buttons(cfg, Some(&ci.buttons), &common_vals("", "", "", "", "", "")) {
                act.insert("buttons".into(), b);
            }
            return Some(Value::Object(act));
        }
        let mut act = Map::new();
        act.insert("details".into(), json!(clip(&cfg.get_str("idle_details"))));
        act.insert("state".into(), json!(">.<"));
        act.insert("large_image".into(), json!(large.clone()));
        act.insert("large_text".into(), json!(""));
        act.insert("activity_type".into(), json!(1));
        act.insert("_src".into(), json!("Đang rảnh"));
        act.insert("_idle".into(), json!(true));
        // Thêm nút bấm chung vào trạng thái rảnh
        let vals = common_vals("", "", "", "", "", "");
        act.extend(visuals(cfg, "", &vals, None, false, false));
        return Some(Value::Object(act));
    }
    // --------------------------------------
    None
}

static MEDIA_CTX: Lazy<Mutex<HashMap<(String, String), (Option<Value>, Option<String>)>>> = Lazy::new(|| Mutex::new(HashMap::new()));
