//! Windows-only, current-user lifecycle control; independent of chat/CLI transport.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io,
    os::windows::{io::AsRawHandle, process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio},
    ptr,
    sync::mpsc,
    time::{Duration, Instant},
};
use tauri::Manager;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
    runtime::{Builder, Runtime},
    time::{sleep, timeout},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetHandleInformation, LocalFree, SetHandleInformation, HANDLE,
        HANDLE_FLAG_INHERIT, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    },
    System::{
        Console::{
            AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
            STD_OUTPUT_HANDLE,
        },
        Pipes::GetNamedPipeServerProcessId,
        RemoteDesktop::ProcessIdToSessionId,
        Threading::{
            CreateMutexW, GetCurrentProcess, OpenProcess, OpenProcessToken, ReleaseMutex,
            WaitForSingleObject, PROCESS_SYNCHRONIZE,
        },
    },
};

pub const PROTOCOL: &str = "octoppet-component@1";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_FRAME: usize = 8192;

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    protocol: String,
    command: String,
}

fn response(command: &str, running: bool, visible: bool) -> Value {
    json!({"protocol":PROTOCOL,"id":"octoppet","version":VERSION,"ok":true,
        "command":command,"running":running,"visible":visible,
        "pid":if running {Some(std::process::id())} else {None},"error":null,
        "data_dir":data_dir().ok(),
        "control_pipe":current_sid().ok().and_then(|sid| pipe_name(&sid).ok())})
}

fn failure(command: &str, code: &str, message: impl ToString) -> Value {
    json!({"protocol":PROTOCOL,"id":"octoppet","version":VERSION,"ok":false,
        "command":command,"error":{"code":code,"message":message.to_string()}})
}

fn command_valid(command: &str) -> bool {
    matches!(command, "status" | "start" | "stop" | "show" | "hide")
}

pub fn data_dir() -> Result<PathBuf, String> {
    let path = std::env::var_os("OCTOPPET_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("com.octop.pet")))
        .ok_or("APPDATA is unavailable")?;
    if !path.is_absolute() {
        return Err("user data directory must be absolute".into());
    }
    // Resolve existing ancestors before creating anything, including symlink targets.
    let mut ancestor = path;
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        suffix.push(
            ancestor
                .file_name()
                .ok_or("invalid user data path")?
                .to_os_string(),
        );
        if !ancestor.pop() {
            return Err("invalid user data path".into());
        }
    }
    let mut path = ancestor.canonicalize().map_err(|e| e.to_string())?;
    for name in suffix.into_iter().rev() {
        path.push(name);
    }
    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("executable directory is unavailable")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let managed_root = exe_dir
        .parent()
        .filter(|p| {
            p.file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("versions"))
        })
        .and_then(|p| p.parent())
        .unwrap_or(&exe_dir);
    if path.starts_with(managed_root) || managed_root.starts_with(&path) {
        return Err("user data must be outside the component directory".into());
    }
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

fn current_sid() -> io::Result<String> {
    unsafe {
        let mut token: HANDLE = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut size = 0;
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut size);
        // TOKEN_USER contains pointers; the allocation must have pointer alignment.
        let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        );
        CloseHandle(token);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut text = ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        LocalFree(text.cast());
        Ok(sid)
    }
}

fn pipe_name(sid: &str) -> Result<String, String> {
    // Fixed FNV-1a scope hash; Rust's DefaultHasher is not stable across releases.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in data_dir()?.to_string_lossy().to_lowercase().as_bytes() {
        hash = (hash ^ *byte as u64).wrapping_mul(0x100000001b3);
    }
    let mut session = 0;
    unsafe {
        if ProcessIdToSessionId(std::process::id(), &mut session) == 0 {
            return Err(io::Error::last_os_error().to_string());
        }
    }
    let mode = if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    };
    Ok(format!(
        r"\\.\pipe\octoppet-{sid}-{session}-{mode}-{:x}",
        hash
    ))
}

fn create_pipe(name: &str, sid: &str, first: bool) -> io::Result<NamedPipeServer> {
    // Explicit current-user ACL; no Everyone/anonymous or remote access.
    let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})\0").encode_utf16().collect();
    unsafe {
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let pipe = ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            // Clients may briefly retain a disconnected instance after acknowledging.
            // Use the default limit while keeping only one pending server listener.
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            );
        LocalFree(descriptor);
        pipe
    }
}

