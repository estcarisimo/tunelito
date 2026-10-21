//! One UAC prompt on first use, then a scheduled task so later switches stay quiet.
//! The window itself stays unelevated. No console is shown.

use std::fs;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use tunelito_core::access::AccessRequest;
use tunelito_core::windows::{REQUEST_FILE, RESULT_FILE, TASK_NAME};
use tunelito_core::Error;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn apply(request: &AccessRequest) -> Result<(), Error> {
    ensure_setup()?;
    dispatch(request)
}

pub fn uninstall_exit() -> i32 {
    let Ok(bin) = priv_path() else {
        return 1;
    };
    match Command::new(bin)
        .args(["uninstall", "--execute"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => 0,
        _ => 1,
    }
}

pub fn spawn_setup() {
    thread::spawn(|| {
        let _ = ensure_setup();
    });
}

fn ensure_setup() -> Result<(), Error> {
    if task_registered() || windows_elevated() {
        return Ok(());
    }
    let dir = work_dir()?;
    fs::create_dir_all(&dir)?;
    let lock_path = dir.join("setup.lock");
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(mut lock) => {
            let _ = writeln!(lock, "setup");
            let outcome = setup_elevated();
            let _ = fs::remove_file(&lock_path);
            outcome
        }
        Err(_) => {
            for _ in 0..180 {
                if task_registered() {
                    return Ok(());
                }
                thread::sleep(Duration::from_secs(1));
            }
            Err(Error::needs_admin())
        }
    }
}

fn setup_elevated() -> Result<(), Error> {
    let request = AccessRequest {
        authorized_key_lines: Vec::new(),
        dropin: None,
        write_keys: false,
        prepare: true,
        start_sshd: false,
        stop_sshd: false,
        reload_sshd: false,
        windows_account_is_admin: false,
        windows_set_global_max_sessions: true,
        windows_add_sftp: true,
    };
    dispatch_direct(&request, true)
}

fn dispatch(request: &AccessRequest) -> Result<(), Error> {
    if windows_elevated() || !task_registered() {
        return dispatch_direct(request, !windows_elevated());
    }
    fs::create_dir_all(r"C:\ProgramData\tunelito")?;
    let _ = fs::remove_file(RESULT_FILE);
    fs::write(
        REQUEST_FILE,
        serde_json::to_vec(request).map_err(|err| Error::with_detail("io", err.to_string()))?,
    )?;
    let status = Command::new("schtasks")
        .args(["/Run", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .status()
        .map_err(|err| Error::prepare_failed(err.to_string()))?;
    if !status.success() {
        return dispatch_direct(request, true);
    }
    wait_result(Path::new(RESULT_FILE))
}

fn dispatch_direct(request: &AccessRequest, elevate: bool) -> Result<(), Error> {
    let bin = priv_path()?;
    let dir = work_dir()?;
    fs::create_dir_all(&dir)?;
    let request_path = dir.join("request.json");
    let result_path = dir.join("result.json");
    let _ = fs::remove_file(&result_path);
    fs::write(
        &request_path,
        serde_json::to_vec(request).map_err(|err| Error::with_detail("io", err.to_string()))?,
    )?;
    let status = if elevate {
        elevate_priv(&bin, &request_path, &result_path)?
    } else {
        Command::new(&bin)
            .args([
                "apply",
                "--request",
                request_path.to_str().unwrap_or(""),
                "--root",
                r"C:\",
                "--execute",
                "--windows",
                "--result",
                result_path.to_str().unwrap_or(""),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .status()
            .map_err(|err| Error::prepare_failed(err.to_string()))?
    };
    if !status.success() && !result_path.exists() {
        return Err(Error::needs_admin());
    }
    wait_result(&result_path)
}

fn elevate_priv(
    bin: &Path,
    request: &Path,
    result: &Path,
) -> Result<std::process::ExitStatus, Error> {
    let script = format!(
        "$p = Start-Process -FilePath {bin} -ArgumentList @('apply','--request',{req},'--root','C:\\','--execute','--windows','--result',{res}) -Verb RunAs -Wait -PassThru -WindowStyle Hidden; if ($null -eq $p) {{ exit 1 }}; exit $p.ExitCode",
        bin = ps_quote(bin),
        req = ps_quote(request),
        res = ps_quote(result),
    );
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .status()
        .map_err(|err| Error::prepare_failed(err.to_string()))
}

fn wait_result(path: &Path) -> Result<(), Error> {
    for _ in 0..180 {
        if path.exists() {
            let text = fs::read_to_string(path).unwrap_or_default();
            let parsed: serde_json::Value = serde_json::from_str(&text)
                .map_err(|err| Error::prepare_failed(err.to_string()))?;
            if parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Ok(());
            }
            let detail = parsed
                .get("detail")
                .and_then(|v| v.as_str())
                .unwrap_or("windows setup failed");
            return Err(Error::prepare_failed(detail));
        }
        thread::sleep(Duration::from_secs(1));
    }
    Err(Error::prepare_failed(
        "the privileged helper did not finish",
    ))
}

fn priv_path() -> Result<PathBuf, Error> {
    if let Some(path) = std::env::var_os("TUNELITO_PRIV_BIN") {
        return Ok(PathBuf::from(path));
    }
    if let Some(bytes) = crate::embed::embedded_priv() {
        let dest = work_dir()?.join("tunelito-priv.exe");
        if fs::read(&dest).ok().as_deref() != Some(bytes) {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&dest, bytes)?;
        }
        return Ok(dest);
    }
    let name = "tunelito-priv.exe";
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(Error::needs_admin)?;
    let candidate = dir.join(name);
    if candidate.exists() {
        return Ok(candidate);
    }
    Err(Error::needs_admin())
}

fn work_dir() -> Result<PathBuf, Error> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    Ok(base.join("tunelito"))
}

fn task_registered() -> bool {
    Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn windows_elevated() -> bool {
    Command::new("net")
        .arg("session")
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn ps_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}
