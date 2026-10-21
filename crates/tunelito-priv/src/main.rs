#![cfg_attr(windows, windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::Parser;
use tunelito_core::access::{write_linux_access_files, AccessRequest};
use tunelito_core::plan::{
    activate_commands, disable_commands, install_commands, reload_command, Probe,
};
use tunelito_core::windows::{write_windows_access_files, WindowsFilePlan};
use tunelito_core::Error;

#[cfg(windows)]
mod win;

/// Apply a validated access request. `--execute` is refused unless `--root /` and euid is 0.
#[derive(Debug, Parser)]
#[command(name = "tunelito-priv", version)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, clap::Subcommand)]
enum Cmd {
    Apply {
        #[arg(long)]
        request: PathBuf,
        #[arg(long, default_value = "/")]
        root: PathBuf,
        /// Run system commands. Refused when root is not the real system root or the process is not privileged.
        #[arg(long)]
        execute: bool,
        /// Write the Windows OpenSSH layout. On Windows, `--execute` runs it.
        #[arg(long)]
        windows: bool,
        /// JSON `{"ok":true|false,"detail":"..."}` for the app that launched us without a console.
        #[arg(long)]
        result: Option<PathBuf>,
    },
    /// Remove the dedicated user, the scheduled task, and the managed sshd block.
    /// Does not remove the OpenSSH capability.
    Uninstall {
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        result: Option<PathBuf>,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("tunelito-priv: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Apply {
            request,
            root,
            execute,
            windows,
            result,
        } => {
            let outcome = apply(&request, &root, execute, windows);
            write_result(result.as_deref(), &outcome);
            outcome
        }
        Cmd::Uninstall { execute, result } => {
            let outcome = uninstall(execute);
            write_result(result.as_deref(), &outcome);
            outcome
        }
    }
}

fn write_result(path: Option<&Path>, outcome: &Result<(), Error>) {
    let Some(path) = path else {
        return;
    };
    let (ok, detail) = match outcome {
        Ok(()) => (true, String::new()),
        Err(err) => (false, err.to_string()),
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let body = serde_json::json!({ "ok": ok, "detail": detail });
    let _ = fs::write(path, body.to_string());
}

fn apply(
    request_path: &std::path::Path,
    root: &std::path::Path,
    execute: bool,
    windows: bool,
) -> Result<(), Error> {
    let text = fs::read_to_string(request_path)?;
    let request: AccessRequest = serde_json::from_str(&text)
        .map_err(|err| Error::with_detail("prepare_failed", err.to_string()))?;
    if windows {
        return apply_windows(&request, root, execute);
    }
    if execute {
        if root != std::path::Path::new("/") {
            return Err(Error::prepare_failed(
                "refusing to run system commands against a fake root",
            ));
        }
        if !is_root() {
            return Err(Error::needs_admin());
        }
        if request.prepare {
            let probe = probe_linux();
            for step in install_commands(&probe)? {
                run_step(&step)?;
            }
        }
    }
    let written = write_linux_access_files(root, &request)?;
    for path in &written {
        println!("{}", path.display());
    }
    if execute {
        if Path::new("/home/tunelito/.ssh").exists() {
            let _ = Command::new("chown")
                .args(["-R", "tunelito:tunelito", "/home/tunelito/.ssh"])
                .status();
        }
        if let Some(dropin) = written
            .iter()
            .find(|path| path.ends_with("00-tunelito.conf"))
        {
            let _ = Command::new("chown").arg("root:root").arg(dropin).status();
        }
        let probe = probe_linux();
        if request.reload_sshd {
            run_step(&reload_command(&probe)?)?;
        }
        if request.start_sshd {
            let mut starting = probe.clone();
            starting.sshd_active = false;
            for step in activate_commands(&starting)? {
                run_step(&step)?;
            }
        }
        if request.stop_sshd {
            for step in disable_commands(&probe, true, true)? {
                run_step(&step)?;
            }
        }
    }
    Ok(())
}

fn apply_windows(request: &AccessRequest, root: &Path, execute: bool) -> Result<(), Error> {
    if execute {
        #[cfg(windows)]
        {
            let _ = root;
            return win::execute(request);
        }
        #[cfg(not(windows))]
        {
            let _ = (request, root);
            return Err(Error::prepare_failed(
                "refusing to run Windows commands on this operating system",
            ));
        }
    }
    let plan = WindowsFilePlan {
        set_global_max_sessions: request.windows_set_global_max_sessions,
        add_sftp: request.windows_add_sftp,
        account_is_admin: request.windows_account_is_admin,
        authorized_key_lines: request.authorized_key_lines.clone(),
        write_keys: request.write_keys,
    };
    let written = write_windows_access_files(root, &plan)?;
    for path in &written {
        println!("{}", path.display());
    }
    Ok(())
}

fn uninstall(execute: bool) -> Result<(), Error> {
    if !execute {
        return Ok(());
    }
    #[cfg(windows)]
    {
        return win::uninstall();
    }
    #[cfg(not(windows))]
    {
        Err(Error::prepare_failed(
            "refusing to run Windows uninstall on this operating system",
        ))
    }
}

fn run_step(step: &tunelito_core::plan::CommandStep) -> Result<(), Error> {
    let mut command = Command::new(&step.program);
    command
        .args(&step.args)
        .envs(step.env.iter().cloned())
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command
        .output()
        .map_err(|err| Error::prepare_failed(format!("{}: {err}", step.purpose)))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut detail = stderr.trim().to_string();
    if detail.is_empty() {
        detail = stdout.trim().to_string();
    }
    if detail.len() > 300 {
        let mut start = detail.len() - 300;
        while !detail.is_char_boundary(start) {
            start += 1;
        }
        detail = detail[start..].to_string();
    }
    Err(Error::prepare_failed(format!("{}: {detail}", step.purpose)))
}

fn systemctl_is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim() == "0")
        .unwrap_or(false)
}

fn probe_linux() -> Probe {
    let mut probe = Probe::linux_fresh();
    probe.groups_present.clear();
    probe.openssh_installed = Command::new("sh")
        .args([
            "-c",
            "command -v sshd >/dev/null || dpkg -s openssh-server >/dev/null 2>&1",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    probe.user_exists = Command::new("id")
        .args(["-u", "tunelito"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    probe.sshd_active = systemctl_is_active("ssh") || systemctl_is_active("sshd");
    for group in ["plugdev", "adbusers"] {
        let present = Command::new("getent")
            .args(["group", group])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if present {
            probe.groups_present.push(group.into());
        }
    }
    probe.groups_present.dedup();
    probe
}