fn runtime() -> Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("component runtime")
}

async fn read_frame<R: AsyncReadExt + Unpin>(stream: &mut R) -> io::Result<Vec<u8>> {
    let len = stream.read_u32_le().await? as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame size",
        ));
    }
    let mut frame = vec![0; len];
    stream.read_exact(&mut frame).await?;
    Ok(frame)
}

async fn write_frame<W: AsyncWriteExt + Unpin>(stream: &mut W, value: &Value) -> io::Result<()> {
    let frame = serde_json::to_vec(value)?;
    if frame.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "response too large",
        ));
    }
    stream.write_u32_le(frame.len() as u32).await?;
    stream.write_all(&frame).await
}

async fn rpc(name: &str, command: &str) -> io::Result<Value> {
    let mut client = loop {
        match ClientOptions::new().open(name) {
            Ok(client) => break client,
            Err(e) if e.raw_os_error() == Some(231) => sleep(Duration::from_millis(25)).await,
            Err(e) => return Err(e),
        }
    };
    write_frame(&mut client, &json!({"protocol":PROTOCOL,"command":command})).await?;
    let value: Value = serde_json::from_slice(&read_frame(&mut client).await?)?;
    let mut server_pid = 0;
    if unsafe { GetNamedPipeServerProcessId(client.as_raw_handle(), &mut server_pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if value["protocol"] != PROTOCOL || value["id"] != "octoppet" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incompatible response",
        ));
    }
    if value["running"] == true && value["pid"].as_u64() != Some(server_pid as u64) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "process identity mismatch",
        ));
    }
    // Keep the server connected until its entire reply has been consumed.
    client.write_u8(1).await?;
    Ok(value)
}

// Startup can briefly close a connection before the GUI event loop is ready.
// Retry only idempotent probes; the caller's total deadline still bounds this loop.
async fn startup_rpc(name: &str, command: &str) -> io::Result<Value> {
    loop {
        match rpc(name, command).await {
            Err(e)
                if matches!(command, "start" | "status")
                    && matches!(
                        e.kind(),
                        io::ErrorKind::UnexpectedEof | io::ErrorKind::BrokenPipe
                    ) =>
            {
                sleep(Duration::from_millis(25)).await;
            }
            result => return result,
        }
    }
}

fn process_running(pid: u32) -> bool {
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if process.is_null() {
            return false;
        }
        let running = WaitForSingleObject(process, 0) == WAIT_TIMEOUT;
        CloseHandle(process);
        running
    }
}

async fn control(name: &str, command: &str) -> io::Result<Value> {
    match startup_rpc(name, command).await {
        Ok(mut value) => {
            if command == "stop" && value["ok"] == true {
                if let Some(pid) = value["pid"].as_u64() {
                    while process_running(pid as u32) {
                        sleep(Duration::from_millis(25)).await;
                    }
                }
                value["running"] = false.into();
                value["visible"] = false.into();
                value["pid"] = Value::Null;
            }
            Ok(value)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if matches!(command, "status" | "stop") {
                return Ok(response(command, false, false));
            }
            if command != "start" {
                return Ok(failure(command, "NOT_RUNNING", "start the component first"));
            }
            let mut child = spawn_host()?;
            loop {
                match startup_rpc(name, "start").await {
                    Ok(value) => return Ok(value),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
                if let Some(status) = child.try_wait()? {
                    // A competing host exits successfully after finding the existing owner.
                    // Keep probing that owner instead of reporting a failed launch.
                    if !status.success() {
                        return Ok(failure(
                            command,
                            "START_FAILED",
                            format!("host exited: {status}"),
                        ));
                    }
                }
                sleep(Duration::from_millis(50)).await;
            }
        }
        Err(e) => Err(e),
    }
}

struct StartLock(HANDLE);

impl StartLock {
    fn acquire(pipe_name: &str, milliseconds: u32) -> io::Result<Self> {
        let name: Vec<u16> = format!("Local\\{}-start", pipe_name.replace('\\', "_"))
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            let handle = CreateMutexW(ptr::null(), 0, name.as_ptr());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            match WaitForSingleObject(handle, milliseconds) {
                WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Self(handle)),
                result => {
                    let error = if result == WAIT_TIMEOUT {
                        io::Error::new(
                            io::ErrorKind::TimedOut,
                            "another start is still in progress",
                        )
                    } else {
                        io::Error::last_os_error()
                    };
                    CloseHandle(handle);
                    Err(error)
                }
            }
        }
    }
}

