// dialog.rs - hộp thoại thêm / sửa trang web (port class SiteDialog của main.py).

use egui::{Align2, RichText, Vec2};
use serde_json::Value;

use crate::app::App;
use crate::config::{buttons_to_pairs, new_site, pairs_to_buttons};
use crate::pages::{button_rows, check_buttons, type_combo};
use crate::theme::Palette;
use crate::util::{label_type, type_label};

pub struct SiteDialog {
    /// Vị trí trong danh sách (None = thêm mới; thêm mới chèn đầu danh sách vì ưu tiên cao nhất).
    pub index: Option<usize>,
    pub v_match: String,
    pub v_name: String,
    pub v_type: String,
    pub v_details: String,
    pub v_state: String,
    pub v_image: String,
    pub v_domain: String,
    pub v_timer: bool,
    pub v_enabled: bool,
    pub v_fav: bool,
    pub v_nobtn: bool,
    pub b_label: [String; 2],
    pub b_url: [String; 2],
    /// Thông báo lỗi hiển thị ngay trong hộp thoại (thay messagebox.showwarning của Tk).
    pub warn: String,
}

impl SiteDialog {
    pub fn new(index: Option<usize>, site: Option<Value>) -> Self {
        let s = site.unwrap_or_else(|| new_site("", "", "watching", true, "", "", "", true, "", true, None, false));
        let pairs = buttons_to_pairs(s.get("buttons"));
        let get = |k: &str| crate::util::as_str(s.get(k));
        let getb = |k: &str, d: bool| if s.get(k).is_none() { d } else { crate::util::truthy(s.get(k)) };
        let mut dlg = Self {
            index,
            v_match: get("match"),
            v_name: get("name"),
            v_type: type_label(&get("type")).to_string(),
            v_details: get("details"),
            v_state: get("state"),
            v_image: get("large_image"),
            v_domain: get("domain"),
            v_timer: getb("timer", true),
            v_enabled: getb("enabled", true),
            v_fav: getb("favicon", true),
            v_nobtn: getb("buttons_off", false),
            b_label: [String::new(), String::new()],
            b_url: [String::new(), String::new()],
            warn: String::new(),
        };
        for i in 0..2 {
            dlg.b_label[i] = pairs.get(i).map(|(l, _)| l.clone()).unwrap_or_default();
            dlg.b_url[i] = pairs.get(i).map(|(_, u)| u.clone()).unwrap_or_default();
        }
        dlg
    }

    fn build_result(&self) -> Value {
        let m = self.v_match.trim();
        let name = {
            let n = self.v_name.trim();
            if n.is_empty() { m } else { n }
        };
        let pairs: Vec<(String, String)> = (0..2)
            .map(|i| (self.b_label[i].trim().to_string(), self.b_url[i].trim().to_string()))
            .collect();
        new_site(
            m,
            name,
            label_type(&self.v_type),
            self.v_enabled,
            self.v_state.trim(),
            self.v_details.trim(),
            self.v_image.trim(),
            self.v_timer,
            &crate::browser::clean_domain(&self.v_domain),
            self.v_fav,
            Some(&pairs_to_buttons(&pairs)),
            self.v_nobtn,
        )
    }

    /// Kiểm tra dữ liệu; trả về thông báo lỗi hoặc None nếu ổn.
    fn validate(&self) -> Option<String> {
        if self.v_match.trim().is_empty() {
            return Some("Hãy nhập từ khoá nhận diện.".to_string());
        }
        check_buttons(&self.b_label, &self.b_url)
    }

    /// Lưu: kiểm tra dữ liệu, ổn thì ghi vào config và trả về true (đóng hộp thoại).
    fn commit_dialog(&mut self, app: &mut App) -> bool {
        if let Some(msg) = self.validate() {
            self.warn = msg;
            return false;
        }
        let site = self.build_result();
        let idx = self.index;
        {
            let mut cfg = app.cfg_lock();
            let mut sites = cfg.sites();
            match idx {
                Some(i) => {
                    if i < sites.len() {
                        sites[i] = site;
                    } else {
                        sites.insert(0, site);
                    }
                }
                None => sites.insert(0, site),
            }
            cfg.set_sites(sites);
        }
        app.commit();
        app.sel_site = idx;
        true
    }
}

