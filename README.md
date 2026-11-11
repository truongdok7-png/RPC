# DiscordPresence (Rust)

Port Rust của bản Python `DiscordPresence` (main.py) — desktop app hiển thị Rich Presence
nhạc / video / web đang xem lên Discord, kèm GUI, khay hệ thống, cửa sổ thu nhỏ,
tích hợp bot Nino. Không cần Python hay PreMiD.

## Biên dịch

```bat
cargo build --release
```

File exe nằm ở `target\release\discordpresence.exe`. Cần Rust toolchain ổn định (Windows).

## Chạy

- Đặt `config.json` (của bản Python, hoặc để app tự tạo) cạnh file exe — app đọc/ghi đúng
  mọi khoá của bản gốc, tự migrate config cũ (`custom_sites` → `sites`, v.v.).
- Bot Nino (Node.js) đặt ở thư mục `bot` cạnh app như cũ; app chỉ bật/tắt và gọi API
  nội bộ của bot qua cổng 3000, logic bot không đổi.
- Icon `app.ico` đặt cạnh exe nếu có (không có thì dùng hình vẽ mặc định).

```bat
discordpresence.exe              :: mở bình thường
discordpresence.exe --minimized  :: mở sẵn dưới khay (lệnh dùng cho autostart)
```

## Ghi chú kỹ thuật

- GUI: eframe/egui 0.27; khay hệ thống: tray-icon + muda; đọc địa chỉ trình duyệt:
  crate `uiautomation` (thay thư viện python `uiautomation`); Discord IPC tự implement
  qua named pipe `\\.\pipe\discord-ipc-{0..9}`.
- Cấu trúc `src/`:
  - `main.rs` — điểm vào, single-instance (named mutex), icon cửa sổ.
  - `app.rs` — cửa sổ chính, thanh bên / tiêu đề / thẻ nội dung, khay, mini, toast.
  - `pages.rs`, `pages/dialog.rs` — 11 trang + hộp thoại thêm/sửa trang web.
  - `engine.rs` — luồng cập nhật presence (media + web + idle + giờ yên lặng).
  - `activity.rs` — dựng payload activity từ nguồn.
  - `media.rs` — Windows Media Session; `browser.rs` — UI Automation.
  - `discord.rs` — kết nối Discord IPC; `bot.rs` — chạy bot Nino + gọi API.
  - `config.rs` — config.json tương thích 100% với bản Python.
  - `win.rs` — WinAPI tiện ích; `theme.rs`, `bg.rs`, `icons.rs`, `util.rs`.

## Trạng thái biên dịch

Code đã được sửa hết lỗi biên dịch và qua `cargo check --target x86_64-pc-windows-msvc`
(0 lỗi, còn một số cảnh báo "unused" vô hại). Đã kèm `Cargo.lock` để build ra đúng các
phiên bản thư viện đã kiểm tra.

Lưu ý: việc kiểm tra chỉ gồm biên dịch (type-check / borrow-check), **chưa liên kết (link) và
chưa chạy thử** trên Windows, nên hành vi lúc chạy (khay hệ thống, UI Automation, Discord IPC…)
vẫn cần bạn thử thực tế. Nếu `cargo build --release` báo lỗi ở bước link/ring, hãy cài
"Desktop development with C++" (MSVC Build Tools).