impl Drop for StartLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

fn spawn_host() -> io::Result<std::process::Child> {
    // Giving the child NUL stdio alone does not exclude other inherited pipe handles.
    // A lingering capture handle would make the controller wait until the GUI exits.
    let mut inherited = Vec::new();
    unsafe {
        for kind in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let handle = GetStdHandle(kind);
            let mut flags = 0;
            if GetHandleInformation(handle, &mut flags) != 0 && flags & HANDLE_FLAG_INHERIT != 0 {
                if SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) == 0 {
                    return Err(io::Error::last_os_error());
                }
                inherited.push((handle, flags));
            }
        }
    }
    let result = (|| {
        Command::new(std::env::current_exe()?)
            .arg("--component-host")
            .current_dir(data_dir().map_err(io::Error::other)?)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
    })();
    for (handle, flags) in inherited {
        unsafe {
            SetHandleInformation(handle, HANDLE_FLAG_INHERIT, flags & HANDLE_FLAG_INHERIT);
        }
    }
    result
}

fn print_result(value: Value) -> ! {
    let code = if value["ok"] == true {
        0
    } else {
        match value["error"]["code"].as_str() {
            Some("INVALID_ARGUMENT") => 2,
            Some("UNSUPPORTED_PROTOCOL") => 3,
            Some("NOT_RUNNING") => 4,
            Some("TIMEOUT") => 5,
            Some("START_FAILED") => 6,
            Some("WINDOW_ERROR") => 9,
            _ => 8,
        }
    };
    println!("{value}");
    std::process::exit(code)
}

pub struct Host {
    runtime: Runtime,
    pipe: NamedPipeServer,
    name: String,
    sid: String,
}

