// bot.rs - chạy bot Nino (node nino.js) như tiến trình con (port class BotRunner của main.py).

use crossbeam_channel::Sender;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

use crate::config::bot_dir;
use crate::util::{bot_clean, now};

pub const BOT_PORT: u16 = 3000;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

struct Inner {
    proc: Mutex<Option<Child>>,
    job: Mutex<Option<isize>>,
    state: Mutex<String>,
    started_at: Mutex<f64>,
    busy: AtomicBool,
    stopping: AtomicBool,
    log: Sender<(String, String)>,
}

/// Chạy bot Nino như tiến trình con; mọi dòng log của bot đưa vào log -> tab "Log bot".
#[derive(Clone)]
pub struct BotRunner {
    inner: Arc<Inner>,
}

impl BotRunner {
    pub fn new(log: Sender<(String, String)>) -> Self {
        Self {
            inner: Arc::new(Inner {
                proc: Mutex::new(None),
                job: Mutex::new(None),
                state: Mutex::new("Đã dừng".to_string()),
                started_at: Mutex::new(0.0),
                busy: AtomicBool::new(false),
                stopping: AtomicBool::new(false),
                log,
            }),
        }
    }

    pub fn running(&self) -> bool {
        running_inner(&self.inner)
    }

    pub fn busy(&self) -> bool {
        self.inner.busy.load(Ordering::SeqCst)
    }

    pub fn state(&self) -> String {
        self.inner.state.lock().unwrap().clone()
    }

    pub fn started_at(&self) -> f64 {
        *self.inner.started_at.lock().unwrap()
    }

    pub fn start(&self) {
        if self.running() || self.busy() {
            return;
        }
        self.inner.busy.store(true, Ordering::SeqCst);
        self.inner.stopping.store(false, Ordering::SeqCst);
        let inner = self.inner.clone();
        let _ = std::thread::Builder::new().name("bot-runner".into()).spawn(move || {
            bot_run(inner);
        });
    }

    pub fn stop(&self) {
        if !running_inner(&self.inner) {
            return;
        }
        // dừng ở luồng nền, không đơ giao diện
        let inner = self.inner.clone();
        let _ = std::thread::Builder::new().name("bot-stop".into()).spawn(move || {
            stop_inner(&inner);
        });
    }

    pub fn restart(&self) {
        let inner = self.inner.clone();
        let _ = std::thread::Builder::new().name("bot-restart".into()).spawn(move || {
            stop_inner(&inner);
            for _ in 0..30 {
                if !running_inner(&inner) && !inner.busy.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if !running_inner(&inner) && !inner.busy.load(Ordering::SeqCst) {
                inner.busy.store(true, Ordering::SeqCst);
                inner.stopping.store(false, Ordering::SeqCst);
                let inner2 = inner.clone();
                let _ = std::thread::Builder::new().name("bot-runner".into()).spawn(move || {
                    bot_run(inner2);
                });
            }
        });
    }
}

fn handle_from_isize(v: isize) -> HANDLE {
    HANDLE(v as *mut core::ffi::c_void)
}

fn running_inner(inner: &Inner) -> bool {
    match inner.proc.lock().unwrap().as_ref() {
        Some(p) => p.try_wait().map(|s| s.is_none()).unwrap_or(false),
        None => false,
    }
}

fn stop_inner(inner: &Inner) {
    let has = inner.proc.lock().unwrap().is_some();
    if !has {
        return;
    }
    inner.stopping.store(true, Ordering::SeqCst);
    // Job Object kill-on-close: chấm dứt cả tiến trình con chỉ bằng TerminateJobObject
    if let Some(job) = *inner.job.lock().unwrap() {
        unsafe {
            let _ = TerminateJobObject(handle_from_isize(job), 0);
        }
    }
    if let Some(mut child) = inner.proc.lock().unwrap().take() {
        let _ = child.kill();
    }
}

fn emit(inner: &Inner, msg: &str) {
    let _ = inner.log.send((crate::util::timestamp_hms(), msg.to_string()));
}

fn find_node() -> Option<PathBuf> {
    let mut cands = vec![bot_dir().join("node").join("node.exe")]; // tuỳ chọn: nhét sẵn node.exe vào bot\node
    if let Some(w) = which("node") {
        cands.push(PathBuf::from(w));
    }
    for env in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Ok(base) = std::env::var(env) {
            let b = PathBuf::from(base);
            cands.push(b.join("nodejs").join("node.exe"));
            cands.push(b.join("Programs").join("nodejs").join("node.exe"));
        }
    }
    cands.into_iter().find(|c| c.exists())
}

