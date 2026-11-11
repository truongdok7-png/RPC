// bg.rs - các hàm xử lý ảnh nền (port render_bg / glass_panel / solid_panel / rr_mask của main.py).

use egui::Color32;
use image::{imageops, Rgb, RgbImage};

pub const SS: u32 = 4; // siêu lấy mẫu ở bốn góc

fn rr_coverage(x: u32, y: u32, w: u32, h: u32, r: f32) -> u32 {
    let ss = SS as f32;
    let r = r.max(0.0).min(w as f32 / 2.0).min(h as f32 / 2.0);
    let mut count = 0u32;
    for j in 0..SS {
        for i in 0..SS {
            let px = x as f32 + (i as f32 + 0.5) / ss;
            let py = y as f32 + (j as f32 + 0.5) / ss;
            // kiểm tra nằm trong chữ nhật bo góc
            let qx = px.max(r).min(w as f32 - r);
            let qy = py.max(r).min(h as f32 - r);
            let dx = px - qx;
            let dy = py - qy;
            if dx * dx + dy * dy <= r * r {
                count += 1;
            }
        }
    }
    count
}

/// Mặt nạ chữ nhật bo góc đã khử răng cưa (255 = nằm trong hình).
pub fn rr_mask(w: u32, h: u32, r: f32) -> Vec<u8> {
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let total = (SS * SS) as f32;
    let mut m = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let cov = rr_coverage(x, y, w, h, r) as f32 / total;
            m[(y * w + x) as usize] = (cov * 255.0).round() as u8;
        }
    }
    m
}

/// Phóng/cắt ảnh cho vừa kín khung w×h (giữ tỉ lệ, cắt phần thừa ở giữa).
pub fn cover_resize(src: &RgbImage, w: u32, h: u32) -> RgbImage {
    let (iw, ih) = (src.width().max(1), src.height().max(1));
    let s = (w as f32 / iw as f32).max(h as f32 / ih as f32);
    let nw = ((iw as f32 * s).round() as u32).max(w);
    let nh = ((ih as f32 * s).round() as u32).max(h);
    let img = imageops::resize(src, nw, nh, imageops::FilterType::Triangle);
    let left = (nw - w) / 2;
    let top = (nh - h) / 2;
    imageops::crop_imm(&img, left, top, w, h).to_image()
}

/// Nền mặc định: hoàng hôn tím mận, có vệt hồng ở góc dưới bên phải.
pub fn gradient_image(w: u32, h: u32) -> RgbImage {
    let mut img = RgbImage::new(w, h);
    let dark = Rgb([0x12, 0x0d, 0x19]);
    let light = Rgb([0x34, 0x23, 0x5a]);
    let pink = Rgb([0xff, 0x6f, 0x91]);
    let cx = w as f32;
    let cy = h as f32;
    let halfw = w as f32 * 0.55;
    let halfh = h as f32 * 0.65;
    for y in 0..h {
        for x in 0..w {
            let gy = y as f32 / h.max(1) as f32;
            let gx = x as f32 / w.max(1) as f32;
            let m = ((gy + gx) / 2.0).clamp(0.0, 1.0);
            let lerp = |a: u8, b: u8, t: f32| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
            let mut r = lerp(light[0], dark[0], m);
            let mut g = lerp(light[1], dark[1], m);
            let mut b = lerp(light[2], dark[2], m);
            // vệt hồng (tâm góc dưới phải)
            let dx = (x as f32 - cx) / halfw.max(1.0);
            let dy = (y as f32 - cy) / halfh.max(1.0);
            let dd = (dx * dx + dy * dy).sqrt();
            let layer = (1.0 - dd).clamp(0.0, 1.0) * 0.38;
            r = lerp(r, pink[0], layer);
            g = lerp(g, pink[1], layer);
            b = lerp(b, pink[2], layer);
            img.put_pixel(x, y, Rgb([r, g, b]));
        }
    }
    img
}

fn blend_pixel(dst: &mut Rgb<u8>, src: Rgb<u8>, a: u8) {
    let t = a as f32 / 255.0;
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    *dst = Rgb([lerp(dst[0], src[0]), lerp(dst[1], src[1]), lerp(dst[2], src[2])]);
}

fn paste_masked(img: &mut RgbImage, panel: RgbImage, x0: u32, y0: u32, mask: &[u8], mask_w: u32) {
    for (i, a) in mask.iter().enumerate() {
        if *a == 0 {
            continue;
        }
        let px = x0 + (i as u32) % mask_w;
        let py = y0 + (i as u32) / mask_w;
        if px >= img.width() || py >= img.height() {
            continue;
        }
        blend_pixel(img.get_pixel_mut(px, py), panel.get_pixel(px - x0, py - y0).to_owned(), *a);
    }
}

