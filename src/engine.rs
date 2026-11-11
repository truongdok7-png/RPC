// engine.rs - luồng nền Presence (port class Engine của main.py).

use crossbeam_channel::{bounded, Receiver, RecvTimeoutError};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::activity::build_activity;
use crate::config::Config;
use crate::discord::{activity_payload, DiscordRpc, RpcErr};
use crate::util::now;

pub type LogFn = crossbeam_channel::Sender<(String, String)>; // (giờ:phút:giây, nội dung)

pub struct EngineShared {
    pub status: Mutex<String>,
    pub current: Mutex<Option<Value>>,
    pub started_at: Mutex<f64>,
    pub updates: Mutex<i64>,
}

pub struct Engine {
    pub shared: Arc<EngineShared>,
    pub cfg: Arc<Mutex<Config>>,
    log_sender: LogFn,
    cmd_tx: Sender<()>,
    stop_flag: Arc<AtomicBool>,
    thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl Engine {
    pub fn new(cfg: Arc<Mutex<Config>>, log: LogFn) -> Self {
        let shared = Arc::new(EngineShared {
            status: Mutex::new("Đã tắt".to_string()),
            current: Mutex::new(None),
            started_at: Mutex::new(0.0),
            updates: Mutex::new(0),
        });
        let (cmd_tx, _cmd_rx) = bounded::<()>(1);
        Self {
            shared,
            cfg,
            log_sender: log,
            cmd_tx,
            stop_flag: Arc::new(AtomicBool::new(false)),
            thread: Arc::new(Mutex::new(None)),
        }
    }

    pub fn running(&self) -> bool {
        self.thread
            .lock()
            .unwrap()
            .as_ref()
            .map(|t| !t.is_finished())
            .unwrap_or(false)
    }

    pub fn status(&self) -> String {
        self.shared.status.lock().unwrap().clone()
    }

    pub fn updates(&self) -> i64 {
        *self.shared.updates.lock().unwrap()
    }

    pub fn started_at(&self) -> f64 {
        *self.shared.started_at.lock().unwrap()
    }

    pub fn current(&self) -> Option<Value> {
        self.shared.current.lock().unwrap().clone()
    }

    pub fn start(&self) {
        if self.running() {
            return;
        }
        self.stop_flag.store(false, Ordering::SeqCst);
        *self.shared.started_at.lock().unwrap() = now();
        *self.shared.updates.lock().unwrap() = 0;
        let shared = self.shared.clone();
        let cfg = self.cfg.clone();
        let (cmd_tx, cmd_rx) = bounded::<()>(1);
        let stop_flag = self.stop_flag.clone();
        let log_tx = self.log_sender.clone();
        let handle = std::thread::Builder::new()
            .name("presence-engine".into())
            .spawn(move || {
                run_loop(shared, cfg, log_tx, cmd_rx, stop_flag);
            })
            .ok();
        *self.thread.lock().unwrap() = handle;
    }

    /// Yêu cầu dừng (không chặn luồng giao diện).
    pub fn stop(&self) {
        if !self.running() {
            return;
        }
        self.stop_flag.store(true, Ordering::SeqCst);
        let _ = self.cmd_tx.try_send(());
    }
}
struct LoopState {
    rpc: Option<DiscordRpc>,
    rpc_client_id: String,
    last_sent: Option<Value>,
    bad_cid: String,
}

fn drop_rpc(state: &mut LoopState, clear: bool) {
    if let Some(mut rpc) = state.rpc.take() {
        if clear {
            let _ = rpc.clear();
        }
        drop(rpc);
    }
    state.rpc_client_id.clear();
    state.last_sent = None;
}

fn connect(
    state: &mut LoopState,
    cfg: &Config,
    cid: Option<&str>,
    set_status: &dyn Fn(&str),
    log: &LogFn,
) -> bool {
    let main_cid = cfg.get_str("client_id").trim().to_string();
    let cid = cid
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(main_cid.clone());
    if cid.is_empty() {
        set_status("Chưa nhập Client ID (vào tab Cài đặt)");
        return false;
    }
    match DiscordRpc::connect(&cid) {
        Ok(rpc) => {
            state.rpc = Some(rpc);
            state.rpc_client_id = cid.clone();
            set_status("Đã kết nối Discord");
            true
        }
        Err(e) => {
            state.rpc = None;
            if e == RpcErr::Invalid && cid != main_cid {
                state.bad_cid = cid;
                set_status("Client ID rảnh tuỳ chỉnh không hợp lệ, dùng rảnh mặc định");
                let _ = log.send((
                    crate::util::timestamp_hms(),
                    "Client ID rảnh tuỳ chỉnh bị Discord từ chối, tạm dùng rảnh mặc định".to_string(),
                ));
            } else {
                set_status("Chưa kết nối được Discord (hãy mở app Discord)");
            }
            false
        }
    }
}

/// So sánh activity mới với lần gửi cuối, đúng như _push() của Python.
fn changed(sent: Option<&Value>, act: &Value) -> bool {
    let sent = match sent {
        Some(s) => s,
        None => return true,
    };
    for k in [
        "details",
        "state",
        "activity_type",
        "large_image",
        "large_text",
        "small_image",
        "small_text",
        "buttons",
    ] {
        if sent.get(k) != act.get(k) {
            return true;
        }
    }
    let ge = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    let s_end = ge(sent, "end");
    let a_end = ge(act, "end");
    if s_end != a_end && (s_end - a_end).abs() > 3 {
        return true;
    }
    let s_start = ge(sent, "start");
    let a_start = ge(act, "start");
    (s_start - a_start).abs() > 3
}

fn push(
    state: &mut LoopState,
    shared: &EngineShared,
    log: &LogFn,
    act: Option<&Value>,
) -> Result<(), RpcErr> {
    let act = match act {
        None => {
            if state.last_sent.is_some() {
                if let Some(rpc) = state.rpc.as_mut() {
                    let _ = rpc.clear();
                }
                state.last_sent = None;
            }
            return Ok(());
        }
        Some(a) => a,
    };
    if !changed(state.last_sent.as_ref(), act) {
        return Ok(());
    }
    // payload = {k: v for k, v in act.items() if v not in (None, "") and not k.startswith("_")}
    let payload = activity_payload(act);
    if let Some(rpc) = state.rpc.as_mut() {
        rpc.set_activity(Some(&payload))?;
    }
    state.last_sent = Some(act.clone());
    *shared.updates.lock().unwrap() += 1;
    let details = act.get("details").and_then(|v| v.as_str()).unwrap_or("");
    let st = act.get("state").and_then(|v| v.as_str()).unwrap_or("");
    let _ = log.send((crate::util::timestamp_hms(), format!("Cập nhật: {} | {}", details, st)));
    Ok(())
}

fn run_loop(
    shared: Arc<EngineShared>,
    cfg: Arc<Mutex<Config>>,
    log: LogFn,
    cmd_rx: Receiver<()>,
    stop_flag: Arc<AtomicBool>,
) {
    let set_status = |text: &str| {
        let mut st = shared.status.lock().unwrap();
        if *st != text {
            *st = text.to_string();
            drop(st);
            let _ = log.send((crate::util::timestamp_hms(), text.to_string()));
        }
    };

    set_status("Đang khởi động...");
    let mut state = LoopState {
        rpc: None,
        rpc_client_id: String::new(),
        last_sent: None,
        bad_cid: String::new(),
    };

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }
        let snapshot = cfg.lock().unwrap().clone();
        let mut cfg = snapshot;
        if !state.bad_cid.is_empty() && cfg.get_str("idle_custom_client_id").trim() == state.bad_cid {
            // ID rảnh tuỳ chỉnh bị từ chối -> rảnh mặc định
            cfg.set_bool("idle_custom_enabled", false);
        }

