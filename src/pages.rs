// pages.rs - nội dung các trang (port build_home / build_sites / ... của main.py).

pub mod dialog;

use egui::{Color32, RichText, ScrollArea, Sense, Ui, Vec2};
use serde_json::{json, Value};

use crate::app::{App, QuickField};
use crate::config::{default_sites, LOCAL_KEYS};
use crate::theme::{kind_color, Palette};
use crate::util::{fmt_clock, fmt_dur, label_type, type_label, APP_VERSION};

// ---------------------------------------------------------------- nhật ký update (CHANGELOG)

pub const CHANGELOG: [(&str, &str, &str, &[(u8, &str)]); 9] = [
    ("2.2.0", "30/10/2026", "Rảnh tuỳ chỉnh và tích hợp bot Nino", &[
        (0, "Tab Trạng thái rảnh: tự đặt dòng chữ, ảnh, nút bấm và dùng Client ID của bot / app khác cho trạng thái rảnh."),
        (0, "Khi chuyển sang rảnh, app tự đổi kết nối sang Client ID tuỳ chỉnh; rời rảnh thì quay lại Client ID chính."),
        (0, "Cửa sổ thu nhỏ, khay hệ thống và tab Trạng thái rảnh dùng chung một công tắc “Chuyển sang rảnh”."),
        (0, "Tab Bot Nino: bật / tắt / khởi động lại bot ngay trong app, tự bật bot cùng app, gửi chào sáng / ngủ ngon / thông báo."),
        (0, "Tab Log bot: nhật ký chạy của bot hiện ngay trong app."),
        (2, "“Hiện trạng thái rảnh” và “Dòng chữ khi rảnh” chuyển từ Cài đặt sang tab Trạng thái rảnh."),
        (3, "Trang dashboard HTML của bot (không còn tự mở trình duyệt)."),
    ]),
    ("2.1.0", "26/10/2026", "Chế độ thu nhỏ, tự tách số tập và thông báo nổi", &[
        (0, "Chế độ thu nhỏ: cửa sổ nhỏ luôn nằm trên cùng, chỉ hiện trạng thái đang hiển thị và nút Bật / Tắt."),
        (0, "Tự tách số tập / mùa từ tiêu đề (“Tập 12”, “Season 2 Episode 5”, “S2E5”…) và đưa vào dòng 2."),
        (0, "Thông báo nổi dạng chữ nhỏ trên màn hình khi đổi trạng thái (vd: YouTube → Facebook), có công tắc tắt ở tab Tổng quan."),
        (0, "Biểu tượng app lấy từ file app.ico."),
    ]),
    ("2.0.0", "25/10/2026", "Giao diện mới và ảnh nền tuỳ chỉnh", &[
        (0, "Đổi ảnh nền app bằng ảnh của bạn (mục Giao diện → Chọn ảnh), chỉnh được độ tối và độ mờ."),
        (0, "Chế độ màu Tối / Sáng."),
        (0, "Giờ yên lặng: tự ẩn trạng thái trong khung giờ bạn đặt (vd: 23:00 → 07:00)."),
        (0, "Xuất / nhập cấu hình để sao lưu hoặc chuyển sang máy khác."),
        (0, "Tab Nhật ký update ngay trong app, có nhãn “Mới” khi có bản mới."),
        (0, "Biểu tượng khay hiện trạng thái đang hiển thị khi rê chuột vào."),
        (0, "Tổng quan hiện thời gian đã chạy và số lần cập nhật trạng thái."),
        (0, "App nhớ kích thước cửa sổ lần trước."),
        (2, "Làm lại toàn bộ giao diện: thanh điều hướng bên trái, gom cài đặt thành từng nhóm, bỏ viền rối."),
        (2, "Tab Nhật ký cũ đổi tên thành Hoạt động (tránh nhầm với Nhật ký update), có nút Sao chép."),
        (3, "Trạng thái thủ công và Mẫu trạng thái (ít dùng)."),
        (3, "Tạm ẩn, vì trùng tác dụng với nút Bật / Tắt Presence."),
        (3, "Nút Mở config.json, vì đã có Mở thư mục app."),
        (1, "Nhật ký hoạt động giới hạn 500 dòng, ghi đúng giờ phát sinh sự kiện."),
    ]),
    ("1.5.0", "", "Đồng hồ thông minh và trạng thái rảnh", &[
        (0, "Đồng hồ cộng dồn theo từng web, chỉ reset khi đóng hẳn trình duyệt."),
        (0, "Trạng thái “Đang rảnh” khi không mở web / media."),
        (0, "Tự ẩn trạng thái web khi bạn không thao tác sau N phút."),
        (0, "Không bao giờ lộ tab riêng tư / ẩn danh."),
    ]),
    ("1.4.0", "", "Thông tin chi tiết cho video", &[
        (0, "TikTok: lấy caption, @tác giả và ảnh bìa video qua oEmbed."),
        (0, "YouTube: dùng ảnh thu nhỏ của video làm ảnh lớn."),
        (1, "Không lấy nhầm favicon / tiêu đề của tab khác khi đang nghe nhạc ở tab nền."),
    ]),
    ("1.3.0", "", "Favicon và nút bấm", &[
        (0, "Favicon tự động của trang đang xem (ảnh nhỏ hoặc ảnh lớn)."),
        (0, "Nút bấm dưới thẻ Discord (chung và riêng từng trang), hỗ trợ {title} {artist} {url}…"),
        (0, "Đọc địa chỉ trang từ thanh địa chỉ trình duyệt (cần thư viện uiautomation)."),
    ]),
    ("1.2.0", "", "Tiện ích hệ thống", &[
        (0, "Chặn từ khoá: tab chứa từ khoá sẽ không hiện lên Discord."),
        (0, "Thu xuống khay hệ thống, khởi động cùng Windows."),
        (0, "Trạng thái thủ công và mẫu trạng thái đã lưu."),
    ]),
    ("1.1.0", "", "Danh sách trang web tuỳ chỉnh", &[
        (0, "Thêm / sửa / xoá / sắp xếp các trang web được nhận diện."),
        (0, "Tuỳ chỉnh dòng 1, dòng 2, ảnh lớn và đồng hồ cho từng trang."),
    ]),
    ("1.0.0", "", "Bản đầu tiên", &[
        (0, "Thay thế PreMiD bằng 1 file exe duy nhất."),
        (0, "Đọc nhạc / video đang phát qua Windows Media Session và tiêu đề cửa sổ trình duyệt."),
        (0, "Giao diện Tkinter, kết nối Discord Rich Presence."),
    ]),
];

pub const KIND_LABEL: [(u8, &str); 4] = [(0, "Thêm"), (1, "Sửa"), (2, "Đổi"), (3, "Bỏ")];

// ---------------------------------------------------------------- widget phụ

/// Một nhóm trong trang: tiêu đề + (gợi ý) + khung chứa.
pub fn section<R>(ui: &mut Ui, p: &Palette, title: &str, hint: Option<&str>, body: impl FnOnce(&mut Ui) -> R) {
    ui.vertical(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(title).size(15.0).color(p.text));
        if let Some(h) = hint {
            ui.add_space(3.0);
            ui.label(RichText::new(h).size(12.0).color(p.muted));
        }
        ui.add_space(8.0);
        body(ui);
        ui.add_space(16.0);
    });
}

