// media.rs - đọc media đang phát qua Windows Media Session (SMTC), port hàm read_media() của main.py.

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};

pub struct Media {
    pub app: String,
    pub title: String,
    pub artist: String,
    pub pos: f64,
    pub dur: f64,
}

/// Trả về thông tin media đang PHÁT, hoặc None.
pub fn read_media() -> Option<Media> {
    unsafe {
        let mgr = SessionManager::RequestAsync().ok()?.get().ok()?;
        let session = mgr.GetCurrentSession().ok()??;

        let info = session.GetPlaybackInfo().ok()?;
        if info.PlaybackStatus().ok()? != PlaybackStatus::Playing {
            return None;
        }
        let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
        let tl = session.GetTimelineProperties().ok()?;

        let start = tl.StartTime.Duration as f64 / 10_000_000.0;
        let end = tl.EndTime.Duration as f64 / 10_000_000.0;
        let mut pos = tl.Position.Duration as f64 / 10_000_000.0;
        let dur = end - start;

        // Trình duyệt chỉ cập nhật "position" khi play/pause/tua. Vị trí thật bây giờ
        // = position + thời gian đã trôi qua kể từ last_updated_time (nhân tốc độ phát).
        let lut_universal = tl.LastUpdatedTime.UniversalTime as f64 / 10_000_000.0; // giây từ 1601
        let lut_unix = lut_universal - 11_644_473_600.0;
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let elapsed = now_unix - lut_unix;
        let rate = info.PlaybackRate().unwrap_or(0.0);
        let rate = if rate == 0.0 { 1.0 } else { rate };
        if (0.0..(6.0 * 3600.0)).contains(&elapsed) {
            pos += elapsed * rate;
        }
        if dur > 0.0 {
            pos = pos.max(0.0).min(dur);
        }

        let app = session.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default().to_lowercase();
        let title = props.Title().map(|h| h.to_string()).unwrap_or_default();
        let artist = props.Artist().map(|h| h.to_string()).unwrap_or_default();

        Some(Media { app, title, artist, pos, dur })
    }
}
