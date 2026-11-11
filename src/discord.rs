// discord.rs - tự implement Discord Rich Presence IPC qua named pipe
// (thay cho pypresence; protocol: header 8 byte LE [opcode u32, len u32] + JSON).

use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::ffi::OsStrExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcErr {
    /// Không mở được pipe / pipe đứt (Discord chưa mở hoặc vừa tắt).
    Connection,
    /// Discord từ chối Client ID (pypresence.exceptions.InvalidID, code 4000).
    Invalid,
}

pub struct DiscordRpc {
    pipe: File,
    pub client_id: String,
    nonce: u64,
}

fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn try_open_pipe() -> Option<File> {
    use std::os::windows::ffi::OsStringExt;
    for i in 0..=9 {
        let path = format!("\\\\.\\pipe\\discord-ipc-{}", i);
        let wide = to_wide(&path);
        let p = std::ffi::OsString::from_wide(&wide[..wide.len() - 1]);
        if let Ok(f) = OpenOptions::new().read(true).write(true).open(p) {
            return Some(f);
        }
    }
    None
}

impl DiscordRpc {
    fn send(&mut self, op: u32, payload: &Value) -> Result<(), RpcErr> {
        let body = serde_json::to_vec(payload).map_err(|_| RpcErr::Connection)?;
        let mut frame = Vec::with_capacity(8 + body.len());
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&body);
        self.pipe.write_all(&frame).map_err(|_| RpcErr::Connection)?;
        self.pipe.flush().map_err(|_| RpcErr::Connection)
    }

    fn read_exact(&mut self, n: usize) -> Result<Vec<u8>, RpcErr> {
        let mut buf = vec![0u8; n];
        self.pipe.read_exact(&mut buf).map_err(|_| RpcErr::Connection)?;
        Ok(buf)
    }

    /// Đọc một frame; bỏ qua frame điều khiển (op 0/2/3), trả về (op, payload).
    fn read_response(&mut self) -> Result<(u32, Value), RpcErr> {
        loop {
            let header = self.read_exact(8)?;
            let op = u32::from_le_bytes(header[0..4].try_into().unwrap());
            let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
            if len > 10_000_000 {
                return Err(RpcErr::Connection);
            }
            let body = self.read_exact(len)?;
            if op != 1 {
                continue; // chỉ quan tâm op1 (response/event)
            }
            let payload: Value = serde_json::from_slice(&body).map_err(|_| RpcErr::Connection)?;
            let evt = payload.get("evt").and_then(|v| v.as_str()).unwrap_or("");
            match evt {
                "ERROR" => {
                    let code = payload
                        .get("data")
                        .and_then(|d| d.get("code"))
                        .and_then(|c| c.as_i64())
                        .unwrap_or(0);
                    if code == 4000 {
                        return Err(RpcErr::Invalid);
                    }
                    return Err(RpcErr::Connection);
                }
                _ => return Ok((op, payload)),
            }
        }
    }

    /// Kết nối (handshake) bằng Client ID. Trả về Err(Invalid) nếu Discord từ chối ID.
    pub fn connect(client_id: &str) -> Result<DiscordRpc, RpcErr> {
        let mut pipe = try_open_pipe().ok_or(RpcErr::Connection)?;
        let mut rpc = DiscordRpc { pipe, client_id: client_id.to_string(), nonce: 0 };
        let payload = json!({"v": 1, "client_id": client_id});
        rpc.send(0, &payload)?;
        rpc.read_response()?; // READY hoặc ERROR
        Ok(rpc)
    }

    /// SET_ACTIVITY. `activity` là dict có thể chứa None (clear).
    pub fn set_activity(&mut self, activity: Option<&Value>) -> Result<(), RpcErr> {
        self.nonce = self.nonce.wrapping_add(1);
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": std::process::id(),
                "activity": activity,
            },
            "nonce": self.nonce.to_string(),
        });
        self.send(1, &payload)?;
        let (_op, _resp) = self.read_response()?;
        Ok(())
    }

    /// Xoá trạng thái (giữ kết nối).
    pub fn clear(&mut self) -> Result<(), RpcErr> {
        self.set_activity(None)
    }
}

/// Port payload(**act) của pypresence: chuyển dict activity của app sang cấu trúc Discord.
pub fn activity_payload(act: &Value) -> Value {
    let obj = act.as_object().cloned().unwrap_or_default();
    let mut activity = json!({});
    if let Some(v) = obj.get("details") {
        if !v.as_str().unwrap_or("").is_empty() {
            activity["details"] = v.clone();
        }
    }
    if let Some(v) = obj.get("state") {
        if !v.as_str().unwrap_or("").is_empty() {
            activity["state"] = v.clone();
        }
    }
    if let Some(v) = obj.get("activity_type") {
        activity["type"] = v.clone();
    }
    let mut assets = json!({});
    for key in ["large_image", "large_text", "small_image", "small_text"] {
        if let Some(v) = obj.get(key) {
            if !v.as_str().unwrap_or("").is_empty() {
                assets[key] = v.clone();
            }
        }
    }
    if assets.as_object().map(|a| !a.is_empty()).unwrap_or(false) {
        activity["assets"] = assets;
    }
    let mut timestamps = json!({});
    for key in ["start", "end"] {
        if let Some(v) = obj.get(key) {
            if let Some(n) = v.as_i64() {
                timestamps[key] = json!(n);
            }
        }
    }
    if timestamps.as_object().map(|a| !a.is_empty()).unwrap_or(false) {
        activity["timestamps"] = timestamps;
    }
    if let Some(v) = obj.get("buttons") {
        if v.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
            activity["buttons"] = v.clone();
        }
    }
    activity
}