/// Ô nhập có nhãn đứng trước.
pub fn entry(ui: &mut Ui, label: &str, val: &mut String, width: f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(val).desired_width(width));
    });
}

/// Combobox chọn kiểu hoạt động (chữ tiếng Việt, lưu dạng watching/listening/playing).
pub fn type_combo(ui: &mut Ui, id: &str, sel: &mut String) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(sel.clone())
        .width(130.0)
        .show_ui(ui, |ui| {
            for (k, l) in crate::util::TYPE_LABEL {
                ui.selectable_value(sel, l.to_string(), l);
            }
        });
}

/// Nút nhấn màu nhấn (giống Accent.TButton).
pub fn accent_button(ui: &mut Ui, p: &Palette, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).color(p.on_accent)).fill(p.accent),
    )
}

/// Hai hàng nhập nút bấm (chữ + link); dữ liệu nằm ở label/url arrays của app.
pub fn button_rows(ui: &mut Ui, labels: &mut [String; 2], urls: &mut [String; 2]) {
    ui.horizontal(|ui| {
        ui.add_space(52.0);
        ui.label(RichText::new("Chữ trên nút (≤32 ký tự)").size(12.0).color(p_muted(ui)));
        ui.add_space(60.0);
        ui.label(RichText::new("Link (http:// hoặc https://)").size(12.0).color(p_muted(ui)));
    });
    for i in 0..2 {
        ui.horizontal(|ui| {
            ui.label(format!("Nút {}", i + 1));
            ui.add(egui::TextEdit::singleline(&mut labels[i]).desired_width(150.0));
            ui.add(egui::TextEdit::singleline(&mut urls[i]).desired_width(300.0));
        });
    }
}

fn p_muted(ui: &mut Ui) -> Color32 {
    ui.ctx().style().visuals.widgets.inactive.fg_stroke.color
}

/// Kiểm tra nút nhập tay (port check_buttons); trả về thông báo lỗi hoặc None nếu ổn.
pub fn check_buttons(labels: &[String; 2], urls: &[String; 2]) -> Option<String> {
    for i in 0..2 {
        let label = labels[i].trim();
        let url = urls[i].trim();
        if label.is_empty() && url.is_empty() {
            continue;
        }
        let n = i + 1;
        if label.is_empty() || url.is_empty() {
            return Some(format!("Nút {} cần có cả chữ và link (hoặc để trống cả hai).", n));
        }
        if label.chars().count() > 32 {
            return Some(format!("Chữ trên nút {} dài quá 32 ký tự.", n));
        }
        if url.chars().count() > 512 {
            return Some(format!("Link của nút {} dài quá 512 ký tự.", n));
        }
        let lower = url.to_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with('{')) {
            return Some(format!("Link của nút {} phải bắt đầu bằng http:// hoặc https://.", n));
        }
    }
    None
}

/// Hàng đáy trang có nút Lưu + dòng báo nhỏ (flash).
pub fn save_bar(app: &mut App, ui: &mut Ui, p: &Palette, key: &str, text: &str, save: impl FnOnce(&mut App)) {
    ui.separator();
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if accent_button(ui, p, text, true).clicked() {
            save(app);
        }
        if let Some((t, bad)) = app.flash_text(key) {
            ui.label(RichText::new(t).color(if bad { p.bad } else { p.ok }));
        }
    });
}

/// Câu hỏi Yes/No của hệ thống (thay messagebox.askyesno).
pub fn ask(title: &str, text: &str) -> bool {
    use rfd::MessageDialog;
    use rfd::MessageButtons;
    use rfd::MessageDialogResult;
    MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}

// ---------------------------------------------------------------- điều hướng trang

pub fn show_page(app: &mut App, ui: &mut Ui, p: &Palette) {
    let key = app.cur;
    match key {
        "home" => home(app, ui, p),
        "sites" => sites(app, ui, p),
        "visual" => visual(app, ui, p),
        "block" => block(app, ui, p),
        "idle" => idle(app, ui, p),
        "look" => look(app, ui, p),
        "settings" => settings(app, ui, p),
        "bot" => bot(app, ui, p),
        "botlog" => botlog(app, ui, p),
        "log" => log(app, ui, p),
        "changelog" => changelog(app, ui, p),
        _ => {}
    }
}

// ---------------------------------------------------------------- Tổng quan

fn home(app: &mut App, ui: &mut Ui, p: &Palette) {
    let running = app.engine.running();
    let cur = if running { app.engine.current() } else { None };

    // thẻ hoạt động: giống thẻ người khác thấy trên hồ sơ Discord của bạn
    egui::Frame::none()
        .fill(p.field)
        .rounding(10.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // ô ảnh (favicon / thumbnail / ô giữ chỗ)
                let icon_tex = hero_icon(app, ui, cur.as_ref(), p);
                if let Some(tex) = icon_tex {
                    let rect = egui::Rect::from_min_size(ui.cursor().min, Vec2::splat(76.0));
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(76.0), Sense::hover());
                    ui.painter().image(
                        tex.id(),
                        r.rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(app.pv_type.as_str()).color(kind_color(p, &app.pv_type)).size(13.5));
                    ui.add_space(3.0);
                    ui.label(RichText::new(app.pv_details.as_str()).size(17.0).color(p.text));
                    if !app.pv_state.is_empty() {
                        ui.label(RichText::new(app.pv_state.as_str()).size(12.5).color(p.muted));
                    }
                    // hàng đồng hồ
                    if let Some(a) = &cur {
                        if let Some(start) = a.get("start").and_then(Value::as_i64) {
                            let now = crate::util::now() as i64;
                            let end = a.get("end").and_then(Value::as_i64);
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(fmt_clock(now - start)).size(13.0).color(p.muted));
                                match end {
                                    Some(e) if e > start => {
                                        ui.label(
                                            RichText::new(format!("/ {}", fmt_clock(e - start)))
                                                .size(11.0)
                                                .color(p.muted),
                                        );
                                    }
                                    _ => {
                                        ui.label(RichText::new("đã trôi qua").size(11.0).color(p.muted));
                                    }
                                }
                            });
                            if let Some(e) = end {
                                if e > start {
                                    // thanh tiến độ
                                    let (resp, rect) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 10.0), Sense::hover());
                                    let frac = ((now - start) as f32 / (e - start) as f32).clamp(0.0, 1.0);
                                    let inner = egui::Rect::from_min_max(
                                        egui::Pos2::new(rect.min.x, rect.center().y - 3.0),
                                        egui::Pos2::new(rect.max.x, rect.center().y + 3.0),
                                    );
                                    ui.painter().rect_filled(inner.shrink2(Vec2::new(3.0, 0.0)), 3.0, p.line);
                                    if frac > 0.01 {
                                        let mut fill = inner;
                                        fill.max.x = fill.min.x + (fill.width() - 6.0) * frac + 6.0;
                                        ui.painter()
                                            .rect_filled(fill.shrink2(Vec2::new(3.0, 0.0)), 3.0, kind_color(p, &app.pv_type));
                                    }
                                    let _ = resp;
                                }
                            }
                        }
                    }
                });
            });
            // nút bấm của activity
            let btns = cur
                .as_ref()
                .and_then(|a| a.get("buttons"))
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .map(|b| {
                            (
                                b.get("label").and_then(Value::as_str).unwrap_or("").to_string(),
                                b.get("url").and_then(Value::as_str).unwrap_or("").to_string(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !btns.is_empty() {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    for (label, url) in btns {
                        if ui.button(label).clicked() && !url.is_empty() {
                            let _ = webbrowser::open(&url);
                        }
                    }
                });
            }
        });
    ui.add_space(20.0);

    // hai cột công tắc
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            section(ui, p, "Nguồn hiển thị", Some("Chọn thứ được phép hiện lên Discord."), |ui| {
                if ui.checkbox(&mut app.c_media, "Nhạc / video đang phát").changed() {
                    app.quick_set(QuickField::Media, app.c_media);
                }
                if ui.checkbox(&mut app.c_browse, "Trang web đang duyệt").changed() {
                    app.quick_set(QuickField::Browse, app.c_browse);
                }
            });
            section(
                ui,
                p,
                "Kiểu hiển thị web khác",
                Some("Áp dụng cho trang web không nằm trong tab Trang web. Trang đã khai báo vẫn dùng kiểu riêng của nó."),
                |ui| {
                    type_combo(ui, "webtype", &mut app.i_webtype);
                },
            );
            // cập nhật webtype khi đổi (so sánh với config)
            let code = label_type(&app.i_webtype);
            {
                let mut cfg = app.cfg_lock();
                if cfg.get_str("web_default_type") != code {
                    cfg.set_str("web_default_type", code);
                    drop(cfg);
                    app.commit();
                }
            }
        });
        cols[1].vertical(|ui| {
            section(
                ui,
                p,
                "Thông báo",
                Some("Dòng chữ nhỏ nổi trên màn hình mỗi khi nguồn hiển thị đổi (vd: YouTube → Facebook). Không phải thông báo của Windows."),
                |ui| {
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut app.c_toast, "Hiện khi đổi trạng thái").changed() {
                            app.quick_set(QuickField::Toast, app.c_toast);
                        }
                        if ui.button("Xem thử").clicked() {
                            app.show_toast("YouTube  →  Facebook".to_string());
                        }
                    });
                },
            );
        });
    });

    // dòng thống kê + nút tiện ích
    if running {
        ui.label(RichText::new(format!(
            "Đã chạy {} · {} lần cập nhật trạng thái",
            fmt_dur((crate::util::now() - app.engine.started_at()) as i64),
            app.engine.updates()
        ))
        .color(p.muted));
    } else {
        ui.label(RichText::new("Presence đang tắt").color(p.muted));
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Mở thư mục app").clicked() {
            crate::win::startfile(crate::config::base_dir());
        }
        if ui.button("Thu xuống khay").clicked() {
            app.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if ui.button("Chế độ thu nhỏ").clicked() {
            app.open_mini();
        }
    });
}