        // dựng activity trước để biết cần kết nối bằng Client ID nào (chính hay của rảnh tuỳ chỉnh)
        let act = build_activity(&cfg);
        let mut want = act
            .as_ref()
            .and_then(|a| a.get("_cid"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| cfg.get_str("client_id").trim().to_string());
        if act.is_none() && state.rpc.is_some() {
            want = state.rpc_client_id.clone(); // không có gì để hiện: giữ kết nối, chỉ xoá trạng thái
        }
        if state.rpc.is_some() && state.rpc_client_id != want {
            drop_rpc(&mut state, true);
            let _ = log.send((
                crate::util::timestamp_hms(),
                if act.as_ref().and_then(|a| a.get("_cid")).is_some() {
                    "Đổi sang Client ID của rảnh tuỳ chỉnh".to_string()
                } else {
                    "Quay lại Client ID chính".to_string()
                },
            ));
            if wait(&cmd_rx, &stop_flag, Duration::from_millis(500)) {
                break;
            }
        }
        if state.rpc.is_none() && !connect(&mut state, &cfg, Some(&want), &set_status, &log) {
            *shared.current.lock().unwrap() = None;
            if wait(&cmd_rx, &stop_flag, Duration::from_secs(8)) {
                break;
            }
            continue;
        }
        *shared.current.lock().unwrap() = act.clone();
        if let Err(e) = push(&mut state, &shared, &log, act.as_ref()) {
            let _ = log.send((
                crate::util::timestamp_hms(),
                format!("Lỗi: mất kết nối Discord ({:?})", e),
            ));
            drop_rpc(&mut state, false);
            *shared.current.lock().unwrap() = None;
            set_status("Mất kết nối Discord, đang thử lại...");
        }

        let wait_secs = {
            let v = cfg.get_f64("interval_seconds");
            if v < 1.0 {
                5.0
            } else {
                v
            }
        };
        if wait(&cmd_rx, &stop_flag, Duration::from_secs_f64(wait_secs)) {
            break;
        }
    }

    // finally: xoá trạng thái rồi tắt
    drop_rpc(&mut state, true);
    *shared.current.lock().unwrap() = None;
    *shared.status.lock().unwrap() = "Đã tắt".to_string();
    let _ = log.send((crate::util::timestamp_hms(), "Đã tắt Presence".to_string()));
}

/// Chờ `dur` giây hoặc tới khi nhận lệnh dừng. Trả về true nếu cần dừng.
fn wait(cmd_rx: &Receiver<()>, stop_flag: &AtomicBool, dur: Duration) -> bool {
    if stop_flag.load(Ordering::SeqCst) {
        return true;
    }
    match cmd_rx.recv_timeout(dur) {
        Ok(()) => true,
        Err(RecvTimeoutError::Timeout) => false,
        Err(RecvTimeoutError::Disconnected) => true,
    }
}