fn spawn_cmd(args: &[&Path], node: &Path) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt;
    let dir = bot_dir();
    let mut cmd = Command::new(args[0]);
    for a in &args[1..] {
        cmd.arg(a);
    }
    let node_parent = node.parent().map(|p| p.display().to_string()).unwrap_or_default();
    let path = format!("{};{}", node_parent, std::env::var("PATH").unwrap_or_default());
    cmd.current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NINO_API_PORT", BOT_PORT.to_string())
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0")
        .env("PATH", path)
        .creation_flags(CREATE_NO_WINDOW);
    cmd.spawn()
}

fn pump(inner: &Inner, proc: &mut Child) -> i32 {
    // stderr của bot gộp vào log (giống stderr=subprocess.STDOUT của Python)
    if let Some(err) = proc.stderr.take() {
        let log = inner.log.clone();
        let _ = std::thread::Builder::new().name("bot-stderr".into()).spawn(move || {
            let reader = BufReader::new(err);
            for line in reader.lines().flatten() {
                let line = bot_clean(&line);
                if !line.is_empty() {
                    let _ = log.send((crate::util::timestamp_hms(), line));
                }
            }
        });
    }
    if let Some(out) = proc.stdout.take() {
        let reader = BufReader::new(out);
        for raw in reader.lines() {
            match raw {
                Ok(line) => {
                    let line = bot_clean(&line);
                    if !line.is_empty() {
                        emit(inner, &line);
                    }
                }
                Err(_) => break,
            }
        }
    }
    match proc.wait() {
        Ok(status) => status.code().unwrap_or(-1),
        Err(_) => -1,
    }
}

fn install(inner: &Inner, node: &Path) -> bool {
    let cli = node
        .parent()
        .unwrap_or(Path::new("."))
        .join("node_modules")
        .join("npm")
        .join("bin")
        .join("npm-cli.js");
    let mut args: Vec<PathBuf> = if cli.exists() {
        vec![node.to_path_buf(), cli]
    } else {
        match which("npm") {
            Some(npm) => vec![PathBuf::from(npm)],
            None => {
                emit(inner, "Lỗi: không tìm thấy npm. Hãy cài lại Node.js bản LTS (https://nodejs.org).");
                return false;
            }
        }
    };
    emit(inner, "Lần đầu chạy: đang cài thư viện cho bot (cần internet, có thể mất vài phút)...");
    args.push(PathBuf::from("install"));
    args.push(PathBuf::from("--omit=dev"));
    args.push(PathBuf::from("--no-audit"));
    args.push(PathBuf::from("--no-fund"));
    let refs: Vec<&Path> = args.iter().map(|p| p.as_path()).collect();
    match spawn_cmd(&refs, node) {
        Ok(mut proc) => pump(inner, &mut proc) == 0,
        Err(_) => false,
    }
}

/// Job Object: bot Node.js tự chết theo app, kể cả khi app bị tắt đột ngột.
fn attach_job(proc_handle: isize) -> Option<isize> {
    unsafe {
        let job = CreateJobObjectW(None, None).ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .is_err()
        {
            return None;
        }
        if AssignProcessToJobObject(job, handle_from_isize(proc_handle)).is_err() {
            return None;
        }
        Some(job.0 as isize)
    }
}