/// Ảnh trong thẻ xem trước: favicon/thumbnail đã tải, hoặc ô giữ chỗ theo loại hoạt động.
fn hero_icon(app: &mut App, ui: &mut Ui, cur: Option<&Value>, p: &Palette) -> Option<egui::TextureHandle> {
    let img_url: Option<String> = cur.and_then(|a| {
        let big = a.get("large_image").and_then(Value::as_str).unwrap_or("");
        a.get("small_image")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| if big.starts_with("http") { Some(big.to_string()) } else { None })
    });
    if let Some(url) = img_url {
        crate::icons::IconCache::global().request(&url);
        if let Some(bytes) = crate::icons::IconCache::global().loaded(&url) {
            if let Some(tex) = app.hero_icons.get(&url) {
                return Some(tex.clone());
            }
            if let Some(tex) = crate::app::tex_from_bytes(ui.ctx(), &format!("hero-{}", url), &bytes, 76, 76) {
                app.hero_icons.insert(url, tex.clone());
                return Some(tex);
            }
        }
        return None;
    }
    // ô giữ chỗ theo loại
    let key = crate::theme::kind_key(&app.pv_type).unwrap_or("off").to_string();
    let field = [p.field.r(), p.field.g(), p.field.b()];
    if let Some((k, f, tex)) = &app.hero_kind_tex {
        if k == &key && f == &field {
            return Some(tex.clone());
        }
    }
    let tile = crate::bg::kind_tile(p, Some(key.as_str()), 76);
    let ci = egui::ColorImage::from_rgb([76, 76], &crate::bg::rgb_to_rgba(&tile));
    let tex = ui.ctx().load_texture("hero-kind", ci, egui::TextureOptions::LINEAR);
    app.hero_kind_tex = Some((key, field, tex.clone()));
    Some(tex)
}

// ---------------------------------------------------------------- Trang web

fn sites(app: &mut App, ui: &mut Ui, p: &Palette) {
    ui.horizontal(|ui| {
        if accent_button(ui, p, "Thêm", true).clicked() {
            app.sel_site = None;
            app.dialog = Some(dialog::SiteDialog::new(None, None));
        }
        if ui.add_enabled(app.sel_site.is_some(), egui::Button::new("Sửa")).clicked() {
            open_edit_dialog(app);
        }
        if ui.add_enabled(app.sel_site.is_some(), egui::Button::new("Bật / tắt")).clicked() {
            toggle_site(app);
        }
        if ui.add_enabled(app.sel_site.is_some(), egui::Button::new("Xoá")).clicked() {
            delete_site(app);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if accent_button(ui, p, "Khôi phục", true).clicked() && ask(
                "Khôi phục",
                "Đưa danh sách về mặc định? Các trang bạn tự thêm sẽ bị xoá.",
            ) {
                app.cfg_lock().set_sites(default_sites());
                app.commit();
                app.sel_site = None;
            }
            if ui.add_enabled(app.sel_site.is_some(), egui::Button::new("▼")).clicked() {
                move_site(app, 1);
            }
            if ui.add_enabled(app.sel_site.is_some(), egui::Button::new("▲")).clicked() {
                move_site(app, -1);
            }
        });
    });
    ui.add_space(10.0);

    egui::Frame::none()
        .fill(p.field)
        .rounding(6.0)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            let sites = app.cfg_lock().sites();
            for (i, s) in sites.iter().enumerate() {
                let name = crate::util::as_str(s.get("name"));
                let m = crate::util::as_str(s.get("match"));
                let kind = type_label(&crate::util::as_str(s.get("type")));
                let domain = crate::util::as_str(s.get("domain"));
                let state = crate::util::as_str(s.get("state"));
                let on = crate::util::truthy(s.get("enabled"));
                let line = format!(
                    "{}  {:<18} {:<22} {:<12} {:<16} {}",
                    if on { "✔" } else { "—" },
                    name,
                    m,
                    kind,
                    domain,
                    state
                );
                let selected = app.sel_site == Some(i);
                let resp = ui.selectable_label(selected, line);
                if resp.clicked() {
                    app.sel_site = Some(i);
                }
                if resp.double_clicked() {
                    app.sel_site = Some(i);
                    open_edit_dialog(app);
                }
            }
        });
}

fn sites_mut(app: &mut App, f: impl FnOnce(&mut Vec<Value>)) {
    let mut sites = app.cfg_lock().sites();
    f(&mut sites);
    app.cfg_lock().set_sites(sites);
}

fn open_edit_dialog(app: &mut App) {
    let i = match app.sel_site {
        Some(i) => i,
        None => return,
    };
    let site = app.cfg_lock().sites().get(i).cloned();
    if let Some(s) = site {
        app.dialog = Some(dialog::SiteDialog::new(Some(i), Some(s)));
    }
}

