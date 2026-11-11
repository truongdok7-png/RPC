// app.rs - cửa sổ chính (port class MainWindow của main.py) sang egui/eframe.

use crossbeam_channel::{unbounded, Receiver, Sender};
use egui::{
    Color32, Context, FontId, Id, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2, ViewportCommand, ViewportId,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::activity::{in_quiet_hours, FORCE_IDLE};
use crate::bot::{bot_api, BotRunner};
use crate::config::Config;
use crate::engine::Engine;
use crate::icons::IconCache;
use crate::pages::dialog::SiteDialog;
use crate::theme::{self, Palette, DOT_OFF, DOT_ON, DOT_WARN, GLASS, SIDE_MUTED, SIDE_TEXT, TALLY};
use crate::util::{kind_of_code, type_label, APP_VERSION, LOG_MAX};
use crate::win;

// ---------------------------------------------------------------- trang

// (khoá, tên trên thanh bên, mô tả dưới tiêu đề, khoảng cách phía trên)
pub const PAGES: [(&str, &str, &str, f32); 11] = [
    ("home", "Tổng quan", "Trạng thái đang hiện trên Discord và các công tắc nhanh", 0.0),
    ("sites", "Trang web", "Nguồn được nhận diện. Bấm đúp một dòng để sửa", 0.0),
    ("visual", "Favicon & nút", "Ảnh nhỏ theo từng trang và nút bấm dưới thẻ", 0.0),
    ("block", "Chặn từ khoá", "Tab hoặc bài hát chứa từ này sẽ không hiện lên Discord", 0.0),
    ("idle", "Trạng thái rảnh", "Rảnh mặc định, rảnh tuỳ chỉnh bằng Client ID khác, công tắc nhanh", 0.0),
    ("look", "Giao diện", "Ảnh nền và màu sắc của app", 12.0),
    ("settings", "Cài đặt", "Kết nối, nội dung hiển thị và hành vi của app", 0.0),
    ("bot", "Bot Nino", "Bật / tắt bot Discord và gửi nhanh tin nhắn", 12.0),
    ("botlog", "Log bot", "Nhật ký chạy của bot (thay cho trang dashboard cũ)", 0.0),
    ("log", "Hoạt động", "Những gì app vừa làm", 12.0),
    ("changelog", "Nhật ký update", "Lịch sử các lần cập nhật", 0.0),
];

// ---------------------------------------------------------------- thông điệp từ luồng nền

pub enum UiMsg {
    /// Kết quả gọi API của bot. clear: 0 = không xoá, 1 = xoá ô thông báo, 2 = xoá ô tin nhắn kênh.
    BotMsg { text: String, bad: bool, clear: u8 },
    /// Kết quả thử lấy favicon (bytes hoặc None).
    Favicon(Option<Vec<u8>>),
    /// Kết quả thử đọc thanh địa chỉ.
    Address(String),
    /// Danh sách kênh của bot: (danh sách (tên, id), thông báo, là lỗi).
    Channels(Vec<(String, String)>, String, bool),
}

// ---------------------------------------------------------------- khay hệ thống

/// Hàng đợi hành động từ khay hệ thống (0 mở, 1 thu nhỏ, 2 rảnh, 3 bot, 4 presence, 5 thoát).
static TRAY_Q: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Chạy trong handler của tray (có thể ở luồng/ngữ cảnh không có khung vẽ egui).
fn tray_dispatch(action: u8, ctx: &Context) {
    match action {
        0 => crate::win::show_own_main_window(), // hiện cửa sổ ngay bằng WinAPI
        5 => {
            // lưới an toàn: nếu egui không kịp đóng (cửa sổ đang ẩn) thì vẫn thoát hẳn
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(2500));
                std::process::exit(0);
            });
        }
        _ => {}
    }
    TRAY_Q.lock().unwrap().push(action);
    ctx.request_repaint();
}

pub struct TrayUi {
    pub icon: tray_icon::TrayIcon,
    pub mi_open: tray_icon::menu::MenuItem,
    pub mi_mini: tray_icon::menu::MenuItem,
    pub mi_idle: tray_icon::menu::MenuItem,
    pub mi_bot: tray_icon::menu::MenuItem,
    pub mi_presence: tray_icon::menu::MenuItem,
    pub mi_quit: tray_icon::menu::MenuItem,
}

// ---------------------------------------------------------------- App

