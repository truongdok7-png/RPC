// icons.rs - IconCache: tải favicon / thumbnail (kiểm tra link sống + xem trước trong app). Có cache.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

use once_cell::sync::Lazy;

pub struct IconCache {
    imgs: Mutex<HashMap<String, Option<Vec<u8>>>>, // url -> bytes hoặc None (tải lỗi)
    times: Mutex<HashMap<String, f64>>,
    pending: Mutex<HashSet<String>>,
}

static ICONS: Lazy<IconCache> = Lazy::new(IconCache::new);

impl IconCache {
    pub fn new() -> Self {
        Self {
            imgs: Mutex::new(HashMap::new()),
            times: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashSet::new()),
        }
    }

    pub fn global() -> &'static IconCache {
        &ICONS
    }

    fn download(url: &str) -> Option<Vec<u8>> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(6))
            .timeout_read(Duration::from_secs(6))
            .build();
        let resp = agent
            .get(url)
            .set("User-Agent", "Mozilla/5.0 DiscordPresence")
            .call()
            .ok()?;
        let mut data = Vec::new();
        resp.into_reader()
            .take(2_000_000)
            .read_to_end(&mut data)
            .ok()?;
        if data.is_empty() {
            None
        } else {
            Some(data)
        }
    }

    /// Tải (chặn) và trả về bytes hoặc None. Ảnh lỗi được thử lại sau 5 phút.
    pub fn fetch(&self, url: &str) -> Option<Vec<u8>> {
        let now = crate::util::now();
        {
            let imgs = self.imgs.lock().unwrap();
            if let Some(hit) = imgs.get(url) {
                let fresh = {
                    let times = self.times.lock().unwrap();
                    now - times.get(url).copied().unwrap_or(0.0) < 300.0
                };
                if hit.is_some() || fresh {
                    return hit.clone();
                }
            }
        }
        let img = Self::download(url);
        let mut imgs = self.imgs.lock().unwrap();
        let mut times = self.times.lock().unwrap();
        imgs.insert(url.to_string(), img.clone());
        times.insert(url.to_string(), now);
        img
    }

    /// Tải ngầm, không chặn (dùng cho giao diện).
    pub fn request(&self, url: &str) {
        {
            let mut pending = self.pending.lock().unwrap();
            let have = self.imgs.lock().unwrap().contains_key(url);
            if have || pending.contains(url) {
                return;
            }
            pending.insert(url.to_string());
        }
        let url2 = url.to_string();
        let this: &'static IconCache = Self::global();
        std::thread::Builder::new()
            .name("icon-fetch".into())
            .spawn(move || {
                let _ = this.fetch(&url2);
                let mut pending = this.pending.lock().unwrap();
                pending.remove(&url2);
            })
            .ok();
    }

    /// Ảnh đã tải xong (None nếu chưa có / lỗi).
    pub fn loaded(&self, url: &str) -> Option<Vec<u8>> {
        self.imgs.lock().unwrap().get(url).cloned().flatten()
    }
}