fn toggle_site(app: &mut App) {
    let i = match app.sel_site {
        Some(i) => i,
        None => return,
    };
    sites_mut(app, |sites| {
        if let Some(s) = sites.get_mut(i) {
            let cur = crate::util::truthy(s.get("enabled"));
            s["enabled"] = json!(!cur);
        }
    });
    app.commit();
}

fn delete_site(app: &mut App) {
    let i = match app.sel_site {
        Some(i) => i,
        None => return,
    };
    let name = app
        .cfg_lock()
        .sites()
        .get(i)
        .map(|s| crate::util::as_str(s.get("name")))
        .unwrap_or_default();
    if ask("Xoá", &format!("Xoá “{}”?", name)) {
        sites_mut(app, |sites| {
            if i < sites.len() {
                sites.remove(i);
            }
        });
        app.commit();
        app.sel_site = None;
    }
}

fn move_site(app: &mut App, d: i64) {
    let i = match app.sel_site {
        Some(i) => i as i64,
        None => return,
    };
    let j = i + d;
    let n = app.cfg_lock().sites().len() as i64;
    if 0 <= j && j < n {
        sites_mut(app, |sites| sites.swap(i as usize, j as usize));
        app.commit();
        app.sel_site = Some(j as usize);
    }
}

// ---------------------------------------------------------------- Favicon & nút

fn visual(app: &mut App, ui: &mut Ui, p: &Palette) {
    section(
        ui,
        p,
        "Favicon tự động",
        Some("App lấy favicon của trang đang xem và gửi lên Discord làm ảnh. Tên miền được xác định theo thứ tự: ô Tên miền của trang → thanh địa chỉ trình duyệt → từ khoá / danh sách có sẵn."),
        |ui| {
            ui.horizontal(|ui| {
                radio(ui, &mut app.i_fmode, "off", "Tắt");
                radio(ui, &mut app.i_fmode, "small", "Biểu tượng nhỏ (góc ảnh lớn)");
                radio(ui, &mut app.i_fmode, "large", "Dùng làm ảnh lớn");
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Nguồn favicon");
                ui.add(egui::TextEdit::singleline(&mut app.i_ftpl).desired_width(360.0));
                if ui.button("Mặc định").clicked() {
                    app.i_ftpl = crate::config::DEFAULT_FAVICON_TPL.to_string();
                }
            });
            ui.checkbox(&mut app.c_addr, "Tự đọc địa chỉ trang từ thanh địa chỉ trình duyệt (thử nghiệm)");
            ui.label(
                RichText::new("Đọc địa chỉ: sẵn sàng (uiautomation)")
                    .color(p.ok)
                    .size(12.0),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut app.i_tdom).desired_width(160.0));
                if ui.button("Thử lấy favicon").clicked() {
                    let d = crate::browser::clean_domain(&app.i_tdom);
                    if !d.is_empty() {
                        let url = crate::activity::favicon_url(&app.i_ftpl.trim().to_string(), &d);
                        app.test_txt = "Đang tải…".to_string();
                        app.test_tex = None;
                        app.bg_fetch_favicon(url);
                    }
                }
                if ui.button("Thử đọc thanh địa chỉ").clicked() {
                    app.test_txt = "Đang đọc…".to_string();
                    app.test_tex = None;
                    app.bg_read_address();
                }
            });
            ui.horizontal(|ui| {
                if let Some(tex) = &app.test_tex {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                    ui.painter().image(
                        tex.id(),
                        r.rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    ui.add_space(6.0);
                }
                ui.label(RichText::new(app.test_txt.as_str()).color(p.muted));
            });
        },
    );

    section(
        ui,
        p,
        "Nút bấm dưới thẻ (tối đa 2)",
        Some("Nút chung áp dụng cho mọi trạng thái, trừ khi trang có nút riêng. Trong chữ nút dùng được {title} {artist} {site} {domain}. Trong link dùng được {url} (địa chỉ trang đang mở), {domain}, {title_q}, {artist_q} (đã mã hoá, để ghép vào link tìm kiếm). Nút nào có {url} mà app không đọc được địa chỉ thì tự ẩn.\nLưu ý: Discord không hiện nút trên chính máy bạn, hãy nhìn từ tài khoản khác."),
        |ui| {
            ui.checkbox(&mut app.c_btn_on, "Hiện nút bấm");
            ui.add_space(6.0);
            button_rows(ui, &mut app.gb_label, &mut app.gb_url);
        },
    );

    save_bar(app, ui, p, "visual", "Lưu thay đổi", |app| {
        let btns_bad = check_buttons(&app.gb_label, &app.gb_url);
        if let Some(msg) = btns_bad {
            app.flash_bad("visual", &msg);
            return;
        }
        let tpl = app.i_ftpl.trim().to_string();
        let tpl = if tpl.is_empty() { crate::config::DEFAULT_FAVICON_TPL.to_string() } else { tpl };
        if !tpl.contains("{domain}") {
            app.flash_bad("visual", "Link nguồn favicon phải chứa {domain}.");
            return;
        }
        let pairs: Vec<(String, String)> = (0..2).map(|i| (app.gb_label[i].trim().to_string(), app.gb_url[i].trim().to_string())).collect();
        {
            let mut cfg = app.cfg_lock();
            cfg.set_str("favicon_mode", &app.i_fmode);
            cfg.set_str("favicon_url_template", &tpl);
            cfg.set_bool("read_address_bar", app.c_addr);
            cfg.set_bool("buttons_enabled", app.c_btn_on);
            cfg.set("buttons", crate::config::pairs_to_buttons(&pairs));
        }
        app.commit();
        app.flash("visual", "Đã lưu");
    });
}

fn radio(ui: &mut Ui, sel: &mut String, value: &str, text: &str) {
    if ui
        .selectable_label(sel == value, RichText::new(if sel == value { format!("● {}", text) } else { format!("○ {}", text) }))
        .clicked()
    {
        *sel = value.to_string();
    }
    ui.add_space(4.0);
}

// ---------------------------------------------------------------- Chặn từ khoá

fn block(app: &mut App, ui: &mut Ui, p: &Palette) {
    ui.horizontal(|ui| {
        let add = ui.add(egui::TextEdit::singleline(&mut app.v_kw).desired_width(300.0));
        if add.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            add_kw(app);
        }
        if accent_button(ui, p, "Thêm", true).clicked() {
            add_kw(app);
        }
        if ui.add_enabled(app.sel_kw.is_some(), egui::Button::new("Xoá mục chọn")).clicked() {
            del_kw(app);
        }
    });
    ui.label(RichText::new("Ví dụ: bank, mail, password. Không phân biệt hoa thường.").color(p.muted).size(12.0));
    ui.add_space(8.0);

    egui::Frame::none()
        .fill(p.field)
        .rounding(6.0)
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            let kws = app.cfg_lock().blacklist();
            for (i, k) in kws.iter().enumerate() {
                let selected = app.sel_kw == Some(i);
                if ui.selectable_label(selected, k.as_str()).clicked() {
                    app.sel_kw = Some(i);
                }
            }
        });
}