/// Called before Tauri creates any windows; the first pipe instance is the owner lock.
pub fn entry() -> Host {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sid = current_sid().unwrap_or_else(|e| print_result(failure("", "IO_ERROR", e)));
    let name = pipe_name(&sid).unwrap_or_else(|e| print_result(failure("", "IO_ERROR", e)));
    let runtime = runtime();
    if !args.is_empty() && args != ["--component-host"] {
        // GUI builds have no console by default; preserve redirected handles from callers.
        unsafe {
            let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
            if stdout.is_null() || stdout == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
        let command = args.first().map(String::as_str).unwrap_or("");
        let mut milliseconds = 15000;
        let valid = command_valid(command)
            && args.get(1).map(String::as_str) == Some("--json")
            && (args.len() == 2
                || (args.len() == 4
                    && args[2] == "--timeout-ms"
                    && args[3].parse::<u64>().is_ok_and(|n| {
                        milliseconds = n;
                        (100..=15000).contains(&n)
                    })));
        if !valid {
            print_result(failure(
                command,
                "INVALID_ARGUMENT",
                "expected command --json [--timeout-ms 100..15000]",
            ));
        }
        let deadline = Instant::now() + Duration::from_millis(milliseconds);
        let start_lock = if command == "start" {
            Some(
                StartLock::acquire(&name, milliseconds as u32).unwrap_or_else(|e| {
                    let code = if e.kind() == io::ErrorKind::TimedOut {
                        "TIMEOUT"
                    } else {
                        "IO_ERROR"
                    };
                    print_result(failure(command, code, e))
                }),
            )
        } else {
            None
        };
        let value = runtime.block_on(async {
            timeout(
                deadline.saturating_duration_since(Instant::now()),
                control(&name, command),
            )
            .await
        });
        drop(start_lock);
        print_result(match value {
            Ok(Ok(value)) => value,
            Ok(Err(e)) => failure(command, "IO_ERROR", e),
            Err(_) => failure(
                command,
                "TIMEOUT",
                "operation exceeded its deadline; inspect status before retry",
            ),
        });
    }
    let pipe = {
        let _guard = runtime.enter();
        match create_pipe(&name, &sid, true) {
            Ok(pipe) => pipe,
            Err(_) => {
                // A direct repeat launch is a no-op, never opens a second UI or steals focus.
                let result = runtime.block_on(async {
                    timeout(Duration::from_secs(15), startup_rpc(&name, "status")).await
                });
                if matches!(result, Ok(Ok(ref value)) if value["ok"] == true) {
                    std::process::exit(0);
                }
                print_result(failure(
                    "start",
                    "IO_ERROR",
                    "unable to own the component pipe",
                ));
            }
        }
    };
    Host {
        runtime,
        pipe,
        name,
        sid,
    }
}

fn dispatch(app: &tauri::AppHandle, request: Request) -> Value {
    if request.protocol != PROTOCOL {
        return failure(
            &request.command,
            "UNSUPPORTED_PROTOCOL",
            "expected octoppet-component@1",
        );
    }
    if !command_valid(&request.command) {
        return failure(
            &request.command,
            "INVALID_ARGUMENT",
            "unknown lifecycle command",
        );
    }
    let Some(pet) = app.get_webview_window("pet") else {
        return failure(
            &request.command,
            "WINDOW_ERROR",
            "pet window is unavailable",
        );
    };
    let operation = match request.command.as_str() {
        "show" => pet.show(),
        "hide" => {
            for label in ["chat", "settings"] {
                if let Some(window) = app.get_webview_window(label) {
                    let _ = window.hide();
                }
            }
            pet.hide()
        }
        _ => Ok(()),
    };
    match operation {
        Ok(()) => response(&request.command, true, pet.is_visible().unwrap_or(false)),
        Err(e) => failure(&request.command, "WINDOW_ERROR", e),
    }
}

impl Host {
    pub fn serve(self, app: tauri::AppHandle) {
        std::thread::spawn(move || {
            self.runtime.block_on(async move {
                let mut pipe = self.pipe;
                loop {
                    let connected = pipe.connect().await;
                    // Keep a fresh listener alive before releasing the previous instance.
                    // Reusing a disconnected overlapped pipe can retain stale read readiness.
                    let next = match create_pipe(&self.name, &self.sid, false) {
                        Ok(next) => next,
                        Err(_) => {
                            app.exit(8);
                            break;
                        }
                    };
                    if connected.is_err() {
                        let _ = pipe.disconnect();
                        pipe = next;
                        continue;
                    }
                    let result = timeout(Duration::from_secs(3), async {
                        let request: Request =
                            serde_json::from_slice(&read_frame(&mut pipe).await?)?;
                        let (send, receive) = mpsc::sync_channel(1);
                        let handle = app.clone();
                        app.run_on_main_thread(move || {
                            let _ = send.send(dispatch(&handle, request));
                        })
                        .map_err(io::Error::other)?;
                        let value = receive
                            .recv_timeout(Duration::from_secs(2))
                            .map_err(io::Error::other)?;
                        write_frame(&mut pipe, &value).await?;
                        let _ = pipe.read_u8().await?;
                        Ok::<bool, io::Error>(value["ok"] == true && value["command"] == "stop")
                    })
                    .await;
                    let _ = pipe.disconnect();
                    pipe = next;
                    if matches!(result, Ok(Ok(true))) {
                        app.exit(0);
                        break;
                    }
                }
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_lifecycle_commands_are_accepted() {
        for command in ["status", "start", "stop", "show", "hide"] {
            assert!(command_valid(command));
        }
        for command in ["exec", "shell", "window activate", ""] {
            assert!(!command_valid(command));
        }
    }

    #[test]
    fn stopped_status_has_no_process_id() {
        let value = response("status", false, false);
        assert_eq!(value["protocol"], PROTOCOL);
        assert_eq!(value["id"], "octoppet");
        assert!(value["pid"].is_null());
    }

    #[test]
    fn failure_is_structured() {
        let value = failure("show", "NOT_RUNNING", "start first");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "NOT_RUNNING");
    }
}
