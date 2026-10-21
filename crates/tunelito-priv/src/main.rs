#![cfg_attr(windows, windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::Parser;
use tunelito_core::access::{write_linux_access_files, write_macos_access_files, AccessRequest};
#[cfg(unix)]
use tunelito_core::linux::{render_polkit_rule, POLKIT_RULE, PRIV_PATH};
use tunelito_core::plan::{
    activate_commands, disable_commands, install_commands, reload_command, Probe,
};
use tunelito_core::sshd::includes_dropins;
use tunelito_core::windows::{write_windows_access_files, WindowsFilePlan};
use tunelito_core::Error;

#[cfg(unix)]
mod mac;
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
        /// Write the macOS layout. On macOS this is the default, and `--execute` runs it.
        #[arg(long)]
        macos: bool,
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
            macos,
            result,
        } => {
            let macos = macos || cfg!(target_os = "macos");
            let outcome = apply(&request, &root, execute, windows, macos);
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
    macos: bool,
) -> Result<(), Error> {
    // The request may sit in a directory a less trusted account can write.
    // Refuse a link there instead of reading whatever it points at.
    if fs::symlink_metadata(request_path)?.file_type().is_symlink() {
        return Err(Error::prepare_failed("refusing a linked request file"));
    }
    // Started through pkexec, only the real system and only `--execute`. A
    // polkit rule may let the desktop user do this without a password, so
    // files-only mode must not become a way to create directories as root
    // anywhere else. pkexec always sets PKEXEC_UID; the caller cannot clear it.
    if std::env::var_os("PKEXEC_UID").is_some() && (!execute || root != Path::new("/")) {
        return Err(Error::prepare_failed(
            "through pkexec, tunelito-priv only runs --execute against /",
        ));
    }
    let text = fs::read_to_string(request_path)?;
    let request: AccessRequest = serde_json::from_str(&text)
        .map_err(|err| Error::with_detail("prepare_failed", err.to_string()))?;
    if windows {
        return apply_windows(&request, root, execute);
    }
    if macos {
        return apply_macos(&request, root, execute);
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
        if request.dropin.is_some() {
            let config = fs::read_to_string("/etc/ssh/sshd_config")
                .or_else(|_| fs::read_to_string("/usr/etc/ssh/sshd_config"))
                .unwrap_or_default();
            if !includes_dropins(&config) {
                return Err(Error::prepare_failed(
                    "sshd_config does not include /etc/ssh/sshd_config.d, so the tunelito settings would be ignored",
                ));
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
        // SELinux (Fedora): files root creates under the home get the wrong
        // label, and sshd then refuses the key.
        if Path::new("/usr/sbin/restorecon").exists() {
            let _ = Command::new("/usr/sbin/restorecon")
                .args(["-R", "/home/tunelito/.ssh"])
                .status();
        }
        #[cfg(unix)]
        if request.prepare {
            write_polkit_rule();
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

#[cfg(unix)]
/// After a setup the person approved through pkexec, let that same person
/// switch again without a password. Only for the packaged helper path the
/// polkit action names; a failure here leaves the password prompt in place.
fn write_polkit_rule() {
    let Ok(uid) = std::env::var("PKEXEC_UID") else {
        return;
    };
    if std::env::current_exe().ok().as_deref() != Some(Path::new(PRIV_PATH)) {
        return;
    }
    let rules_dir = Path::new(POLKIT_RULE).parent().unwrap_or(Path::new("/"));
    if !rules_dir.is_dir() || uid.is_empty() || !uid.bytes().all(|b| b.is_ascii_digit()) {
        return;
    }
    let Some(login) = Command::new("id")
        .args(["-nu", &uid])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
    else {
        return;
    };
    let Ok(rule) = render_polkit_rule(&login) else {
        return;
    };
    if fs::write(POLKIT_RULE, rule).is_ok() {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(POLKIT_RULE, fs::Permissions::from_mode(0o644));
    }
}

fn apply_macos(request: &AccessRequest, root: &Path, execute: bool) -> Result<(), Error> {
    if execute {
        if root != Path::new("/") {
            return Err(Error::prepare_failed(
                "refusing to run system commands against a fake root",
            ));
        }
        #[cfg(unix)]
        {
            return mac::execute(request);
        }
        #[cfg(not(unix))]
        {
            return Err(Error::prepare_failed(
                "refusing to run macOS commands on this operating system",
            ));
        }
    }
    let written = write_macos_access_files(root, request)?;
    for path in &written {
        println!("{}", path.display());
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
    let stderr = strip_powershell_noise(&String::from_utf8_lossy(&output.stderr));
    let stdout = strip_powershell_noise(&String::from_utf8_lossy(&output.stdout));
    let mut detail = if !stdout.is_empty() { stdout } else { stderr };
    if detail.len() > 350 {
        let mut end = 350;
        while !detail.is_char_boundary(end) {
            end -= 1;
        }
        detail.truncate(end);
    }
    Err(Error::prepare_failed(format!("{}: {detail}", step.purpose)))
}

fn strip_powershell_noise(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("#< CLIXML")
                && !line.starts_with("<Objs")
                && !line.contains("Preparing modules for first use")
        })
        .collect::<Vec<_>>()
        .join(" ")
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
    probe.linux_packages = tunelito_core::linux::Packages::detect(|program| {
        Command::new("sh")
            .args(["-c", "command -v \"$1\" >/dev/null 2>&1", "sh", program])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    });
    probe.ssh_unit = probe.linux_packages.ssh_unit().into();
    probe.openssh_installed = Command::new("sh")
        .args([
            "-c",
            "command -v sshd >/dev/null || test -x /usr/sbin/sshd || dpkg -s openssh-server >/dev/null 2>&1",
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