pub struct App {
    pub cfg: Arc<Mutex<Config>>,
    pub engine: Engine,
    pub bot: BotRunner,
    pub ui_tx: Sender<UiMsg>,
    pub ui_rx: Receiver<UiMsg>,
    pub log_rx: Receiver<(String, String)>,
    pub logs: Vec<(String, String)>,
    pub botlogs: Vec<(String, String)>,
    pub cur: &'static str,
    pub flash: HashMap<String, (String, f64, bool)>,
    pub toast: Option<(String, f64)>,
    pub toast_key: Option<String>,
    pub toast_at: f64,
    // nền
    pub bg_src: Option<image::RgbImage>,
    pub bg_tex: Option<TextureHandle>,
    pub bg_key: String,
    pub bg_job: Option<Receiver<(String, image::RgbImage)>>, // luồng nền đang dựng ảnh nền
    pub bg_job_key: String,
    pub bg_thumb: Option<TextureHandle>,
    pub bg_thumb_key: String,
    pub bg_txt: String,
    pub logo_tex: Option<TextureHandle>,
    // thẻ xem trước (hero) - nội dung do tick() cập nhật
    pub pv_type: String,
    pub pv_details: String,
    pub pv_state: String,
    pub hero_icons: HashMap<String, TextureHandle>,
    pub hero_kind_tex: Option<(String, [u8; 3], TextureHandle)>,
    // trang
    pub sel_site: Option<usize>,
    pub dialog: Option<SiteDialog>,
    pub sel_kw: Option<usize>,
    // ô nhập (đổ từ config lúc mở app / nhập cấu hình)
    pub i_cid: String,
    pub i_interval: String,
    pub i_img: String,
    pub i_idle_min: String,
    pub i_bstate: String,
    pub i_qfrom: String,
    pub i_qto: String,
    pub c_hover: bool,
    pub c_focus: bool,
    pub c_auto: bool,
    pub c_tray: bool,
    pub c_win: bool,
    pub c_quiet: bool,
    pub c_media: bool,
    pub c_browse: bool,
    pub c_toast: bool,
    pub i_webtype: String,
    pub i_fmode: String,
    pub i_ftpl: String,
    pub c_addr: bool,
    pub c_btn_on: bool,
    pub gb_label: [String; 2],
    pub gb_url: [String; 2],
    pub s_idle_txt: String,
    pub c_show_idle: bool,
    pub c_force: bool,
    pub ci_on: bool,
    pub ci_cid: String,
    pub ci_type: String,
    pub ci_d: String,
    pub ci_s: String,
    pub ci_img: String,
    pub ci_lt: String,
    pub ci_timer: bool,
    pub ci_label: [String; 2],
    pub ci_url: [String; 2],
    pub i_theme: String,
    pub dim: f32,
    pub blur: f32,
    pub v_kw: String,
    pub bot_auto: bool,
    pub i_tdom: String,
    pub test_tex: Option<TextureHandle>,
    pub test_txt: String,
    // trang bot
    pub chan_names: Vec<String>,
    pub chan_ids: Vec<String>,
    pub chan_sel: usize,
    pub bot_ann: String,
    pub bot_chan: String,
    pub bot_msg: String,
    pub bot_msg_bad: bool,
    // mini
    pub mini_open: bool,
    pub mini_pos: (f32, f32),
    // khay + vòng đời
    pub tray: Option<TrayUi>,
    pub tray_tip: String,
    pub start_hidden: bool,
    pub shown_once: bool,
    pub quitting: bool,
    pub ctx: Context,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, cfg: Config, start_hidden: bool) -> Self {
        crate::diag::note("App::new: nạp font");
        theme::setup_fonts(cc);
        crate::diag::note("App::new: font xong");
        let cfg = Arc::new(Mutex::new(cfg));
        let (ui_tx, ui_rx) = unbounded::<UiMsg>();
        let (log_tx, log_rx) = unbounded::<(String, String)>();
        let engine = Engine::new(cfg.clone(), log_tx.clone());
        let bot = BotRunner::new(log_tx);
        let ctx = cc.egui_ctx.clone();
        let theme_name = cfg.lock().unwrap().get_str("theme");
        theme::apply_style(&ctx, &theme::palette(&theme_name));
        crate::diag::note("App::new: style xong");

        let mut app = Self {
            cfg,
            engine,
            bot,
            ui_tx,
            ui_rx,
            log_rx,
            logs: Vec::new(),
            botlogs: Vec::new(),
            cur: "home",
            flash: HashMap::new(),
            toast: None,
            toast_key: None,
            toast_at: 0.0,
            bg_src: None,
            bg_tex: None,
            bg_key: String::new(),
            bg_job: None,
            bg_job_key: String::new(),
            bg_thumb: None,
            bg_thumb_key: String::new(),
            bg_txt: String::new(),
            logo_tex: None,
            pv_type: "Chưa có gì".into(),
            pv_details: String::new(),
            pv_state: String::new(),
            hero_icons: HashMap::new(),
            hero_kind_tex: None,
            sel_site: None,
            dialog: None,
            sel_kw: None,
            i_cid: String::new(),
            i_interval: String::new(),
            i_img: String::new(),
            i_idle_min: String::new(),
            i_bstate: String::new(),
            i_qfrom: String::new(),
            i_qto: String::new(),
            c_hover: true,
            c_focus: false,
            c_auto: true,
            c_tray: true,
            c_win: false,
            c_quiet: false,
            c_media: true,
            c_browse: true,
            c_toast: true,
            i_webtype: "Đang chơi".into(),
            i_fmode: "small".into(),
            i_ftpl: crate::config::DEFAULT_FAVICON_TPL.into(),
            c_addr: true,
            c_btn_on: true,
            gb_label: [String::new(), String::new()],
            gb_url: [String::new(), String::new()],
            s_idle_txt: "Đang rảnh".into(),
            c_show_idle: true,
            c_force: false,
            ci_on: false,
            ci_cid: String::new(),
            ci_type: "Đang nghe".into(),
            ci_d: String::new(),
            ci_s: String::new(),
            ci_img: String::new(),
            ci_lt: String::new(),
            ci_timer: false,
            ci_label: [String::new(), String::new()],
            ci_url: [String::new(), String::new()],
            i_theme: "dark".into(),
            dim: 35.0,
            blur: 0.0,
            v_kw: String::new(),
            bot_auto: false,
            i_tdom: "youtube.com".into(),
            test_tex: None,
            test_txt: String::new(),
            chan_names: Vec::new(),
            chan_ids: Vec::new(),
            chan_sel: 0,
            bot_ann: String::new(),
            bot_chan: String::new(),
            bot_msg: String::new(),
            bot_msg_bad: false,
            mini_open: false,
            mini_pos: mini_start_pos(),
            tray: None,
            tray_tip: String::new(),
            start_hidden,
            shown_once: false,
            quitting: false,
            ctx,
        };
        app.sync_from_cfg();
        crate::diag::note("App::new: sync_from_cfg xong");
        app.load_bg();
        app.refresh_bg_text();
        crate::diag::note("App::new: nền xong");
        app.setup_tray();
        crate::diag::note(&format!(
            "App::new: khay hệ thống {}",
            if app.tray.is_some() { "OK" } else { "KHÔNG tạo được" }
        ));
        if app.cfg_lock().get_bool("auto_start_presence") {
            app.engine.start();
            crate::diag::note("App::new: đã bật Presence tự động");
        }
        if app.bot_auto {
            app.bot.start();
            crate::diag::note("App::new: đã bật bot tự động");
        }
        crate::diag::note("App::new: xong");
        app
    }

    // ------------------------------------------------------------ config & tiện ích giao diện

    pub fn cfg_lock(&self) -> std::sync::MutexGuard<'_, Config> {
        self.cfg.lock().unwrap()
    }

    pub fn commit(&self) {
        let cfg = self.cfg.lock().unwrap();
        crate::config::save_config(&cfg);
    }

    /// Đổ toàn bộ cấu hình hiện tại vào các ô nhập (lúc mở app và sau khi nhập cấu hình).
    pub fn sync_from_cfg(&mut self) {
        let cfg_arc = self.cfg.clone();
        let c = cfg_arc.lock().unwrap();
        self.c_media = c.get_bool("show_media");
        self.c_browse = c.get_bool("show_browsing");
        self.c_toast = c.get_bool_or("toast_enabled", true);
        self.i_webtype = type_label(&c.get_str("web_default_type")).to_string();
        self.i_cid = c.get_str("client_id");
        self.i_interval = format!("{}", c.get_f64("interval_seconds"));
        self.i_img = c.get_str("large_image");
        self.i_idle_min = format!("{}", c.get_i64("idle_minutes"));
        self.i_bstate = c.get_str("browsing_state");
        self.c_hover = c.get_bool_or("show_large_text", true);
        self.c_focus = c.get_bool("browsing_requires_focus");
        self.c_auto = c.get_bool("auto_start_presence");
        self.c_tray = c.get_bool("minimize_to_tray");
        self.c_win = c.get_bool("start_with_windows");
        self.c_show_idle = c.get_bool_or("show_idle", true);
        self.s_idle_txt = c.get_str("idle_details");
        self.c_quiet = c.get_bool("quiet_enabled");
        self.i_qfrom = c.get_str("quiet_from");
        self.i_qto = c.get_str("quiet_to");
        self.i_fmode = c.get_str("favicon_mode");
        self.i_ftpl = c.get_str("favicon_url_template");
        self.c_addr = c.get_bool_or("read_address_bar", true);
        self.c_btn_on = c.get_bool_or("buttons_enabled", true);
        let pairs = crate::config::buttons_to_pairs(c.get("buttons"));
        for i in 0..2 {
            self.gb_label[i] = pairs.get(i).map(|(l, _)| l.clone()).unwrap_or_default();
            self.gb_url[i] = pairs.get(i).map(|(_, u)| u.clone()).unwrap_or_default();
        }
        self.ci_on = c.get_bool("idle_custom_enabled");
        self.ci_cid = c.get_str("idle_custom_client_id");
        self.ci_type = type_label(&c.get_str("idle_custom_type")).to_string();
        self.ci_d = c.get_str("idle_custom_details");
        self.ci_s = c.get_str("idle_custom_state");
        self.ci_img = c.get_str("idle_custom_large_image");
        self.ci_lt = c.get_str("idle_custom_large_text");
        self.ci_timer = c.get_bool("idle_custom_timer");
        let cpairs = crate::config::buttons_to_pairs(c.get("idle_custom_buttons"));
        for i in 0..2 {
            self.ci_label[i] = cpairs.get(i).map(|(l, _)| l.clone()).unwrap_or_default();
            self.ci_url[i] = cpairs.get(i).map(|(_, u)| u.clone()).unwrap_or_default();
        }
        self.bot_auto = c.get_bool("bot_auto_start");
        self.i_theme = c.get_str("theme");
        self.dim = c.get_f64("bg_dim") as f32;
        self.blur = c.get_f64("bg_blur") as f32;
        self.mini_pos = mini_parse_pos(&c.get_str("mini_pos"));
    }

    pub fn flash(&mut self, key: &str, text: &str) {
        self.flash.insert(key.to_string(), (text.to_string(), crate::util::now() + 2.5, false));
        self.ctx.request_repaint();
    }

    pub fn flash_bad(&mut self, key: &str, text: &str) {
        self.flash.insert(key.to_string(), (text.to_string(), crate::util::now() + 4.0, true));
        self.ctx.request_repaint();
    }

    pub fn flash_text(&self, key: &str) -> Option<(String, bool)> {
        match self.flash.get(key) {
            Some((t, until, bad)) if *until > crate::util::now() => Some((t.clone(), *bad)),
            _ => None,
        }
    }

    pub fn show_toast(&mut self, text: String) {
        self.toast = Some((text, crate::util::now()));
        self.ctx.request_repaint();
    }

    // ------------------------------------------------------------ hành động

    pub fn toggle_engine(&mut self) {
        if self.engine.running() {
            self.engine.stop();
        } else {
            self.engine.start();
        }
    }

    pub fn toggle_bot(&mut self) {
        if self.bot.running() {
            self.bot.stop();
        } else {
            self.bot.start();
        }
    }

    pub fn toggle_idle(&mut self) {
        let cur = FORCE_IDLE.load(AtomicOrdering::Relaxed);
        FORCE_IDLE.store(!cur, AtomicOrdering::Relaxed);
        self.c_force = !cur;
    }

    /// Nút ở cửa sổ thu nhỏ / khay: bật Presence.
    pub fn quick_set(&mut self, field: QuickField, value: bool) {
        match field {
            QuickField::Media => {
                self.c_media = value;
                self.cfg_lock().set_bool("show_media", value);
            }
            QuickField::Browse => {
                self.c_browse = value;
                self.cfg_lock().set_bool("show_browsing", value);
            }
            QuickField::Toast => {
                self.c_toast = value;
                self.cfg_lock().set_bool("toast_enabled", value);
            }
        }
        self.commit();
    }

    /// Gọi API nội bộ của bot ở luồng nền; kết quả về giao diện qua UiMsg.
    pub fn bot_call(&self, path: &str, data: Option<Value>, ok_text: &str, clear: u8) {
        self.ui_tx
            .send(UiMsg::BotMsg { text: "Đang gửi...".into(), bad: false, clear: 0 })
            .ok();
        let tx = self.ui_tx.clone();
        let path = path.to_string();
        let ok_text = ok_text.to_string();
        std::thread::Builder::new().name("bot-api".into()).spawn(move || {
            let res = bot_api(&path, data.as_ref(), 90);
            let msg = match &res {
                None => ("Không kết nối được bot. Hãy bật bot và đợi bot khởi động xong.".to_string(), true),
                Some(v) => {
                    if v.get("success").and_then(Value::as_bool).unwrap_or(false) {
                        let m = v.get("message").and_then(Value::as_str).unwrap_or("");
                        if m.is_empty() {
                            (ok_text, false)
                        } else {
                            (m.to_string(), false)
                        }
                    } else {
                        let e = v.get("error").and_then(Value::as_str).unwrap_or("không rõ");
                        (format!("Lỗi: {}", e), true)
                    }
                }
            };
            tx.send(UiMsg::BotMsg { text: msg.0, bad: msg.1, clear }).ok();
        }).ok();
    }

    pub fn bot_load_channels(&self) {
        self.ui_tx
            .send(UiMsg::BotMsg { text: "Đang tải...".into(), bad: false, clear: 0 })
            .ok();
        let tx = self.ui_tx.clone();
        std::thread::Builder::new().name("bot-chans".into()).spawn(move || {
            let res = bot_api("/api/channels", None, 10);
            match res {
                Some(Value::Array(arr)) => {
                    let list: Vec<(String, String)> = arr
                        .iter()
                        .map(|c| {
                            let name = format!(
                                "{} · #{}",
                                c.get("guildName").and_then(Value::as_str).unwrap_or(""),
                                c.get("name").and_then(Value::as_str).unwrap_or("")
                            );
                            (name, c.get("id").map(|v| v.to_string()).unwrap_or_default().trim_matches('"').to_string())
                        })
                        .collect();
                    let msg = if list.is_empty() {
                        ("Chưa có kênh nào (bot có thể chưa đăng nhập xong).".to_string(), true)
                    } else {
                        (format!("Đã tải {} kênh.", list.len()), false)
                    };
                    tx.send(UiMsg::Channels(list, msg.0, msg.1)).ok();
                }
                _ => {
                    tx.send(UiMsg::Channels(
                        Vec::new(),
                        "Không tải được danh sách kênh. Hãy bật bot và đợi bot khởi động xong.".into(),
                        true,
                    ))
                    .ok();
                }
            }
        }).ok();
    }

    /// Chạy fn ở luồng nền, kết quả (Vec<u8> hoặc None) gửi về giao diện.
    pub fn bg_fetch_favicon(&self, url: String) {
        let tx = self.ui_tx.clone();
        std::thread::Builder::new().name("fav-test".into()).spawn(move || {
            let bytes = IconCache::global().fetch(&url);
            tx.send(UiMsg::Favicon(bytes)).ok();
        }).ok();
    }

    pub fn bg_read_address(&self) {
        let tx = self.ui_tx.clone();
        std::thread::Builder::new().name("addr-test".into()).spawn(move || {
            let text = match win::top_browser_title() {
                None => "Không thấy cửa sổ trình duyệt nào (hoặc đang là tab riêng tư).".to_string(),
                Some((t, _b, hwnd)) => {
                    crate::browser::clear_url_cache();
                    match crate::browser::read_address_bar(hwnd, &t) {
                        Some(url) => format!("Đọc được: {}", url),
                        None => "Không đọc được địa chỉ từ cửa sổ này.".to_string(),
                    }
                }
            };
            tx.send(UiMsg::Address(text)).ok();
        }).ok();
    }

    pub fn set_botmsg(&mut self, text: &str, bad: bool) {
        self.bot_msg = crate::util::bot_clean(text);
        self.bot_msg_bad = bad;
    }

    // ------------------------------------------------------------ chế độ thu nhỏ & cửa sổ chính

    pub fn open_mini(&mut self) {
        if self.mini_open {
            return;
        }
        self.mini_open = true;
        self.ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        // ẩn ngay bằng WinAPI (không phụ thuộc việc egui có kịp vẽ hay không)
        crate::win::hide_own_main_window();
    }

    pub fn show_main(&mut self) {
        self.mini_open = false;
        self.ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd(ViewportCommand::Focus);
        // hiện ngay bằng WinAPI (egui có thể không chạy update() khi cửa sổ đang ẩn)
        crate::win::show_own_main_window();
    }

    pub fn save_mini_pos(&mut self) {
        let pos = mini_pos_string(self.mini_pos);
        self.cfg_lock().set_str("mini_pos", &pos);
        self.commit();
    }

    pub fn quit_app(&mut self) {
        self.quitting = true;
        self.engine.stop();
        self.bot.stop();
        self.ctx.send_viewport_cmd(ViewportCommand::Close);
        // lưới an toàn: nếu vòng vẽ không chạy (cửa sổ đang ẩn) thì vẫn thoát hẳn
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(2500));
            std::process::exit(0);
        });
    }

    // ------------------------------------------------------------ ảnh nền

    pub fn load_bg(&mut self) {
        self.bg_src = None;
        self.bg_key.clear();
        self.bg_job = None;
        self.bg_job_key.clear();
        let name = self.cfg_lock().get_str("bg_image");
        if name.is_empty() {
            return;
        }
        let mut path = std::path::PathBuf::from(&name);
        if !path.is_absolute() {
            path = crate::config::base_dir().join(&name);
        }
        match image::ImageReader::open(&path).ok().and_then(|r| r.decode().ok()) {
            Some(img) => {
                let mut dyn_img = img;
                dyn_img = dyn_img.thumbnail(2560, 1600);
                self.bg_src = Some(dyn_img.to_rgb8());
            }
            None => {
                self.logs.push((
                    crate::util::timestamp_hms(),
                    "Lỗi: không mở được ảnh nền, đang dùng nền mặc định".to_string(),
                ));
            }
        }
    }

    pub fn refresh_bg_text(&mut self) {
        let custom = !self.cfg_lock().get_str("bg_image").is_empty();
        self.bg_txt = match &self.bg_src {
            Some(src) => format!("Đang dùng ảnh của bạn ({}×{}).", src.width(), src.height()),
            None if custom => "Không mở được ảnh đã chọn, đang dùng nền mặc định.".to_string(),
            None => "Đang dùng nền mặc định.".to_string(),
        };
    }

    pub fn pick_bg(&mut self) {
        use rfd::FileDialog;
        let path = FileDialog::new()
            .set_title("Chọn ảnh nền")
            .add_filter("Ảnh", &["png", "jpg", "jpeg", "bmp", "webp", "gif"])
            .add_filter("Tất cả tệp", &["*"])
            .pick_file();
        let path = match path {
            Some(p) => p,
            None => return,
        };
        let img = image::ImageReader::open(&path).ok().and_then(|r| r.decode().ok());
        let img = match img {
            Some(i) => i.thumbnail(2560, 1600).to_rgb8(),
            None => {
                win::message_box("Không dùng được ảnh", "Không mở hoặc lưu được ảnh này.");
                return;
            }
        };
        let out = crate::config::base_dir().join(crate::util::BG_FILE);
        let f = match std::fs::File::create(&out) {
            Ok(f) => f,
            Err(e) => {
                win::message_box("Không dùng được ảnh", &format!("Không ghi được file:\n{}", e));
                return;
            }
        };
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(f, 92);
        if enc.encode_image(&img).is_err() {
            win::message_box("Không dùng được ảnh", "Không lưu được ảnh này.");
            return;
        }
        self.cfg_lock().set_str("bg_image", crate::util::BG_FILE);
        self.commit();
        self.load_bg();
        self.refresh_bg_text();
    }

    pub fn reset_bg(&mut self) {
        self.cfg_lock().set_str("bg_image", "");
        self.commit();
        let _ = std::fs::remove_file(crate::config::base_dir().join(crate::util::BG_FILE));
        self.load_bg();
        self.refresh_bg_text();
    }

    /// Giao diện tab Giao diện: ảnh xem trước 224×126.
    pub fn bg_thumb(&mut self, ctx: &Context, p: &Palette) -> Option<TextureHandle> {
        let key = format!(
            "{:?}|{}|{}",
            self.bg_src.as_ref().map(|i| (i.width(), i.height())),
            self.cfg_lock().get_str("bg_image"),
            format!("{:?}", p.field)
        );
        if self.bg_thumb.is_none() || self.bg_thumb_key != key {
            let src = self.bg_src.clone().unwrap_or_else(|| crate::bg::gradient_image(224, 126));
            let tile = crate::bg::art_tile(&src, 224, 126, 12.0, image::Rgb([p.field.r(), p.field.g(), p.field.b()]));
            let ci = egui::ColorImage::from_rgba_unmultiplied([224, 126], &crate::bg::rgb_to_rgba(&tile)[..]);
            let tex = ctx.load_texture("bg-thumb", ci, TextureOptions::LINEAR);
            self.bg_thumb = Some(tex);
            self.bg_thumb_key = key;
        }
        self.bg_thumb.clone()
    }

    fn maybe_rebuild_bg(&mut self, ctx: &Context, p: &Palette) {
        // 1) ảnh nền do luồng nền dựng xong -> nạp thành texture
        let mut job_result: Option<(String, image::RgbImage)> = None;
        if let Some(rx) = &self.bg_job {
            if let Ok(v) = rx.try_recv() {
                job_result = Some(v);
            }
        }
        if let Some((key, img)) = job_result {
            let (iw, ih) = (img.width() as usize, img.height() as usize);
            let data = crate::bg::rgb_to_rgba(&img);
            let color = egui::ColorImage::from_rgba_unmultiplied([iw, ih], &data);
            self.bg_tex = Some(ctx.load_texture("bg", color, TextureOptions::LINEAR));
            self.bg_key = key;
            self.bg_job = None;
            self.bg_job_key.clear();
            ctx.request_repaint();
        }

        // 2) kích thước cửa sổ thật. Lúc mới khởi tạo (hoặc khi cửa sổ đang ẩn / thu nhỏ),
        // ctx.screen_rect() có thể vẫn là 10000x10000 mặc định của egui -> cấp phát ảnh khổng lồ.
        // Vì vậy ưu tiên inner_rect của viewport và chặn trên theo kích thước màn hình thật.
        let mut screen = ctx.screen_rect();
        if let Some(r) = ctx.input(|i| i.viewport().inner_rect) {
            if r.width() >= 1.0 && r.height() >= 1.0 {
                screen = r;
            }
        }
        let (_, _, vw, vh) = win::virtual_screen();
        let max_w = if vw > 0 { vw as f32 } else { 8000.0 };
        let max_h = if vh > 0 { vh as f32 } else { 8000.0 };
        let w = screen.width().round().min(max_w) as i32;
        let h = screen.height().round().min(max_h) as i32;
        if w < 100 || h < 100 {
            return;
        }
        // chỉ khoá cfg MỘT lần: khoá nhiều lần trong cùng một câu lệnh sẽ deadlock (Mutex không reentrant)
        let (bg_image, bg_dim, bg_blur) = {
            let c = self.cfg_lock();
            (c.get_str("bg_image"), c.get_f64("bg_dim"), c.get_f64("bg_blur"))
        };
        let key = format!(
            "{}x{}|{}|{}|{}|{}|{}",
            w,
            h,
            bg_image,
            bg_dim,
            bg_blur,
            self.i_theme,
            format!("{:?}", p.card)
        );
        if self.bg_key == key && self.bg_tex.is_some() {
            return;
        }
        if self.bg_job.is_some() {
            // đang dựng ảnh nền ở luồng nền; xong sẽ tự nạp ở bước 1
            return;
        }

        // 3) dựng ở luồng nền: dựng ảnh nền rất nặng (phóng ảnh + làm mờ + bo góc), nếu làm ngay
        // trong khung hình đầu tiên thì cửa sổ có thể đứng hình vài giây tới vài chục giây.
        let src = self.bg_src.clone();
        let tint = Color32::from_rgb(GLASS.r(), GLASS.g(), GLASS.b());
        let card = p.card;
        let line = p.line;
        let key2 = key.clone();
        let (tx, rx) = crossbeam_channel::bounded::<(String, image::RgbImage)>(1);
        let spawned = std::thread::Builder::new()
            .name("bg-build".into())
            .spawn(move || {
                let img = App::build_bg(w, h, src, bg_dim as i32, bg_blur as i32, tint, card, line);
                let _ = tx.send((key2, img));
            });
        match spawned {
            Ok(_) => {
                self.bg_job = Some(rx);
                self.bg_job_key = key;
            }
            Err(_) => {
                // không tạo được luồng: đành dựng ngay trên luồng giao diện
                let img = App::build_bg(
                    w,
                    h,
                    self.bg_src.clone(),
                    bg_dim as i32,
                    bg_blur as i32,
                    tint,
                    card,
                    line,
                );
                let data = crate::bg::rgb_to_rgba(&img);
                let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &data);
                self.bg_tex = Some(ctx.load_texture("bg", color, TextureOptions::LINEAR));
                self.bg_key = key;
            }
        }
    }

    /// Dựng ảnh nền hoàn chỉnh (thuần dữ liệu nên chạy được ở luồng nền).
    fn build_bg(
        w: i32,
        h: i32,
        src: Option<image::RgbImage>,
        dim: i32,
        blur: i32,
        tint: Color32,
        card: Color32,
        line: Color32,
    ) -> image::RgbImage {
        let mut img = match src {
            Some(s) => crate::bg::cover_resize(&s, w as u32, h as u32),
            None => crate::bg::gradient_image(w as u32, h as u32),
        };
        if blur > 0 {
            // làm mờ trên bản nửa kích thước rồi phóng lại: nhanh hơn, mắt thường không phân biệt được
            let sw = (w / 2).max(1) as u32;
            let sh = (h / 2).max(1) as u32;
            let small = image::imageops::resize(&img, sw, sh, image::imageops::FilterType::Triangle);
            let small = image::imageops::blur(&small, blur as f32 / 2.0);
            img = image::imageops::resize(&small, w as u32, h as u32, image::imageops::FilterType::Triangle);
        }
        if dim > 0 {
            let t = dim as f32 / 100.0;
            for px in img.pixels_mut() {
                let lerp = |x: u8| (x as f32 * (1.0 - t)).round() as u8;
                *px = image::Rgb([lerp(px[0]), lerp(px[1]), lerp(px[2])]);
            }
        }
        let boxes = panel_boxes(w as f32, h as f32);
        let to_box = |b: (f32, f32, f32, f32)| (b.0 as i32, b.1 as i32, (b.2 - b.0) as i32, (b.3 - b.1) as i32);
        let (sx, sy, sw, sh) = to_box(boxes.side);
        let (hx, hy, hw, hh) = to_box(boxes.head);
        let (cx, cy, cw, ch) = to_box(boxes.card);
        let (tx, ty, tw, th) = to_box(boxes.tally);
        let u = |v: i32| v.max(0) as u32;
        crate::bg::glass_panel(&mut img, u(sx), u(sy), u(sw), u(sh), crate::theme::RADIUS, tint, crate::theme::GLASS_ALPHA);
        crate::bg::glass_panel(&mut img, u(hx), u(hy), u(hw), u(hh), crate::theme::RADIUS, tint, crate::theme::GLASS_ALPHA);
        crate::bg::solid_panel(
            &mut img,
            u(cx),
            u(cy),
            u(cw),
            u(ch),
            crate::theme::RADIUS,
            image::Rgb([card.r(), card.g(), card.b()]),
            image::Rgb([line.r(), line.g(), line.b()]),
            1.0,
        );
        crate::bg::solid_panel(
            &mut img,
            u(tx),
            u(ty),
            u(tw),
            u(th),
            14.0,
            image::Rgb([TALLY.r(), TALLY.g(), TALLY.b()]),
            image::Rgb([255, 255, 255]),
            0.07,
        );
        img
    }

    // ------------------------------------------------------------ khay hệ thống

    fn setup_tray(&mut self) {
        use tray_icon::menu::{Menu, MenuEvent, MenuItem};
        use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
        let (w, h, rgba) = make_icon_rgba();
        let icon = match tray_icon::Icon::from_rgba(rgba, w, h) {
            Ok(i) => i,
            Err(e) => {
                crate::diag::log(&format!("khay: icon lỗi: {e}"));
                return;
            }
        };
        let menu = Menu::new();
        let mi_open = MenuItem::new("Mở cửa sổ", true, None);
        let mi_mini = MenuItem::new("Chế độ thu nhỏ", true, None);
        let mi_idle = MenuItem::new("Chuyển sang rảnh", true, None);
        let mi_bot = MenuItem::new("Bật bot Nino", true, None);
        let mi_presence = MenuItem::new("Bật Presence", true, None);
        let mi_quit = MenuItem::new("Thoát", true, None);
        let _ = menu.append(&mi_open);
        let _ = menu.append(&mi_mini);
        let _ = menu.append(&mi_idle);
        let _ = menu.append(&mi_bot);
        let _ = menu.append(&mi_presence);
        let _ = menu.append(&mi_quit);

        // Xử lý sự kiện bằng handler (chạy ngay khi có click), KHÔNG đợi update() của egui:
        // khi cửa sổ đang ẩn, egui có thể không vẽ lại nên polling trong update() sẽ không bao giờ chạy.
        let ids = [
            mi_open.id().clone(),
            mi_mini.id().clone(),
            mi_idle.id().clone(),
            mi_bot.id().clone(),
            mi_presence.id().clone(),
            mi_quit.id().clone(),
        ];
        let ctx_menu = self.ctx.clone();
        MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
            if let Some(a) = ids.iter().position(|id| id == ev.id()) {
                tray_dispatch(a as u8, &ctx_menu);
            }
        }));
        let ctx_icon = self.ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |ev: TrayIconEvent| {
            // bấm chuột trái vào icon -> mở cửa sổ chính
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                tray_dispatch(0, &ctx_icon);
            }
        }));

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false) // chuột trái = mở cửa sổ, chuột phải = menu
            .with_tooltip("Discord Presence")
            .with_icon(icon)
            .build();
        match tray {
            Ok(t) => {
                self.tray = Some(TrayUi {
                    icon: t,
                    mi_open,
                    mi_mini,
                    mi_idle,
                    mi_bot,
                    mi_presence,
                    mi_quit,
                });
            }
            Err(e) => crate::diag::log(&format!("khay: tạo TrayIcon lỗi: {e}")),
        }
    }

    fn poll_tray(&mut self) {
        let actions: Vec<u8> = std::mem::take(&mut *TRAY_Q.lock().unwrap());
        for a in actions {
            match a {
                0 => self.show_main(),
                1 => self.open_mini(),
                2 => self.toggle_idle(),
                3 => self.toggle_bot(),
                4 => self.toggle_engine(),
                _ => self.quit_app(),
            }
        }
    }

    // ------------------------------------------------------------ vòng tick

    fn tick_start(&mut self, ctx: &Context) {
        // ẩn cửa sổ khi mở bằng --minimized
        if self.start_hidden && !self.shown_once {
            self.shown_once = true;
            if self.cfg_lock().get_bool("minimize_to_tray") {
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
        }
        self.poll_tray();

        // sự kiện từ luồng nền
        while let Ok(m) = self.ui_rx.try_recv() {
            match m {
                UiMsg::BotMsg { text, bad, clear } => {
                    self.set_botmsg(&text, bad);
                    match clear {
                        1 => self.bot_ann.clear(),
                        2 => self.bot_chan.clear(),
                        _ => {}
                    }
                }
                UiMsg::Favicon(bytes) => match bytes {
                    Some(b) => {
                        self.test_tex = tex_from_bytes(ctx, "favicon-test", &b, 32, 32);
                        self.test_txt = "Tải được favicon".to_string();
                    }
                    None => {
                        self.test_tex = None;
                        self.test_txt = "Không tải được favicon (kiểm tra mạng / nguồn favicon).".to_string();
                    }
                },
                UiMsg::Address(text) => self.test_txt = text,
                UiMsg::Channels(chans, msg, bad) => {
                    self.chan_names = chans.iter().map(|(n, _)| n.clone()).collect();
                    self.chan_ids = chans.iter().map(|(_, i)| i.clone()).collect();
                    self.chan_sel = 0;
                    self.set_botmsg(&msg, bad);
                }
            }
        }

        // nhật ký engine + bot (một kênh chung -> cả hai tab log đều thấy)
        while let Ok((ts, msg)) = self.log_rx.try_recv() {
            self.logs.push((ts.clone(), msg.clone()));
            self.botlogs.push((ts, msg));
        }
        if self.logs.len() > LOG_MAX {
            self.logs.drain(..self.logs.len() - LOG_MAX);
        }
        if self.botlogs.len() > LOG_MAX {
            self.botlogs.drain(..self.botlogs.len() - LOG_MAX);
        }

        // đồng bộ công tắc rảnh
        self.c_force = FORCE_IDLE.load(AtomicOrdering::Relaxed);

        // nội dung thẻ xem trước
        let running = self.engine.running();
        let cur = if running { self.engine.current() } else { None };
        if let Some(ref a) = cur {
            let kind = a.get("activity_type").and_then(Value::as_i64).unwrap_or(0);
            let tag = if crate::util::truthy(a.get("_idle")) {
                if a.get("_cid").is_some() {
                    " · rảnh tuỳ chỉnh"
                } else {
                    " · rảnh"
                }
            } else {
                ""
            };
            self.pv_type = format!("{}{}", type_label(kind_of_code(kind)), tag);
            self.pv_details = a.get("details").and_then(Value::as_str).unwrap_or("").to_string();
            self.pv_state = a.get("state").and_then(Value::as_str).unwrap_or("").to_string();
        } else if !running {
            self.pv_type = "Đã tắt".to_string();
            self.pv_details = "Presence đang tắt".to_string();
            self.pv_state.clear();
        } else if in_quiet_hours(&self.cfg_lock()) {
            self.pv_type = "Giờ yên lặng".to_string();
            self.pv_details = "Đang ẩn trạng thái theo giờ yên lặng".to_string();
            self.pv_state.clear();
        } else {
            self.pv_type = "Chưa có gì".to_string();
            self.pv_details = "Chưa có gì đang hiển thị trên Discord".to_string();
            self.pv_state.clear();
        }

        self.check_toast(running, cur.as_ref());
    }

    fn tick_end(&mut self, ctx: &Context) {
        let running = self.engine.running();
        let cur = if running { self.engine.current() } else { None };
        let mut tip = format!("Discord Presence — {}", if running { "đang bật" } else { "đã tắt" });
        if let Some(ref a) = cur {
            let kind = a.get("activity_type").and_then(Value::as_i64).unwrap_or(0);
            let tag = if crate::util::truthy(a.get("_idle")) {
                if a.get("_cid").is_some() {
                    " · rảnh tuỳ chỉnh"
                } else {
                    " · rảnh"
                }
            } else {
                ""
            };
            let details = a.get("details").and_then(Value::as_str).unwrap_or("");
            tip = format!("Discord Presence — {}{}: {}", type_label(kind_of_code(kind)), tag, details);
        }
        let tip: String = tip.chars().take(120).collect();
        let update_tip = tip != self.tray_tip;
        if let Some(t) = &mut self.tray {
            if update_tip {
                let _ = t.icon.set_tooltip(Some(tip.as_str()));
                self.tray_tip = tip.clone();
            }
            let _ = t.mi_idle.set_text(if FORCE_IDLE.load(AtomicOrdering::Relaxed) {
                "Thoát trạng thái rảnh"
            } else {
                "Chuyển sang rảnh"
            });
            let _ = t.mi_bot.set_text(if self.bot.running() { "Tắt bot Nino" } else { "Bật bot Nino" });
            let _ = t.mi_presence.set_text(if running { "Tắt Presence" } else { "Bật Presence" });
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn check_toast(&mut self, running: bool, cur: Option<&Value>) {
        if !running {
            self.toast_key = None; // tắt Presence -> lần bật sau không báo
            return;
        }
        let key = match cur.and_then(|a| a.get("_src")).and_then(Value::as_str) {
            Some(k) => k.to_string(),
            None => return,
        };
        let prev = self.toast_key.replace(key.clone());
        if let Some(prev) = prev {
            if prev != key
                && self.cfg_lock().get_bool_or("toast_enabled", true)
                && crate::util::now() - self.toast_at > 2.0
            {
                self.toast_at = crate::util::now();
                let short = |s: &str| -> String {
                    let chars: Vec<char> = s.chars().collect();
                    if chars.len() <= 28 {
                        s.to_string()
                    } else {
                        chars[..27].iter().collect::<String>() + "…"
                    }
                };
                self.show_toast(format!("{}  →  {}", short(&prev), short(&key)));
            }
        }
    }

    // ------------------------------------------------------------ khung: thanh bên + tiêu đề + thẻ

    fn paint_sidebar(&mut self, ctx: &Context, p: &Palette) {
        let screen = ctx.screen_rect();
        let (x0, y0, x1, y1) = (screen.left(), screen.top(), screen.right(), screen.bottom());
        let boxes = panel_boxes(screen.width(), screen.height());
        let (M, SW, NAV_H) = (crate::theme::MARGIN, crate::theme::SIDE_W, crate::theme::NAV_H);
        let side_rect = Rect::from_min_max(Pos2::new(x0 + M, y0 + M), Pos2::new(x0 + M + SW, y1 - M));
        egui::Area::new(Id::new("sidebar")).fixed_pos(side_rect.min).show(ctx, |ui| {
            ui.set_min_size(side_rect.size());
            ui.set_max_size(side_rect.size());
            // logo + tên app
            if self.logo_tex.is_none() {
                let (w, h, rgba) = make_icon_rgba();
                let im = image::RgbaImage::from_raw(w, h, rgba).unwrap_or_else(|| image::RgbaImage::new(1, 1));
                let im = image::imageops::resize(&im, 34, 34, image::imageops::FilterType::Triangle);
                let data: Vec<u8> = im.pixels().flat_map(|q| [q[0], q[1], q[2], q[3]]).collect();
                let ci = egui::ColorImage::from_rgba_unmultiplied([34, 34], &data);
                self.logo_tex = Some(ctx.load_texture("logo", ci, TextureOptions::LINEAR));
            }
            if let Some(tex) = &self.logo_tex {
                let rect = Rect::from_min_size(Pos2::new(x0 + M + 18.0, y0 + M + 18.0), Vec2::splat(34.0));
                ui.painter().image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            ui.painter().text(
                Pos2::new(x0 + M + 62.0, y0 + M + 19.0),
                egui::Align2::LEFT_TOP,
                "Discord Presence",
                FontId::proportional(14.5),
                SIDE_TEXT,
            );
            ui.painter().text(
                Pos2::new(x0 + M + 62.0, y0 + M + 41.0),
                egui::Align2::LEFT_TOP,
                format!("Phiên bản {}", APP_VERSION),
                FontId::proportional(11.0),
                SIDE_MUTED,
            );

            // các mục điều hướng
            let mut y = y0 + M + 78.0;
            let unseen = self.cfg_lock().get_str("seen_version") != APP_VERSION;
            for (key, label, _sub, gap) in PAGES {
                y += gap;
                let rect = Rect::from_min_max(Pos2::new(x0 + M + 8.0, y), Pos2::new(x0 + M + SW - 8.0, y + NAV_H));
                let resp = ui.allocate_rect(rect, Sense::click());
                let active = self.cur == key;
                let hover = resp.hovered();
                if active {
                    ui.painter()
                        .rect_filled(rect, 10.0, Color32::from_rgba_unmultiplied(255, 111, 145, 56));
                    ui.painter().rect_filled(
                        Rect::from_min_size(Pos2::new(x0 + M + 20.0, y + NAV_H / 2.0 - 3.0), Vec2::splat(6.0)),
                        3.0,
                        Color32::from_rgb(255, 111, 145),
                    );
                } else if hover {
                    ui.painter().rect_filled(rect, 10.0, Color32::from_rgba_unmultiplied(255, 255, 255, 18));
                }
                ui.painter().text(
                    Pos2::new(x0 + M + 36.0, y + NAV_H / 2.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    if active { FontId::proportional(13.5) } else { FontId::proportional(13.0) },
                    if active || hover { SIDE_TEXT } else { SIDE_MUTED },
                );
                if key == "changelog" && unseen && !active {
                    let brect = Rect::from_min_size(
                        Pos2::new(x0 + M + SW - 16.0 - 34.0, y + NAV_H / 2.0 - 8.5),
                        Vec2::new(34.0, 17.0),
                    );
                    ui.painter().rect_filled(brect, 8.0, Color32::from_rgb(255, 191, 94));
                    ui.painter().text(
                        Pos2::new(x0 + M + SW - 16.0 - 17.0, y + NAV_H / 2.0),
                        egui::Align2::CENTER_CENTER,
                        "Mới",
                        FontId::proportional(11.0),
                        Color32::from_rgb(0x2b, 0x1a, 0x05),
                    );
                }
                if resp.clicked() {
                    self.cur = key;
                    if key == "changelog" && self.cfg_lock().get_str("seen_version") != APP_VERSION {
                        self.cfg_lock().set_str("seen_version", APP_VERSION);
                        self.commit();
                    }
                }
                y += NAV_H + 2.0;
            }

            // khối trạng thái ở đáy thanh bên
            let (tx0, ty0, tx1, ty1) = boxes.tally;
            let running = self.engine.running();
            let status = self.engine.status();
            let color = if running {
                if status.contains("Đã kết nối") {
                    DOT_ON
                } else {
                    DOT_WARN
                }
            } else {
                DOT_OFF
            };
            ui.painter().rect_filled(
                Rect::from_min_size(Pos2::new(tx0 + 14.0, ty0 + 16.0), Vec2::splat(10.0)),
                5.0,
                color,
            );
            ui.painter().text(
                Pos2::new(tx0 + 32.0, ty0 + 11.0),
                egui::Align2::LEFT_TOP,
                status,
                FontId::proportional(12.0),
                SIDE_TEXT,
            );
            let btn_rect = Rect::from_min_size(
                Pos2::new(tx0 + 12.0, ty1 - 12.0 - 36.0),
                Vec2::new(tx1 - tx0 - 24.0, 36.0),
            );
            ui.allocate_ui_at_rect(btn_rect, |ui| {
                let label = if running { "Tắt Presence" } else { "Bật Presence" };
                let btn = egui::Button::new(label)
                    .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 26))
                    .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 46)))
                    .min_size(Vec2::new(btn_rect.width(), 36.0));
                if ui.add_sized([btn_rect.width(), 36.0], btn).clicked() {
                    self.toggle_engine();
                }
            });
        });
    }

    fn paint_header(&mut self, ctx: &Context, _p: &Palette) {
        let screen = ctx.screen_rect();
        let (M, SW, HEAD_H) = (crate::theme::MARGIN, crate::theme::SIDE_W, crate::theme::HEAD_H);
        let cx = M * 2.0 + SW;
        let (_key, label, sub, _g) = PAGES.iter().find(|p| p.0 == self.cur).unwrap();
        ui_painter(ctx).text(
            Pos2::new(cx + 26.0, M + 13.0),
            egui::Align2::LEFT_TOP,
            *label,
            FontId::proportional(23.0),
            SIDE_TEXT,
        );
        ui_painter(ctx).text(
            Pos2::new(cx + 26.0, M + 47.0),
            egui::Align2::LEFT_TOP,
            *sub,
            FontId::proportional(13.0),
            SIDE_MUTED,
        );
    }

    fn paint_card(&mut self, ctx: &Context, p: &Palette) {
        let screen = ctx.screen_rect();
        let boxes = panel_boxes(screen.width(), screen.height());
        let (x0, y0, x1, y1) = boxes.card;
        let pad = crate::theme::CARD_PAD;
        let rect = Rect::from_min_max(Pos2::new(x0 + pad, y0 + pad), Pos2::new(x1 - pad, y1 - pad));
        let app: &mut App = self;
        egui::Area::new(Id::new("card"))
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                egui::Frame::none()
                    .inner_margin(egui::Margin {
                        left: 20.0,
                        right: 14.0,
                        top: 16.0,
                        bottom: 12.0,
                    })
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                crate::pages::show_page(app, ui, p);
                            });
                    });
            });
    }

    // ------------------------------------------------------------ cửa sổ thu nhỏ

    fn show_mini_viewport(&mut self, ctx: &Context, p: &Palette) {
        let mut pos = self.mini_pos;
        let pv_type = self.pv_type.clone();
        let pv_details = self.pv_details.clone();
        let pv_state = self.pv_state.clone();
        let running = self.engine.running();
        let forced = FORCE_IDLE.load(AtomicOrdering::Relaxed);
        let dot_color = if running {
            if self.engine.status().contains("Đã kết nối") {
                DOT_ON
            } else {
                DOT_WARN
            }
        } else {
            DOT_OFF
        };
        let kind_color = theme::kind_color(p, &pv_type);
        let card = p.card;
        let muted = p.muted;
        let text = p.text;
        let accent = p.accent;
        let mut save_pos = false;
        let mut actions = MiniAction::None;
        ctx.show_viewport_immediate(
            ViewportId("mini".into()),
            egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_resizable(false)
                .with_always_on_top()
                .with_taskbar(false)
                .with_inner_size(Vec2::new(300.0, 132.0))
                .with_position(Pos2::new(pos.0, pos.1)),
            |vp_ctx, _class| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(card).inner_margin(egui::Margin::same(1.0)))
                    .show(vp_ctx, |ui| {
                        egui::Frame::none().fill(card).inner_margin(egui::Margin {
                            left: 12.0,
                            right: 12.0,
                            top: 10.0,
                            bottom: 10.0,
                        }).show(ui, |ui| {
                            // thanh tiêu đề: kéo để di chuyển
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click_and_drag());
                            ui.painter().rect_filled(rect, 4.0, card);
                            ui.painter().circle_filled(Pos2::new(rect.left() + 6.0, rect.center().y), 5.0, dot_color);
                            ui.painter().text(
                                Pos2::new(rect.left() + 20.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                cut(&pv_type, 24),
                                FontId::proportional(12.5),
                                kind_color,
                            );
                            if resp.dragged() {
                                pos.0 += resp.drag_delta().x;
                                pos.1 += resp.drag_delta().y;
                                let (vx, vy, vw, vh) = win::virtual_screen();
                                pos.0 = crate::util::clamp_safe(pos.0, vx as f32, (vx + vw) as f32 - 300.0);
                                pos.1 = crate::util::clamp_safe(pos.1, vy as f32, (vy + vh) as f32 - 132.0);
                                vp_ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(pos.0, pos.1)));
                            }
                            if resp.drag_stopped() {
                                save_pos = true;
                            }
                            if resp.double_clicked() {
                                actions = MiniAction::Expand;
                            }
                            ui.add_space(4.0);
                            ui.label(RichTextText::new(cut(&pv_details, 32), 15.0, text));
                            ui.label(RichTextText::new(cut(&pv_state, 46), 12.0, muted));
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                let idle_btn = egui::Button::new(
                                    egui::RichText::new(if forced { "Thoát rảnh" } else { "Chuyển sang rảnh" })
                                        .color(Color32::from_rgb(0x2b, 0x0f, 0x1b)),
                                )
                                .fill(accent);
                                if ui
                                    .add_enabled(running, idle_btn)
                                    .clicked()
                                {
                                    actions = MiniAction::Idle;
                                }
                                if ui.button("Mở rộng").clicked() {
                                    actions = MiniAction::Expand;
                                }
                            });
                        });
                    });
            },
        );
        self.mini_pos = pos;
        if save_pos {
            self.save_mini_pos();
        }
        match actions {
            MiniAction::Idle => self.toggle_idle(),
            MiniAction::Expand => self.show_main(),
            MiniAction::None => {}
        }
    }

    // ------------------------------------------------------------ thông báo nổi

    fn show_toast_viewport(&mut self, ctx: &Context, _p: &Palette) {
        let (text, started) = match &self.toast {
            Some(t) => (t.0.clone(), t.1),
            None => return,
        };
        let now = crate::util::now();
        // fade in 6 bước × 30ms, đứng 3200ms, fade out 10 bước × 30ms
        let t = now - started;
        let fade_in = 6.0 * 0.03;
        let hold = 3.2;
        let fade_out = 10.0 * 0.03;
        const ALPHA: f32 = 0.92;
        let alpha = if t < fade_in {
            ((t / fade_in) as f32) * ALPHA
        } else if t < fade_in + hold {
            ALPHA
        } else if t < fade_in + hold + fade_out {
            ((1.0 - (t - fade_in - hold) / fade_out) as f32) * ALPHA
        } else {
            0.0
        };
        if t >= fade_in + hold + fade_out {
            self.toast = None;
            return;
        }
        let alpha = alpha.clamp(0.05, 0.92);
        let accent = Color32::from_rgb(0xff, 0x6f, 0x91);
        let text_show = text.clone();
        let (sw, _sh) = win::screen_size();
        ctx.show_viewport_immediate(
            ViewportId("toast".into()),
            egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_resizable(false)
                .with_always_on_top()
                .with_taskbar(false)
                .with_transparent(true)
                .with_inner_size(Vec2::new(460.0, 46.0))
                .with_position(Pos2::new(((sw as f32) - 460.0) / 2.0, 28.0)),
            |vp_ctx, _class| {
                egui::CentralPanel::default().frame(egui::Frame::none()).show(vp_ctx, |ui| {
                    let rect = ui.max_rect();
                    let a = (alpha * 255.0) as u8;
                    ui.painter().rect_filled(
                        Rect::from_min_size(Pos2::new(rect.left() + 2.0, rect.top()), Vec2::new(rect.width() - 4.0, rect.height() - 2.0)),
                        12.0,
                        Color32::from_rgba_unmultiplied(GLASS.r(), GLASS.g(), GLASS.b(), a),
                    );
                    ui.painter().circle_filled(
                        Pos2::new(rect.left() + 20.0, rect.center().y),
                        4.5,
                        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), a),
                    );
                    ui.painter().text(
                        Pos2::new(rect.left() + 34.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        text_show,
                        FontId::proportional(13.5),
                        Color32::from_rgba_unmultiplied(SIDE_TEXT.r(), SIDE_TEXT.g(), SIDE_TEXT.b(), a),
                    );
                });
            },
        );
    }
}

