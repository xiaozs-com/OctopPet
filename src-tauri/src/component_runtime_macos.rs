//! macOS current-user lifecycle control for the `octoppet-component@1`
//! component protocol.
//!
//! Mirrors `component_runtime.rs` (Windows) so a calling helper sees the same
//! commands, JSON payloads, error codes and exit codes. Only the transport
//! differs: Windows owns a current-user named pipe, macOS binds a Unix domain
//! socket inside the user data directory, which gives the same per-user
//! isolation because that directory is created mode 0700.
//!
//! Scope mapping (Windows -> macOS):
//!   named pipe ACL          -> per-user `$TMPDIR` (mode 0700) plus socket mode 0600
//!   user SID                -> owning uid, encoded in the socket file name
//!   data directory hash     -> the same FNV-1a hash, encoded in the file name
//!   dev/release split       -> `dev`/`release` socket file name segment
//!   logon session           -> not reproduced; see `socket_path`
//!   `GetNamedPipeServerProcessId` -> `LOCAL_PEERPID` peer credential check
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, PermissionsExt},
        io::{AsRawFd, RawFd},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use tauri::Manager;

pub const PROTOCOL: &str = "octoppet-component@1";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_FRAME: usize = 8192;
/// Server-side bound for reading a request and writing a reply.
const IO_DEADLINE: Duration = Duration::from_secs(3);
/// Server-side bound for the window work handed to the main thread.
const MAIN_THREAD_DEADLINE: Duration = Duration::from_secs(2);
/// `sun_path` is 104 bytes including the terminating NUL on macOS.
const MAX_SOCKET_PATH: usize = 100;

// <sys/file.h>
const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;
// <sys/un.h>
const SOL_LOCAL: i32 = 0;
const LOCAL_PEERPID: i32 = 0x002;
// <sys/signal.h>
const EPERM: i32 = 1;

extern "C" {
    fn flock(fd: RawFd, operation: i32) -> i32;
    fn kill(pid: i32, signal: i32) -> i32;
    fn getuid() -> u32;
    fn getsockopt(
        socket: RawFd,
        level: i32,
        option_name: i32,
        option_value: *mut core::ffi::c_void,
        option_len: *mut u32,
    ) -> i32;
}

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    protocol: String,
    command: String,
}

pub fn data_dir() -> Result<PathBuf, String> {
    let path = std::env::var_os("OCTOPPET_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join("Library/Application Support/com.octop.pet"))
        })
        .ok_or("HOME is unavailable")?;
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
    // 0700 stands in for the Windows current-user-only pipe ACL.
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&path)
        .map_err(|e| e.to_string())?;
    Ok(path)
}

/// Socket directory. macOS points `TMPDIR` at a per-user directory with mode
/// 0700, so another user cannot pre-create the socket. It is also short, which
/// matters because `sun_path` is only 104 bytes including the terminator.
fn socket_dir() -> PathBuf {
    std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_dir())
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Fixed FNV-1a scope hash over the data directory, matching the Windows
/// implementation so the same directory always maps to the same endpoint.
fn scope_hash() -> Result<u64, String> {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in data_dir()?.to_string_lossy().to_lowercase().as_bytes() {
        hash = (hash ^ *byte as u64).wrapping_mul(0x100000001b3);
    }
    Ok(hash)
}

/// Socket name. Keeping the endpoint short means the length of the user data
/// directory cannot make the component unstartable.
///
/// The Windows implementation also folds in the logon session id. macOS has no
/// supported equivalent for a GUI session, and guessing one would make the
/// client and host disagree about the path, so it is deliberately omitted.
fn socket_path() -> Result<PathBuf, String> {
    let mode = if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    };
    let uid = unsafe { getuid() };
    let hash = scope_hash()?;
    let path = socket_dir().join(format!("octoppet-{uid}-{mode}-{hash:016x}.sock"));
    if path.as_os_str().as_bytes().len() > MAX_SOCKET_PATH {
        return Err("socket path is too long for this system".into());
    }
    Ok(path)
}

fn response(command: &str, running: bool, visible: bool) -> Value {
    json!({"protocol":PROTOCOL,"id":"octoppet","version":VERSION,"ok":true,
        "command":command,"running":running,"visible":visible,
        "pid":if running {Some(std::process::id())} else {None},"error":null,
        "data_dir":data_dir().ok(),
        "control_pipe":socket_path().ok().map(|p| p.to_string_lossy().into_owned())})
}

fn failure(command: &str, code: &str, message: impl ToString) -> Value {
    json!({"protocol":PROTOCOL,"id":"octoppet","version":VERSION,"ok":false,
        "command":command,"error":{"code":code,"message":message.to_string()}})
}