fn add_kw(app: &mut App) {
    let k = app.v_kw.trim().to_string();
    if !k.is_empty() {
        let kws = app.cfg_lock().blacklist();
        if !kws.iter().any(|x| x.eq_ignore_ascii_case(&k)) {
            drop(kws);
            sites_mut_kw(app, |kws| kws.push(k.clone()));
            app.commit();
        }
    }
    app.v_kw.clear();
}

fn sites_mut_kw(app: &mut App, f: impl FnOnce(&mut Vec<String>)) {
    let mut kws = app.cfg_lock().blacklist();
    f(&mut kws);
    app.cfg_lock().set("blacklist_keywords", json!(kws));
}

fn del_kw(app: &mut App) {
    let i = match app.sel_kw {
        Some(i) => i,
        None => return,
    };
    sites_mut_kw(app, |kws| {
        if i < kws.len() {
            kws.remove(i);
        }
    });
    app.commit();
    app.sel_kw = None;
}

// ---------------------------------------------------------------- Trạng thái rảnh

fn idle(app: &mut App, ui: &mut Ui, p: &Palette) {
    // dòng trạng thái động (port update_extras)
    let live = {
        let running = app.engine.running();
        let cur = if running { app.engine.current() } else { None };
        if !running {
            "Presence đang tắt".to_string()
        } else if let Some(a) = &cur {
            if crate::util::truthy(a.get("_idle")) {
                if a.get("_cid").is_some() {
                    "Đang hiển thị trên Discord: rảnh tuỳ chỉnh (Client ID khác)".to_string()
                } else {
                    "Đang hiển thị trên Discord: rảnh mặc định".to_string()
                }
            } else {
                "Đang hiển thị trên Discord: nguồn khác, chưa phải trạng thái rảnh".to_string()
            }
        } else {
            "Chưa có gì đang hiển thị trên Discord".to_string()
        }
    };
    // thẻ xem trước: đổi theo những gì đang nhập (chưa cần bấm Lưu)
    egui::Frame::none()
        .fill(p.field)
        .rounding(10.0)
        .inner_margin(egui::Margin::same(18.0))
        .show(ui, |ui| {
            ui.label(RichText::new("Xem trước (chưa lưu)").color(p.muted).size(12.0));
            ui.add_space(8.0);
            let cid = app.ci_cid.trim().to_string();
            if app.ci_on && cid.chars().all(|c| c.is_ascii_digit()) && !cid.is_empty() {
                ui.label(RichText::new(format!("{} · rảnh tuỳ chỉnh", type_label(&label_type(&app.ci_type))))
                    .color(p.idle));
                ui.add_space(4.0);
                ui.label(RichText::new(if app.ci_d.trim().is_empty() { "(chưa nhập dòng 1)" } else { app.ci_d.trim() }).size(16.0));
                if !app.ci_s.trim().is_empty() {
                    ui.label(RichText::new(app.ci_s.trim()).size(12.5).color(p.text));
                }
                ui.add_space(8.0);
                ui.label(RichText::new(format!("Hiện bằng Client ID {}", cid)).color(p.muted).size(12.0));
            } else {
                ui.label(RichText::new("Đang nghe · rảnh mặc định").color(p.idle));
                ui.add_space(4.0);
                ui.label(RichText::new(if app.s_idle_txt.trim().is_empty() { "Đang rảnh" } else { app.s_idle_txt.trim() }).size(16.0));
                ui.label(RichText::new(">.<").size(12.5).color(p.text));
                ui.add_space(8.0);
                let note = if app.ci_on { " (Client ID tuỳ chỉnh chưa hợp lệ nên chưa dùng)" } else { "" };
                ui.label(RichText::new(format!("Hiện bằng Client ID chính{}", note)).color(p.muted).size(12.0));
            }
            ui.add_space(4.0);
            ui.label(RichText::new(live.as_str()).color(p.muted).size(12.0));
        });
    ui.add_space(24.0);

    section(
        ui,
        p,
        "Công tắc nhanh",
        Some("Dùng chung với nút ở cửa sổ thu nhỏ và menu khay. Bật thì app bỏ qua nhạc / web và chỉ hiện trạng thái rảnh (áp dụng ở lần cập nhật kế tiếp)."),
        |ui| {
            if ui.checkbox(&mut app.c_force, "Chuyển sang rảnh ngay").changed() {
                crate::activity::FORCE_IDLE.store(app.c_force, std::sync::atomic::Ordering::Relaxed);
            }
            ui.checkbox(&mut app.c_show_idle, "Hiện trạng thái rảnh khi không mở web / media");
        },
    );

    section(
        ui,
        p,
        "Rảnh mặc định",
        Some("Dùng Client ID chính ở tab Cài đặt. Áp dụng khi rảnh tuỳ chỉnh đang tắt."),
        |ui| {
            entry(ui, "Dòng chữ khi rảnh", &mut app.s_idle_txt, 300.0);
        },
    );

    section(
        ui,
        p,
        "Rảnh tuỳ chỉnh (Client ID khác)",
        Some("Dùng Application ID của một bot / app Discord khác (lấy ở Developer Portal): tên app đó sẽ hiện trên thẻ, vd “Đang nghe <tên app>”. Ảnh lớn phải là tên ảnh đã tải lên mục Rich Presence → Art Assets của app đó (hoặc một link ảnh). Khi rảnh, app tự đổi sang ID này; hết rảnh thì quay lại Client ID chính."),
        |ui| {
            ui.checkbox(&mut app.ci_on, "Bật rảnh tuỳ chỉnh");
            ui.add_space(6.0);
            entry(ui, "Client ID (Application ID)", &mut app.ci_cid, 280.0);
            ui.horizontal(|ui| {
                ui.label("Kiểu hiển thị");
                type_combo(ui, "ci-type", &mut app.ci_type);
                ui.add_space(16.0);
                ui.label("Dòng 1");
                ui.add(egui::TextEdit::singleline(&mut app.ci_d).desired_width(320.0));
            });
            ui.horizontal(|ui| {
                ui.label("Dòng 2 (trống = ẩn)");
                ui.add(egui::TextEdit::singleline(&mut app.ci_s).desired_width(320.0));
            });
            ui.horizontal(|ui| {
                ui.label("Ảnh lớn (tên ảnh hoặc link)");
                ui.add(egui::TextEdit::singleline(&mut app.ci_img).desired_width(280.0));
            });
            ui.horizontal(|ui| {
                ui.label("Chữ khi rê vào ảnh");
                ui.add(egui::TextEdit::singleline(&mut app.ci_lt).desired_width(280.0));
            });
            ui.checkbox(&mut app.ci_timer, "Hiện đồng hồ đếm thời gian đã rảnh");
        },
    );

    section(
        ui,
        p,
        "Nút bấm của rảnh tuỳ chỉnh",
        Some("Để trống cả hai thì dùng nút chung (Favicon & nút). Tối đa 2 nút."),
        |ui| {
            button_rows(ui, &mut app.ci_label, &mut app.ci_url);
        },
    );

    save_bar(app, ui, p, "idle", "Lưu trạng thái rảnh", |app| {
        let cid = app.ci_cid.trim().to_string();
        if !cid.is_empty() && !cid.chars().all(|c| c.is_ascii_digit()) {
            app.flash_bad("idle", "Client ID chỉ gồm chữ số.");
            return;
        }
        if app.ci_on && cid.is_empty() {
            app.flash_bad("idle", "Hãy nhập Client ID của bot / app khác (hoặc tắt rảnh tuỳ chỉnh).");
            return;
        }
        if app.ci_on && app.ci_d.trim().is_empty() {
            app.flash_bad("idle", "Dòng 1 không được để trống.");
            return;
        }
        if let Some(msg) = check_buttons(&app.ci_label, &app.ci_url) {
            app.flash_bad("idle", &msg);
            return;
        }
        let pairs: Vec<(String, String)> = (0..2).map(|i| (app.ci_label[i].trim().to_string(), app.ci_url[i].trim().to_string())).collect();
        {
            let mut cfg = app.cfg_lock();
            cfg.set_bool("show_idle", app.c_show_idle);
            cfg.set_str("idle_details", if app.s_idle_txt.trim().is_empty() { "Đang rảnh" } else { app.s_idle_txt.trim() });
            cfg.set_bool("idle_custom_enabled", app.ci_on);
            cfg.set_str("idle_custom_client_id", &cid);
            cfg.set_str("idle_custom_type", label_type(&app.ci_type));
            cfg.set_str("idle_custom_details", app.ci_d.trim());
            cfg.set_str("idle_custom_state", app.ci_s.trim());
            cfg.set_str("idle_custom_large_image", app.ci_img.trim());
            cfg.set_str("idle_custom_large_text", app.ci_lt.trim());
            cfg.set_bool("idle_custom_timer", app.ci_timer);
            cfg.set("idle_custom_buttons", crate::config::pairs_to_buttons(&pairs));
        }
        app.commit();
        app.flash("idle", "Đã lưu");
    });
}