#[derive(PartialEq)]
enum MiniAction {
    None,
    Idle,
    Expand,
}

#[derive(Clone, Copy)]
pub enum QuickField {
    Media,
    Browse,
    Toast,
}

// ---------------------------------------------------------------- khung & toạ độ

pub struct Boxes {
    pub side: (f32, f32, f32, f32),
    pub head: (f32, f32, f32, f32),
    pub card: (f32, f32, f32, f32),
    pub tally: (f32, f32, f32, f32),
}

/// Toạ độ các mặt phẳng bo góc (port panel_boxes).
pub fn panel_boxes(w: f32, h: f32) -> Boxes {
    let m = crate::theme::MARGIN;
    let cx = m * 2.0 + crate::theme::SIDE_W;
    Boxes {
        side: (m, m, m + crate::theme::SIDE_W, h - m),
        head: (cx, m, w - m, m + crate::theme::HEAD_H),
        card: (cx, m + crate::theme::HEAD_H + crate::theme::GAP, w - m, h - m),
        tally: (m + 10.0, h - m - 10.0 - crate::theme::TALLY_H, m + crate::theme::SIDE_W - 10.0, h - m - 10.0),
    }
}

fn ui_painter(ctx: &Context) -> egui::Painter {
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, Id::new("canvas")))
        .with_clip_rect(screen)
}