fn command_valid(command: &str) -> bool {
    matches!(command, "status" | "start" | "stop" | "show" | "hide")
}

fn read_frame<R: Read>(stream: &mut R) -> io::Result<Vec<u8>> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header)?;
    let len = u32::from_le_bytes(header) as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame size",
        ));
    }
    let mut frame = vec![0; len];
    stream.read_exact(&mut frame)?;
    Ok(frame)
}

fn write_frame<W: Write>(stream: &mut W, value: &Value) -> io::Result<()> {
    let frame = serde_json::to_vec(value)?;
    if frame.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "response too large",
        ));
    }
    stream.write_all(&(frame.len() as u32).to_le_bytes())?;
    stream.write_all(&frame)?;
    stream.flush()
}

/// A socket file with no listener is a crashed host, not a running one, so it
/// maps onto the same "not found" outcome the Windows transport reports.
fn connect(path: &Path) -> io::Result<UnixStream> {
    match UnixStream::connect(path) {
        Ok(stream) => Ok(stream),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            Err(io::Error::new(io::ErrorKind::NotFound, error.to_string()))
        }
        Err(error) => Err(error),
    }
}

fn peer_pid(stream: &UnixStream) -> io::Result<u32> {
    let mut pid: i32 = 0;
    let mut len = size_of::<i32>() as u32;
    let result = unsafe {
        getsockopt(
            stream.as_raw_fd(),
            SOL_LOCAL,
            LOCAL_PEERPID,
            (&mut pid as *mut i32).cast(),
            &mut len,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(pid as u32)
}

fn rpc(path: &Path, command: &str) -> io::Result<Value> {
    let mut client = connect(path)?;
    client.set_read_timeout(Some(IO_DEADLINE))?;
    client.set_write_timeout(Some(IO_DEADLINE))?;
    write_frame(&mut client, &json!({"protocol":PROTOCOL,"command":command}))?;
    let value: Value = serde_json::from_slice(&read_frame(&mut client)?)?;
    if value["protocol"] != PROTOCOL || value["id"] != "octoppet" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incompatible response",
        ));
    }
    // Confirm the reply came from the process that actually owns the socket.
    let server_pid = peer_pid(&client)?;
    if value["running"] == true && value["pid"].as_u64() != Some(server_pid as u64) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "process identity mismatch",
        ));
    }
    // Keep the server connected until its entire reply has been consumed.
    client.write_all(&[1])?;
    Ok(value)
}