// ---------------------------------------------------------------- Giao diện

fn look(app: &mut App, ui: &mut Ui, p: &Palette) {
    section(
        ui,
        p,
        "Ảnh nền",
        Some("Ảnh được cắt vừa khung cửa sổ và sao chép vào thư mục app, nên bạn có thể xoá file gốc. Thanh bên trái là lớp kính mờ nên chữ luôn dễ đọc, dù ảnh sáng hay rối."),
        |ui| {
            ui.horizontal(|ui| {
                if let Some(tex) = app.bg_thumb(ui.ctx(), p) {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(224.0, 126.0), Sense::hover());
                    ui.painter().image(
                        tex.id(),
                        r.rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(app.bg_txt.as_str()).color(p.text));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if accent_button(ui, p, "Chọn ảnh…", true).clicked() {
                            app.pick_bg();
                        }
                        let custom = !app.cfg_lock().get_str("bg_image").is_empty();
                        if ui.add_enabled(custom, egui::Button::new("Dùng nền mặc định")).clicked() {
                            app.reset_bg();
                        }
                    });
                });
            });
        },
    );

    section(
        ui,
        p,
        "Độ dễ đọc",
        Some("Tăng độ tối hoặc làm mờ nếu ảnh nền quá chói hoặc quá rối."),
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Độ tối");
                if ui.add(egui::Slider::new(&mut app.dim, 0.0..=85.0).show_value(false)).changed() {
                    app.cfg_lock().set("bg_dim", json!(app.dim.round() as i64));
                    app.commit();
                }
                ui.label(RichText::new(format!("{}%", app.dim.round() as i64)).color(p.muted).size(12.0));
            });
            ui.horizontal(|ui| {
                ui.label("Làm mờ");
                if ui.add(egui::Slider::new(&mut app.blur, 0.0..=20.0).show_value(false)).changed() {
                    app.cfg_lock().set("bg_blur", json!(app.blur.round() as i64));
                    app.commit();
                }
                ui.label(RichText::new(format!("{}", app.blur.round() as i64)).color(p.muted).size(12.0));
            });
        },
    );

    section(
        ui,
        p,
        "Chế độ màu",
        Some("Áp dụng cho khung nội dung bên phải. Thanh bên trái luôn tối."),
        |ui| {
            let before = app.i_theme.clone();
            ui.horizontal(|ui| {
                radio(ui, &mut app.i_theme, "dark", "Tối");
                radio(ui, &mut app.i_theme, "light", "Sáng");
            });
            if app.i_theme != before {
                app.cfg_lock().set_str("theme", &app.i_theme);
                app.commit();
                let np = crate::theme::palette(&app.i_theme);
                crate::theme::apply_style(&app.ctx, &np);
            }
        },
    );
}

// ---------------------------------------------------------------- Cài đặt

fn settings(app: &mut App, ui: &mut Ui, p: &Palette) {
    section(ui, p, "Kết nối", None, |ui| {
        entry(ui, "Client ID (Application ID)", &mut app.i_cid, 280.0);
        entry(ui, "Chu kỳ cập nhật (giây)", &mut app.i_interval, 80.0);
    });

    section(ui, p, "Nội dung hiển thị", None, |ui| {
        entry(ui, "Tên ảnh lớn mặc định", &mut app.i_img, 200.0);
        entry(ui, "Dòng 2 khi duyệt web (trống = ẩn)", &mut app.i_bstate, 300.0);
        ui.checkbox(&mut app.c_hover, "Hiện dòng chữ dưới cùng khi rê vào ảnh (tên nguồn, vd: Media, YouTube)");
        ui.checkbox(&mut app.c_focus, "Chỉ hiện web khi cửa sổ trình duyệt đang được focus");
    });

    section(
        ui,
        p,
        "Tự ẩn trạng thái",
        Some("Giờ yên lặng ẩn mọi trạng thái trong khung giờ bạn đặt, kể cả khung qua nửa đêm (vd: 23:00 đến 07:00)."),
        |ui| {
            entry(ui, "Ẩn web khi không thao tác (phút, 0 = tắt)", &mut app.i_idle_min, 80.0);
            ui.checkbox(&mut app.c_quiet, "Bật giờ yên lặng");
            ui.horizontal(|ui| {
                ui.label("Từ");
                ui.add(egui::TextEdit::singleline(&mut app.i_qfrom).desired_width(60.0));
                ui.label("đến");
                ui.add(egui::TextEdit::singleline(&mut app.i_qto).desired_width(60.0));
                ui.label(RichText::new("(giờ:phút, vd 23:00)").color(p.muted).size(12.0));
            });
        },
    );

    section(ui, p, "Ứng dụng", None, |ui| {
        ui.checkbox(&mut app.c_auto, "Tự bật Presence khi mở app");
        ui.checkbox(&mut app.c_tray, "Bấm X thì thu xuống khay (không thoát hẳn)");
        ui.checkbox(&mut app.c_win, "Khởi động cùng Windows (mở sẵn dưới khay)");
    });

    section(
        ui,
        p,
        "Sao lưu cấu hình",
        Some("Xuất toàn bộ cài đặt (trang web, nút, từ khoá chặn…) ra file để sao lưu hoặc chuyển sang máy khác. Ảnh nền và kích thước cửa sổ không nằm trong file."),
        |ui| {
            ui.horizontal(|ui| {
                if ui.button("Xuất cấu hình…").clicked() {
                    export_cfg(app);
                }
                if ui.button("Nhập cấu hình…").clicked() {
                    import_cfg(app);
                }
            });
        },
    );

    save_bar(app, ui, p, "settings", "Lưu cài đặt", |app| {
        let interval = app.i_interval.trim().replace(',', ".");
        let interval = match interval.parse::<f64>() {
            Ok(v) if v >= 1.0 => v,
            _ => {
                app.flash_bad("settings", "Chu kỳ cập nhật phải là số từ 1 trở lên.");
                return;
            }
        };
        let idle_min = match app.i_idle_min.trim().parse::<i64>() {
            Ok(v) if v >= 0 => v,
            _ => {
                app.flash_bad("settings", "Số phút không thao tác phải là số nguyên từ 0 trở lên.");
                return;
            }
        };
        let cid = app.i_cid.trim().to_string();
        if !cid.is_empty() && !cid.chars().all(|c| c.is_ascii_digit()) {
            app.flash_bad("settings", "Client ID chỉ gồm chữ số.");
            return;
        }
        let qf = crate::config::parse_hhmm(&app.i_qfrom);
        let qt = crate::config::parse_hhmm(&app.i_qto);
        if app.c_quiet && (qf.is_none() || qt.is_none() || qf == qt) {
            app.flash_bad("settings", "Hãy nhập giờ bắt đầu và kết thúc khác nhau, dạng giờ:phút (vd 23:00 và 07:00).");
            return;
        }
        let fmt = |m: Option<i64>, old: &str| -> String {
            match m {
                Some(m) => format!("{:02}:{:02}", m / 60, m % 60),
                None => old.to_string(),
            }
        };
        let qfrom_new;
        let qto_new;
        {
            let mut cfg = app.cfg_lock();
            qfrom_new = fmt(qf, &cfg.get_str("quiet_from"));
            qto_new = fmt(qt, &cfg.get_str("quiet_to"));
            cfg.set_str("client_id", &cid);
            cfg.set("interval_seconds", json!(interval));
            cfg.set("idle_minutes", json!(idle_min));
            cfg.set_str("large_image", if app.i_img.trim().is_empty() { "logo" } else { app.i_img.trim() });
            cfg.set_str("browsing_state", app.i_bstate.trim());
            cfg.set_bool("show_large_text", app.c_hover);
            cfg.set_bool("browsing_requires_focus", app.c_focus);
            cfg.set_bool("auto_start_presence", app.c_auto);
            cfg.set_bool("minimize_to_tray", app.c_tray);
            cfg.set_bool("start_with_windows", app.c_win);
            cfg.set_bool("quiet_enabled", app.c_quiet);
            cfg.set_str("quiet_from", &qfrom_new);
            cfg.set_str("quiet_to", &qto_new);
        }
        app.i_qfrom = qfrom_new;
        app.i_qto = qto_new;
        if let Err(e) = crate::win::set_autostart(app.c_win) {
            app.flash_bad("settings", &format!("Khởi động cùng Windows: không đặt được: {}", e));
        }
        app.commit();
        app.flash("settings", "Đã lưu");
    });
}