/// Cắt bớt chữ + chuẩn hoá khoảng trắng (MiniWindow._cut).
pub fn cut(text: &str, n: usize) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = t.chars().collect();
    if chars.len() <= n {
        t
    } else {
        chars[..n - 1].iter().collect::<String>() + "…"
    }
}

/// Vị trí mặc định của cửa sổ thu nhỏ: góc dưới bên phải, trên thanh tác vụ.
pub fn mini_start_pos() -> (f32, f32) {
    let (sw, sh) = win::screen_size();
    (sw as f32 - 300.0 - 24.0, sh as f32 - 132.0 - 80.0)
}

pub fn mini_parse_pos(s: &str) -> (f32, f32) {
    let re = crate::util::regex_cache("^\\+(-?\\d+)\\+(-?\\d+)$");
    if let Some(m) = re.captures(s) {
        let x: f32 = m.get(1).unwrap().as_str().parse().unwrap_or(0.0);
        let y: f32 = m.get(2).unwrap().as_str().parse().unwrap_or(0.0);
        let (vx, vy, vw, vh) = win::virtual_screen();
        return (
            crate::util::clamp_safe(x, vx as f32, (vx + vw) as f32 - 300.0),
            crate::util::clamp_safe(y, vy as f32, (vy + vh) as f32 - 132.0),
        );
    }
    mini_start_pos()
}