/// Vẽ hộp thoại (được gọi từ App::update khi app.dialog.is_some()).
pub fn show(app: &mut App, ctx: &egui::Context, p: &Palette) {
    // Escape đóng hộp thoại (giống bind("<Escape>") của bản Python)
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.dialog = None;
        return;
    }

    let mut open = true;
    let mut save = false;
    let mut cancel = false;
    let title = if app.dialog.as_ref().map(|d| d.index.is_some()).unwrap_or(false) {
        "Sửa trang web"
    } else {
        "Thêm trang web"
    };

    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            if let Some(d) = app.dialog.as_mut() {
                ui.set_max_width(560.0);
                ui.vertical(|ui| {
                    row(ui, p, "Từ khoá nhận diện *", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut d.v_match).desired_width(360.0));
                    });
                    row(ui, p, "Tên hiển thị", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut d.v_name).desired_width(360.0));
                    });
                    ui.horizontal(|ui| {
                        ui.add_sized(Vec2::new(150.0, 20.0), egui::Label::new(RichText::new("Kiểu hoạt động").color(p.muted)));
                        type_combo(ui, "dlg-type", &mut d.v_type);
                    });
                    row(ui, p, "Dòng 1", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut d.v_details).desired_width(360.0));
                    });
                    row(ui, p, "Dòng 2", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut d.v_state).desired_width(360.0));
                    });
                    row(ui, p, "Ảnh lớn (tuỳ chọn)", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut d.v_image).desired_width(360.0));
                    });
                    row(ui, p, "Tên miền (lấy favicon)", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut d.v_domain).desired_width(360.0));
                    });
                    ui.add_space(4.0);
                    ui.checkbox(&mut d.v_timer, "Hiện đồng hồ / thanh thời gian");
                    ui.checkbox(&mut d.v_fav, "Lấy favicon cho trang này");
                    ui.checkbox(&mut d.v_enabled, "Bật trang này");
                    ui.add_space(10.0);
                    ui.label(RichText::new("Nút bấm riêng (trống = dùng nút chung)").color(p.text).size(13.5));
                    ui.separator();
                    button_rows(ui, &mut d.b_label, &mut d.b_url);
                    ui.checkbox(&mut d.v_nobtn, "Không hiện nút nào cho trang này");
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(
                            "Từ khoá: một đoạn chữ trong tiêu đề tab / tên bài / tên miền (không phân biệt hoa thường). \
Dòng 1, 2 để trống = dùng mặc định; nhập dấu - ở dòng 2 để ẩn dòng đó.\n\
Dùng được {title} {artist} {site} {domain} {url}; trong link thêm được {title_q} {artist_q} (đã mã hoá). \
Tên miền để trống = tự lấy từ thanh địa chỉ / từ khoá.",
                        )
                        .color(p.muted)
                        .size(12.0),
                    );
                    if !d.warn.is_empty() {
                        ui.add_space(6.0);
                        ui.label(RichText::new(d.warn.as_str()).color(p.bad).size(12.5));
                    }
                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::pages::accent_button(ui, p, "Lưu", true).clicked() {
                            save = true;
                        }
                        if ui.button("Huỷ").clicked() {
                            cancel = true;
                        }
                    });
                });
            }
        });

    // Enter = lưu (giống bind("<Return>")); Huỷ hoặc ✕ góc cửa sổ = đóng
    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        save = true;
    }
    if !open || cancel {
        app.dialog = None;
        return;
    }
    if save {
        let mut d = match app.dialog.take() {
            Some(d) => d,
            None => return,
        };
        if d.commit_dialog(app) {
            app.dialog = None;
        } else {
            app.dialog = Some(d); // lỗi -> mở lại hộp thoại kèm dòng cảnh báo
        }
    }
}

fn row(ui: &mut egui::Ui, p: &Palette, label: &str, w: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(Vec2::new(150.0, 20.0), egui::Label::new(RichText::new(label).color(p.muted)));
        w(ui);
    });
}