fn export_cfg(app: &mut App) {
    use rfd::FileDialog;
    let path = FileDialog::new()
        .set_title("Xuất cấu hình")
        .add_filter("JSON", &["json"])
        .set_file_name("discord-presence-config.json")
        .save_file();
    let path = match path {
        Some(p) => p,
        None => return,
    };
    let data: Value = {
        let cfg = app.cfg_lock();
        let mut m = serde_json::Map::new();
        for (k, v) in cfg.0.iter() {
            if k.starts_with('_') || LOCAL_KEYS.contains(&k.as_str()) {
                continue;
            }
            m.insert(k.clone(), v.clone());
        }
        Value::Object(m)
    };
    match serde_json::to_string_pretty(&data) {
        Ok(text) => {
            if std::fs::write(&path, text).is_ok() {
                app.flash("settings", "Đã xuất cấu hình");
            } else {
                app.flash_bad("settings", "Không ghi được file.");
            }
        }
        Err(_) => app.flash_bad("settings", "Không ghi được file."),
    }
}

fn import_cfg(app: &mut App) {
    use rfd::FileDialog;
    let path = FileDialog::new()
        .set_title("Nhập cấu hình")
        .add_filter("JSON", &["json"])
        .add_filter("Tất cả tệp", &["*"])
        .pick_file();
    let path = match path {
        Some(p) => p,
        None => return,
    };
    let raw = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok());
    let raw = match raw {
        Some(Value::Object(m)) => m,
        _ => {
            crate::win::message_box("Nhập cấu hình", "Không đọc được file:\nFile không phải là cấu hình của app này.");
            return;
        }
    };
    if !ask("Nhập cấu hình", "Thay toàn bộ cấu hình hiện tại bằng file này?\n(Ảnh nền và kích thước cửa sổ được giữ nguyên.)") {
        return;
    }
    let (cfg, _) = crate::config::normalize_config(&raw);
    let mut cfg = cfg.0;
    for k in LOCAL_KEYS {
        let v = app.cfg_lock().get(k).cloned().unwrap_or(json!(""));
        cfg.insert(k.to_string(), v);
    }
    let new_cfg = crate::config::Config(cfg);
    let theme_changed = new_cfg.get_str("theme") != app.i_theme;
    app.i_theme = new_cfg.get_str("theme");
    {
        let mut cur = app.cfg_lock();
        *cur = new_cfg;
    }
    app.commit();
    if let Err(_) = crate::win::set_autostart(app.cfg_lock().get_bool("start_with_windows")) {}
    app.sync_from_cfg();
    if theme_changed {
        let np = crate::theme::palette(&app.i_theme);
        crate::theme::apply_style(&app.ctx, &np);
    }
    app.load_bg();
    app.flash("settings", "Đã nhập cấu hình");
}

// ---------------------------------------------------------------- Bot Nino