pub fn mini_pos_string(p: (f32, f32)) -> String {
    format!("+{}+{}", p.0 as i32, p.1 as i32)
}

/// Biểu tượng app: lấy từ app.ico; không có file (hoặc lỗi) thì dùng hình vẽ mặc định.
pub fn make_icon_rgba() -> (u32, u32, Vec<u8>) {
    let path = crate::config::resource_path(crate::util::ICON_FILE);
    if let Some(img) = image::ImageReader::open(&path).ok().and_then(|r| r.decode().ok()) {
        let rgba = img.to_rgba8();
        return (rgba.width(), rgba.height(), rgba.into_raw());
    }
    // hình tròn trắng trên nền hồng #ff6f91
    let mut img = image::RgbaImage::from_pixel(64, 64, image::Rgba([0xff, 0x6f, 0x91, 255]));
    for y in 0..64u32 {
        for x in 0..64u32 {
            let dx = x as f32 - 32.0;
            let dy = y as f32 - 32.0;
            if dx * dx + dy * dy <= 18.0 * 18.0 {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
    }
    (64, 64, img.into_raw())
}

/// Giải mã bytes ảnh (png/jpg/…) và cắt thành ô w×h bo góc để làm texture.
pub fn tex_from_bytes(ctx: &Context, name: &str, bytes: &[u8], w: u32, h: u32) -> Option<TextureHandle> {
    let img = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let rgb = img.to_rgb8();
    let tile = crate::bg::art_tile(&rgb, w, h, 8.0, image::Rgb([0x2a, 0x24, 0x33]));
    let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &crate::bg::rgb_to_rgba(&tile));
    Some(ctx.load_texture(name.to_string(), ci, TextureOptions::LINEAR))
}

// ---------------------------------------------------------------- vòng update chính

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        crate::diag::first_frame_started();
        self.ctx = ctx.clone();
        crate::diag::stage(1, "bắt đầu update");
        // eframe 0.27 không còn on_close_event: bắt yêu cầu đóng và huỷ nếu chỉ thu xuống khay
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
        self.tick_start(ctx);
        crate::diag::stage(2, "tick_start xong");

        let p = theme::palette(&self.cfg_lock().get_str("theme"));
        crate::diag::stage(3, "đọc bảng màu xong");

        // nền: vẽ toàn màn hình qua CentralPanel rỗng
        self.maybe_rebuild_bg(ctx, &p);
        crate::diag::stage(4, "dựng ảnh nền xong");
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let screen = ui.max_rect();
            if let Some(tex) = &self.bg_tex {
                ui.painter().image(
                    tex.id(),
                    screen,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                ui.painter().rect_filled(screen, 0.0, Color32::from_rgb(0x14, 0x15, 0x1b));
            }
        });

        self.paint_sidebar(ctx, &p);
        crate::diag::stage(5, "vẽ thanh bên xong");
        self.paint_header(ctx, &p);
        crate::diag::stage(6, "vẽ tiêu đề xong");
        self.paint_card(ctx, &p);
        crate::diag::stage(7, "vẽ nội dung xong");

        // hộp thoại sửa trang web
        if self.dialog.is_some() {
            crate::pages::dialog::show(self, ctx, &p);
        }
        crate::diag::stage(8, "hộp thoại xong");

        // cửa sổ thu nhỏ
        if self.mini_open {
            self.show_mini_viewport(ctx, &p);
        }
        crate::diag::stage(9, "cửa sổ thu nhỏ xong");

        // thông báo nổi
        if self.toast.is_some() {
            self.show_toast_viewport(ctx, &p);
        }
        crate::diag::stage(10, "thông báo nổi xong");

        self.tick_end(ctx);
        crate::diag::stage(11, "tick_end xong");
        crate::diag::first_frame_done();
    }

}

impl App {
    /// Trả về true nếu cho phép đóng cửa sổ.
    fn allow_close(&mut self) -> bool {
        // lưu kích thước cửa sổ
        {
            let screen = self.ctx.screen_rect();
            let mut cfg = self.cfg_lock();
            cfg.set_str("window_size", &format!("{}x{}", screen.width() as i32, screen.height() as i32));
        }
        self.commit();
        if self.quitting {
            return true;
        }
        if self.cfg_lock().get_bool("minimize_to_tray") {
            self.ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            false
        } else {
            self.quitting = true;
            self.engine.stop();
            self.bot.stop();
            true
        }
    }
}

// helper nhỏ dùng trong mini viewport
struct RichTextText;
impl RichTextText {
    fn new(text: String, size: f32, color: Color32) -> egui::RichText {
        egui::RichText::new(text).size(size).color(color)
    }
}