// Startup can briefly refuse or drop a connection before the GUI event loop is
// ready. Retry only idempotent probes; the caller's total deadline bounds this.
fn startup_rpc(path: &Path, command: &str, deadline: Instant) -> io::Result<Value> {
    loop {
        match rpc(path, command) {
            Err(error)
                if matches!(command, "start" | "status")
                    && matches!(
                        error.kind(),
                        io::ErrorKind::UnexpectedEof
                            | io::ErrorKind::BrokenPipe
                            | io::ErrorKind::WouldBlock
                    ) =>
            {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "startup probe exceeded its deadline",
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            result => return result,
        }
    }
}

fn process_running(pid: u32) -> bool {
    let result = unsafe { kill(pid as i32, 0) };
    if result == 0 {
        return true;
    }
    // EPERM means the process exists but belongs to someone else.
    io::Error::last_os_error().raw_os_error() == Some(EPERM)
}

fn timed_out(command: &str) -> Value {
    failure(
        command,
        "TIMEOUT",
        "operation exceeded its deadline; inspect status before retry",
    )
}

fn control(path: &Path, command: &str, deadline: Instant) -> io::Result<Value> {
    match startup_rpc(path, command, deadline) {
        Ok(mut value) => {
            if command == "stop" && value["ok"] == true {
                if let Some(pid) = value["pid"].as_u64() {
                    while process_running(pid as u32) {
                        if Instant::now() >= deadline {
                            return Ok(timed_out(command));
                        }
                        std::thread::sleep(Duration::from_millis(25));
                    }
                }
                value["running"] = false.into();
                value["visible"] = false.into();
                value["pid"] = Value::Null;
            }
            Ok(value)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if matches!(command, "status" | "stop") {
                return Ok(response(command, false, false));
            }
            if command != "start" {
                return Ok(failure(command, "NOT_RUNNING", "start the component first"));
            }
            let mut child = spawn_host()?;
            loop {
                if Instant::now() >= deadline {
                    return Ok(timed_out(command));
                }
                match startup_rpc(path, "start", deadline) {
                    Ok(value) => return Ok(value),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                        return Ok(timed_out(command))
                    }
                    Err(error) => return Err(error),
                }
                if let Some(status) = child.try_wait()? {
                    // A competing host exits successfully after finding the existing
                    // owner. Keep probing that owner instead of reporting failure.
                    if !status.success() {
                        return Ok(failure(
                            command,
                            "START_FAILED",
                            format!("host exited: {status}"),
                        ));
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::TimedOut => Ok(timed_out(command)),
        Err(error) => Err(error),
    }
}

/// Serialises concurrent starts. `flock` is released by the kernel when the
/// process exits, matching the Windows named mutex on abnormal exit.
struct StartLock {
    _file: fs::File,
}

impl StartLock {
    fn acquire(socket: &Path, deadline: Instant) -> io::Result<Self> {
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(socket.with_extension("start.lock"))?;
        loop {
            if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0 {
                return Ok(Self { _file: file });
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::WouldBlock {
                return Err(error);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "another start is still in progress",
                ));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

fn spawn_host() -> io::Result<std::process::Child> {
    Command::new(std::env::current_exe()?)
        .arg("--component-host")
        .current_dir(data_dir().map_err(io::Error::other)?)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
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
    listener: UnixListener,
    path: PathBuf,
}

/// Take ownership of the component socket. Returns `Err` with `AddrInUse` when
/// a live host already owns it, which the caller treats as a repeat launch.
fn bind_socket(path: &Path) -> io::Result<UnixListener> {
    if UnixStream::connect(path).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "another instance owns the component socket",
        ));
    }
    // Only a socket left behind by a crashed host can be removed here.
    let _ = fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    Ok(listener)
}

/// Called before Tauri creates any windows; owning the socket is the owner lock.
pub fn entry() -> Host {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = socket_path().unwrap_or_else(|e| print_result(failure("", "IO_ERROR", e)));
    if !args.is_empty() && args != ["--component-host"] {
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
            Some(StartLock::acquire(&path, deadline).unwrap_or_else(|e| {
                let code = if e.kind() == io::ErrorKind::TimedOut {
                    "TIMEOUT"
                } else {
                    "IO_ERROR"
                };
                print_result(failure(command, code, e))
            }))
        } else {
            None
        };
        let value = control(&path, command, deadline);
        drop(start_lock);
        print_result(match value {
            Ok(value) => value,
            Err(e) => failure(command, "IO_ERROR", e),
        });
    }
    let listener = match bind_socket(&path) {
        Ok(listener) => listener,
        Err(_) => {
            // A direct repeat launch is a no-op, never opens a second UI or steals focus.
            match startup_rpc(&path, "status", Instant::now() + Duration::from_secs(15)) {
                Ok(ref value) if value["ok"] == true => std::process::exit(0),
                _ => print_result(failure(
                    "start",
                    "IO_ERROR",
                    "unable to own the component socket",
                )),
            }
        }
    };
    Host { listener, path }
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
            let path = self.path;
            for stream in self.listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let value = match handle_request(&app, &mut stream) {
                    Ok(value) => value,
                    Err(_) => {
                        drop(stream);
                        continue;
                    }
                };
                let stopped = value["ok"] == true && value["command"] == "stop";
                drop(stream);
                if stopped {
                    // Remove the socket we still own so the next start binds cleanly.
                    let _ = fs::remove_file(&path);
                    app.exit(0);
                    break;
                }
            }
        });
    }
}

fn handle_request(app: &tauri::AppHandle, stream: &mut UnixStream) -> io::Result<Value> {
    stream.set_read_timeout(Some(IO_DEADLINE))?;
    stream.set_write_timeout(Some(IO_DEADLINE))?;
    let request: Request = serde_json::from_slice(&read_frame(stream)?)?;
    let (send, receive) = mpsc::sync_channel(1);
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = send.send(dispatch(&handle, request));
    })
    .map_err(io::Error::other)?;
    let value = receive
        .recv_timeout(MAIN_THREAD_DEADLINE)
        .map_err(io::Error::other)?;
    write_frame(stream, &value)?;
    let mut ack = [0u8; 1];
    let _ = stream.read_exact(&mut ack);
    Ok(value)
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

    #[test]
    fn frames_round_trip_and_reject_oversized_payloads() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, &json!({"hello":"world"})).expect("write");
        let mut cursor = io::Cursor::new(buffer);
        let frame = read_frame(&mut cursor).expect("read");
        assert_eq!(
            serde_json::from_slice::<Value>(&frame).expect("json")["hello"],
            "world"
        );

        let mut oversized = Vec::new();
        oversized.extend_from_slice(&((MAX_FRAME as u32) + 1).to_le_bytes());
        assert!(read_frame(&mut io::Cursor::new(oversized)).is_err());
    }
}