fn bot(app: &mut App, ui: &mut Ui, p: &Palette) {
    // thẻ trạng thái bot
    egui::Frame::none()
        .fill(p.field)
        .rounding(10.0)
        .inner_margin(egui::Margin::same(18.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let run = app.bot.running();
                let busy = app.bot.busy();
                let color = if run {
                    DOT_ON_C
                } else if busy {
                    DOT_WARN_C
                } else {
                    DOT_OFF_C
                };
                ui.label(RichText::new("●").color(color).size(13.0));
                ui.add_space(8.0);
                let state = app.bot.state();
                let text = if run {
                    format!("{} · đã chạy {}", state, fmt_dur((crate::util::now() - app.bot.started_at()) as i64))
                } else {
                    state
                };
                ui.label(RichText::new(text).size(16.0).color(p.text));
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("Thư mục bot: {}", crate::config::bot_dir().display()))
                    .color(p.muted)
                    .size(12.0),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let run = app.bot.running();
                let busy = app.bot.busy();
                if accent_button(ui, p, "Bật bot", !(run || busy)).clicked() {
                    app.bot.start();
                }
                ui.add_space(8.0);
                if ui.add_enabled(run, egui::Button::new("Tắt bot")).clicked() {
                    app.bot.stop();
                }
                if ui.add_enabled(!busy, egui::Button::new("Khởi động lại")).clicked() {
                    app.bot.restart();
                }
            });
        });
    ui.add_space(24.0);

    section(ui, p, "Tuỳ chọn", None, |ui| {
        if ui.checkbox(&mut app.bot_auto, "Tự bật bot khi mở app").changed() {
            app.cfg_lock().set_bool("bot_auto_start", app.bot_auto);
            app.commit();
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Mở thư mục bot").clicked() {
                let d = crate::config::bot_dir();
                if d.exists() {
                    crate::win::startfile(&d);
                } else {
                    crate::win::message_box("Bot Nino", &format!("Không thấy thư mục bot:\n{}", d.display()));
                }
            }
            if ui.button("Xem log bot").clicked() {
                app.cur = "botlog";
            }
        });
    });

    section(
        ui,
        p,
        "Gửi nhanh qua bot",
        Some("Cần bật bot và đợi bot đăng nhập xong. Lệnh được gửi qua cổng nội bộ 127.0.0.1, không ra ngoài máy."),
        |ui| {
            ui.horizontal(|ui| {
                if ui.button("Chào buổi sáng (/mn)").clicked() {
                    app.bot_call("/api/mn", Some(json!({})), "Đã gửi lời chào buổi sáng", 0);
                }
                if ui.button("Chúc ngủ ngon (/gn)").clicked() {
                    app.bot_call("/api/gn", Some(json!({})), "Đã gửi lời chúc ngủ ngon", 0);
                }
            });
            ui.add_space(12.0);
            ui.label("Thông báo vào kênh thông báo mặc định của bot");
            ui.add_space(4.0);
            egui::Frame::none().fill(p.field).inner_margin(egui::Margin::same(4.0)).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut app.bot_ann)
                        .desired_width(430.0)
                        .desired_rows(3)
                        .frame(false),
                );
            });
            if accent_button(ui, p, "Gửi thông báo", true).clicked() {
                let msg = app.bot_ann.trim().to_string();
                if msg.is_empty() {
                    app.set_botmsg("Hãy nhập nội dung thông báo.", true);
                } else {
                    app.bot_call("/api/announce", Some(json!({"message": msg})), "Đã gửi thông báo", 1);
                }
            }
            ui.add_space(16.0);
            ui.label("Tin nhắn tới một kênh bất kỳ");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let sel_text = app
                    .chan_names
                    .get(app.chan_sel)
                    .cloned()
                    .unwrap_or_else(|| "(chưa có kênh)".to_string());
                egui::ComboBox::from_id_salt("bot-chan")
                    .selected_text(sel_text)
                    .width(340.0)
                    .show_ui(ui, |ui| {
                        let names = app.chan_names.clone();
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut app.chan_sel, i, n.as_str());
                        }
                    });
                if ui.button("Tải danh sách kênh").clicked() {
                    app.bot_load_channels();
                }
            });
            ui.add_space(4.0);
            egui::Frame::none().fill(p.field).inner_margin(egui::Margin::same(4.0)).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut app.bot_chan)
                        .desired_width(430.0)
                        .desired_rows(3)
                        .frame(false),
                );
            });
            if ui.button("Gửi tới kênh đã chọn").clicked() {
                let i = app.chan_sel;
                let msg = app.bot_chan.trim().to_string();
                if i >= app.chan_ids.len() {
                    app.set_botmsg("Hãy bấm Tải danh sách kênh rồi chọn một kênh.", true);
                } else if msg.is_empty() {
                    app.set_botmsg("Hãy nhập nội dung tin nhắn.", true);
                } else {
                    let cid = app.chan_ids[i].clone();
                    app.bot_call(
                        "/api/send-channel-message",
                        Some(json!({"channelId": cid, "message": msg})),
                        "Đã gửi tin nhắn",
                        2,
                    );
                }
            }
        },
    );

    ui.add_space(4.0);
    ui.label(
        RichText::new(app.bot_msg.as_str())
            .color(if app.bot_msg_bad { p.bad } else { p.ok })
            .size(12.5),
    );
}

const DOT_ON_C: Color32 = Color32::from_rgb(0x5d, 0xd6, 0xa2);
const DOT_WARN_C: Color32 = Color32::from_rgb(0xff, 0xbf, 0x5e);
const DOT_OFF_C: Color32 = Color32::from_rgb(0x8b, 0x81, 0x97);

// ---------------------------------------------------------------- Log bot & Hoạt động

fn botlog(app: &mut App, ui: &mut Ui, p: &Palette) {
    log_area(app, ui, p, "botlog", &app.botlogs.clone(), |msg| crate::util::bot_line_is_error(msg), |app| {
        app.botlogs.clear();
    });
}

fn log(app: &mut App, ui: &mut Ui, p: &Palette) {
    log_area(app, ui, p, "log", &app.logs.clone(), |msg| msg.starts_with("Lỗi"), |app| {
        app.logs.clear();
    });
}

fn log_area(
    app: &mut App,
    ui: &mut Ui,
    p: &Palette,
    key: &'static str,
    lines: &[(String, String)],
    is_err: impl Fn(&str) -> bool,
    clear: impl FnOnce(&mut App),
) {
    // khung log chiếm toàn bộ khoảng trống, thanh nút ở đáy
    let bar_h = 34.0;
    let area_h = (ui.available_height() - bar_h - 10.0).max(80.0);
    egui::Frame::none()
        .fill(p.field)
        .rounding(6.0)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.allocate_ui(Vec2::new(ui.available_width(), area_h), |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        egui::Frame::none().inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
                            for (ts, msg) in lines {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new(format!("{}  ", ts)).color(p.muted).monospace());
                                    ui.label(
                                        RichText::new(msg.as_str())
                                            .monospace()
                                            .color(if is_err(msg) { p.bad } else { p.text }),
                                    );
                                });
                            }
                        });
                    });
            });
        });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if let Some((t, bad)) = app.flash_text(key) {
            ui.label(RichText::new(t).color(if bad { p.bad } else { p.ok }));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Xoá").clicked() {
                clear(app);
            }
            if ui.button("Sao chép").clicked() {
                let text = lines.iter().map(|(ts, m)| format!("{}  {}", ts, m)).collect::<Vec<_>>().join("\n");
                ui.output_mut(|o| o.copied_text = text);
                app.flash(key, "Đã sao chép");
            }
        });
    });
}

// ---------------------------------------------------------------- Nhật ký update

fn changelog(app: &mut App, ui: &mut Ui, p: &Palette) {
    // mở trang -> đánh dấu đã xem (nhãn "Mới" ở thanh bên mất đi)
    if app.cfg_lock().get_str("seen_version") != APP_VERSION {
        app.cfg_lock().set_str("seen_version", APP_VERSION);
        app.commit();
    }
    egui::Frame::none()
        .fill(p.field)
        .rounding(6.0)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                egui::Frame::none().inner_margin(egui::Margin::same(12.0)).show(ui, |ui| {
                    for (i, (ver, date, title, items)) in CHANGELOG.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(10.0);
                        }
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Phiên bản {}", ver)).size(18.0).color(p.text));
                            if *ver == crate::util::APP_VERSION {
                                ui.label(RichText::new("đang dùng").color(p.ok).size(12.0));
                            }
                            if !date.is_empty() {
                                ui.label(RichText::new(*date).color(p.muted).size(12.0));
                            }
                        });
                        ui.label(RichText::new(*title).color(p.accent).size(13.5));
                        for (kind, text) in items.iter() {
                            let klabel = KIND_LABEL.iter().find(|(k, _)| k == kind).map(|(_, l)| *l).unwrap_or("");
                            let kcolor = match kind {
                                0 => p.ok,
                                1 => p.accent,
                                2 => p.warn,
                                _ => p.bad,
                            };
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label(RichText::new(klabel).color(kcolor).size(12.0));
                                ui.label(RichText::new(*text).size(13.0).color(p.text));
                            });
                        }
                    }
                });
            });
        });
}