fn draw_ring(img: &mut RgbImage, x0: u32, y0: u32, w: u32, h: u32, r: f32, color: Rgb<u8>, opacity: f32) {
    if opacity <= 0.0 || w < 3 || h < 3 {
        return;
    }
    let outer = rr_mask(w, h, r);
    let inner = rr_mask(w.saturating_sub(2), h.saturating_sub(2), (r - 1.0).max(0.0));
    for y in 0..h {
        for x in 0..w {
            let o = outer[(y * w + x) as usize] as f32;
            let in_x = x.checked_sub(1).unwrap_or(0);
            let in_y = y.checked_sub(1).unwrap_or(0);
            let i = if x >= 1 && y >= 1 && x <= w - 2 && y <= h - 2 {
                inner[(in_y * (w - 2) + in_x) as usize] as f32
            } else {
                0.0
            };
            let a = ((o - i).max(0.0) / 255.0 * opacity * 255.0).round() as u8;
            if a > 0 {
                blend_pixel(img.get_pixel_mut(x0 + x, y0 + y), color, a);
            }
        }
    }
}

/// Lớp kính: làm mờ phần ảnh nền phía sau rồi phủ màu, để chữ luôn đọc được trên mọi ảnh.
pub fn glass_panel(img: &mut RgbImage, x0: u32, y0: u32, w: u32, h: u32, r: f32, tint: Color32, alpha: f32) {
    if w == 0 || h == 0 || x0 >= img.width() || y0 >= img.height() {
        return;
    }
    let w = w.min(img.width() - x0);
    let h = h.min(img.height() - y0);
    let crop = imageops::crop(img, x0, y0, w, h).to_image();
    let panel = imageops::blur(&crop, 10.0);
    // ảnh nền càng sáng, kính càng đậm
    let mut lum = 0f32;
    let n = (w * h) as f32;
    for p in panel.pixels() {
        lum += (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0;
    }
    let lum = lum / n.max(1.0);
    let alpha = (alpha.max(alpha + (lum - 0.3) * 0.75)).min(0.9);
    let tint_rgb = Rgb([tint.r(), tint.g(), tint.b()]);
    let mut out = RgbImage::new(w, h);
    for (x, y, p) in panel.enumerate_pixels() {
        let t = alpha;
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        out.put_pixel(x, y, Rgb([lerp(p[0], tint_rgb[0]), lerp(p[1], tint_rgb[1]), lerp(p[2], tint_rgb[2])]));
    }
    let mask = rr_mask(w, h, r);
    paste_masked(img, out, x0, y0, &mask, w);
    draw_ring(img, x0, y0, w, h, r, Rgb([255, 255, 255]), 0.10);
}

/// Mặt phẳng đặc bo góc + viền.
pub fn solid_panel(img: &mut RgbImage, x0: u32, y0: u32, w: u32, h: u32, r: f32, fill: Rgb<u8>, ring: Rgb<u8>, ring_opacity: f32) {
    if w == 0 || h == 0 || x0 >= img.width() || y0 >= img.height() {
        return;
    }
    let w = w.min(img.width() - x0);
    let h = h.min(img.height() - y0);
    let mask = rr_mask(w, h, r);
    let panel = RgbImage::from_pixel(w, h, fill);
    paste_masked(img, panel, x0, y0, &mask, w);
    draw_ring(img, x0, y0, w, h, r, ring, ring_opacity);
}

/// Ảnh cắt vừa khung w×h và bo góc (art_tile).
pub fn art_tile(src: &RgbImage, w: u32, h: u32, radius: f32, bg: Rgb<u8>) -> RgbImage {
    let mut out = RgbImage::from_pixel(w, h, bg);
    let inner = cover_resize(src, w, h);
    let mask = rr_mask(w, h, radius);
    paste_masked(&mut out, inner, 0, 0, &mask, w);
    out
}

pub fn rgb_to_rgba(img: &RgbImage) -> Vec<u8> {
    let mut out = Vec::with_capacity((img.width() * img.height() * 4) as usize);
    for p in img.pixels() {
        out.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    out
}

/// Ô ảnh giữ chỗ khi nguồn không có ảnh: nền pha màu loại hoạt động + một ký hiệu vẽ tay (kind_tile).
pub fn kind_tile(p: &crate::theme::Palette, key: Option<&str>, size: u32) -> RgbImage {
    let col = match key {
        Some("watch") => p.watch,
        Some("listen") => p.listen,
        Some("play") => p.play,
        Some("idle") => p.idle,
        _ => p.muted,
    };
    let col = Rgb([col.r(), col.g(), col.b()]);
    let field = Rgb([p.field.r(), p.field.g(), p.field.b()]);
    let bg = mix_rgb(field, col, 0.16);
    let mut im = RgbImage::from_pixel(size, size, field);
    let s = size as f32;
    let u = s / 76.0;
    // nền pha màu bo góc
    let mask = rr_mask(size, size, 16.0);
    let fill = RgbImage::from_pixel(size, size, bg);
    paste_masked(&mut im, fill, 0, 0, &mask, size);
    let put = |im: &mut RgbImage, x: f32, y: f32, c: Rgb<u8>| {
        if x >= 0.0 && y >= 0.0 && (x as u32) < im.width() && (y as u32) < im.height() {
            *im.get_pixel_mut(x as u32, y as u32) = c;
        }
    };
    match key {
        Some("watch") => {
            // tam giác phát
            let tri = |px: f32, py: f32| {
                // điểm trong tam giác (30,24)-(30,52)-(54,38)
                let denom = (30.0 - 54.0) * (24.0 - 38.0) - (30.0 - 54.0) * (52.0 - 38.0);
                let _ = denom;
                let inside = px >= 30.0 * u && px <= 54.0 * u && py >= (24.0 + (px / u - 30.0) * (14.0 / 24.0)) * u && py <= (52.0 - (px / u - 30.0) * (14.0 / 24.0)) * u;
                inside
            };
            for y in (20.0 * u) as u32..(56.0 * u) as u32 {
                for x in (28.0 * u) as u32..(56.0 * u) as u32 {
                    if tri(x as f32, y as f32) {
                        put(&mut im, x as f32, y as f32, col);
                    }
                }
            }
        }
        Some("listen") => {
            // thanh equalizer
            for (i, hh) in [16.0, 30.0, 22.0, 36.0].iter().enumerate() {
                let x = (22.0 + i as f32 * 9.0) * u;
                for yy in ((56.0 - hh) * u) as u32..(56.0 * u) as u32 {
                    for xx in x as u32..(x + 5.0 * u) as u32 {
                        put(&mut im, xx as f32, yy as f32, col);
                    }
                }
            }
        }
        Some("play") => {
            // kim cương
            let cx = 38.0 * u;
            let cy = 38.0 * u;
            let a = 18.0 * u;
            for yy in (20.0 * u) as u32..(56.0 * u) as u32 {
                for xx in (20.0 * u) as u32..(56.0 * u) as u32 {
                    if ((xx as f32 - cx).abs() / a) + ((yy as f32 - cy).abs() / a) <= 1.0 {
                        put(&mut im, xx as f32, yy as f32, col);
                    }
                }
            }
        }
        Some("idle") => {
            // trăng khuyết
            for yy in (20.0 * u) as u32..(54.0 * u) as u32 {
                for xx in (22.0 * u) as u32..(56.0 * u) as u32 {
                    let in_main = in_ellipse(xx as f32, yy as f32, 22.0 * u, 20.0 * u, 56.0 * u, 54.0 * u);
                    if in_main {
                        put(&mut im, xx as f32, yy as f32, col);
                    }
                }
            }
            for yy in (15.0 * u) as u32..(45.0 * u) as u32 {
                for xx in (32.0 * u) as u32..(62.0 * u) as u32 {
                    if in_ellipse(xx as f32, yy as f32, 32.0 * u, 15.0 * u, 62.0 * u, 45.0 * u) {
                        put(&mut im, xx as f32, yy as f32, bg);
                    }
                }
            }
        }
        _ => {
            // đang tắt: vòng tròn rỗng
            let cx = 38.0 * u;
            let cy = 38.0 * u;
            let ro = 14.0 * u;
            let ri = 11.0 * u;
            for yy in (24.0 * u) as u32..(52.0 * u) as u32 {
                for xx in (24.0 * u) as u32..(52.0 * u) as u32 {
                    let d = ((xx as f32 - cx).powi(2) + (yy as f32 - cy).powi(2)).sqrt();
                    if d <= ro && d >= ri {
                        put(&mut im, xx as f32, yy as f32, col);
                    }
                }
            }
        }
    }
    im
}

fn in_ellipse(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    let cx = (x0 + x1) / 2.0;
    let cy = (y0 + y1) / 2.0;
    let rx = (x1 - x0) / 2.0;
    let ry = (y1 - y0) / 2.0;
    let dx = (x - cx) / rx.max(1.0);
    let dy = (y - cy) / ry.max(1.0);
    dx * dx + dy * dy <= 1.0
}

pub fn mix_rgb(a: Rgb<u8>, b: Rgb<u8>, t: f32) -> Rgb<u8> {
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgb([lerp(a[0], b[0]), lerp(a[1], b[1]), lerp(a[2], b[2])])
}