fn bot_run(inner: Arc<Inner>) {
    let dir = bot_dir();
    if !dir.join("nino.js").exists() {
        *inner.state.lock().unwrap() = "Thiếu thư mục bot".to_string();
        emit(&inner, &format!(
            "Lỗi: không thấy {}. Hãy đặt thư mục \"bot\" cạnh app.",
            dir.join("nino.js").display()
        ));
        inner.busy.store(false, Ordering::SeqCst);
        return;
    }
    let node = match find_node() {
        Some(n) => n,
        None => {
            *inner.state.lock().unwrap() = "Chưa cài Node.js".to_string();
            emit(&inner, "Lỗi: chưa tìm thấy Node.js. Cài bản LTS tại https://nodejs.org rồi bấm Bật bot lại.");
            inner.busy.store(false, Ordering::SeqCst);
            return;
        }
    };
    if !dir.join("node_modules").join("discord.js").exists() {
        *inner.state.lock().unwrap() = "Đang cài thư viện...".to_string();
        if !install(&inner, &node) {
            *inner.state.lock().unwrap() = "Cài thư viện thất bại".to_string();
            emit(&inner, "Lỗi: cài thư viện cho bot thất bại (xem các dòng phía trên).");
            inner.busy.store(false, Ordering::SeqCst);
            return;
        }
    }
    *inner.state.lock().unwrap() = "Đang khởi động...".to_string();
    let args = [node.as_path(), Path::new("nino.js")];
    let mut proc = match spawn_cmd(&args, &node) {
        Ok(p) => p,
        Err(e) => {
            *inner.state.lock().unwrap() = "Lỗi khởi động".to_string();
            emit(&inner, &format!("Lỗi: {:?}", e));
            inner.busy.store(false, Ordering::SeqCst);
            return;
        }
    };
    // Job Object gắn với handle thật của tiến trình (giống proc._handle của Python)
    let job = attach_job(proc.as_raw_handle() as isize);
    let pid = proc.id();
    *inner.started_at.lock().unwrap() = now();
    *inner.proc.lock().unwrap() = Some(proc);
    *inner.job.lock().unwrap() = job;
    inner.busy.store(false, Ordering::SeqCst);
    *inner.state.lock().unwrap() = "Đang chạy".to_string();
    emit(&inner, &format!("Bot đã khởi động (PID {})", pid));

    let code = {
        let mut guard = inner.proc.lock().unwrap();
        match guard.as_mut() {
            Some(p) => pump(&inner, p),
            None => -1,
        }
    };
    *inner.proc.lock().unwrap() = None;
    if inner.stopping.load(Ordering::SeqCst) {
        *inner.state.lock().unwrap() = "Đã dừng".to_string();
        emit(&inner, "Bot đã dừng");
    } else {
        *inner.state.lock().unwrap() = format!("Bot bị dừng (mã {})", code);
        emit(&inner, &format!("Lỗi: bot tự dừng với mã thoát {}", code));
    }
    inner.busy.store(false, Ordering::SeqCst);
}

/// shutil.which(path) của Python.
fn which(name: &str) -> Option<String> {
    let path_env = std::env::var("PATH").unwrap_or_default();
    for dir in path_env.split(';') {
        if dir.is_empty() {
            continue;
        }
        let dir = dir.trim_matches('"');
        let cand = PathBuf::from(dir).join(format!("{}.exe", name));
        if cand.exists() {
            return Some(cand.display().to_string());
        }
        let cand = PathBuf::from(dir).join(name); // .bat / .cmd (npm trên Windows)
        if cand.exists() {
            return Some(cand.display().to_string());
        }
    }
    None
}

// ---------------------------------------------------------------- gọi API nội bộ của bot

/// Gọi API nội bộ của bot (127.0.0.1). data=None -> GET, ngược lại POST JSON. Bot tắt -> None.
pub fn bot_api(path: &str, data: Option<&Value>, timeout: u64) -> Option<Value> {
    let url = format!("http://127.0.0.1:{}{}", BOT_PORT, path);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(timeout))
        .timeout_read(Duration::from_secs(timeout))
        .build();
    let resp = match data {
        None => agent.get(&url).call(),
        Some(d) => agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_string(&serde_json::to_string(d).unwrap_or_else(|_| "{}".into())),
    };
    match resp {
        Ok(r) => r.into_json::<Value>().ok(),
        Err(ureq::Error::Status(_, r)) => {
            // HTTPError của Python: đọc body JSON nếu có
            match r.into_json::<Value>() {
                Ok(v) => Some(v),
                Err(_) => Some(serde_json::json!({"success": false, "error": "HTTP error"})),
            }
        }
        Err(_) => None,
    }
}
