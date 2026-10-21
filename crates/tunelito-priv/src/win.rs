//! Windows execution. Compiled only for that target. No console window.

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use tunelito_core::access::AccessRequest;
use tunelito_core::cloudflared::OsKind;
use tunelito_core::plan::{
    activate_commands, disable_commands, install_commands, reload_command, Probe,
};
use tunelito_core::sshd::parse_sshd_effective;
use tunelito_core::windows::{
    acl_commands, register_task_command, strip_tunelito_block, uninstall_commands,
    write_windows_access_files, WindowsFilePlan, ADMIN_KEYS, PRIV_INSTALL, SSHD_CONFIG, USER_KEYS,
};
use tunelito_core::Error;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn execute(request: &AccessRequest) -> Result<(), Error> {
    if !is_elevated() {
        return Err(Error::needs_admin());
    }
    install_self()?;
    if request.prepare {
        let probe = probe_windows();
        for step in install_commands(&probe)? {
            super::run_step(&step)?;
        }
        if let Ok(step) = register_task_command(PRIV_INSTALL) {
            let _ = super::run_step(&step);
        }
    }
    let probe = probe_windows();
    if request.prepare || request.dropin.is_some() || request.write_keys {
        let mut plan = WindowsFilePlan::from_probe(
            &probe,
            if request.write_keys {
                request.authorized_key_lines.clone()
            } else {
                Vec::new()
            },
            request.write_keys,
        );
        if request.prepare && probe.max_sessions.unwrap_or(10) < 10 {
            plan.set_global_max_sessions = true;
        }
        if request.prepare && probe.max_sessions.is_none() {
            plan.set_global_max_sessions = true;
        }
        write_windows_access_files(Path::new(r"C:\"), &plan)?;
        for step in acl_commands(plan.account_is_admin) {
            super::run_step(&step)?;
        }
    }
    if request.reload_sshd {
        super::run_step(&reload_command(&probe)?)?;
    }
    if request.start_sshd {
        let mut starting = probe.clone();
        starting.sshd_active = false;
        for step in activate_commands(&starting)? {
            super::run_step(&step)?;
        }
    }
    if request.stop_sshd {
        for step in disable_commands(&probe, true, true)? {
            super::run_step(&step)?;
        }
    }
    Ok(())
}

pub fn uninstall() -> Result<(), Error> {
    if !is_elevated() {
        return Err(Error::needs_admin());
    }
    for step in uninstall_commands() {
        let _ = super::run_step(&step);
    }
    if let Ok(config) = fs::read_to_string(SSHD_CONFIG) {
        let stripped = strip_tunelito_block(&config);
        if stripped != config {
            let _ = fs::write(SSHD_CONFIG, stripped);
        }
    }
    let _ = fs::remove_file(USER_KEYS);
    let _ = fs::remove_file(ADMIN_KEYS);
    Ok(())
}

fn install_self() -> Result<(), Error> {
    let src = std::env::current_exe()?;
    let dest = Path::new(PRIV_INSTALL);
    if src == dest {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&src, dest)?;
    Ok(())
}

fn is_elevated() -> bool {
    ps_true(
        "([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)",
    )
}

fn probe_windows() -> Probe {
    let mut probe = Probe::linux_fresh();
    probe.os = OsKind::Windows;
    probe.groups_present.clear();
    probe.ssh_unit = "sshd".into();
    probe.openssh_installed =
        ps_true("[bool](Get-Service -Name sshd -ErrorAction SilentlyContinue)");
    probe.windows_capability_installed = probe.openssh_installed;
    probe.user_exists =
        ps_true("[bool](Get-LocalUser -Name 'tunelito' -ErrorAction SilentlyContinue)");
    probe.sshd_active =
        ps_true("(Get-Service -Name sshd -ErrorAction SilentlyContinue).Status -eq 'Running'");
    probe.windows_startup_automatic =
        ps_true("(Get-Service -Name sshd -ErrorAction SilentlyContinue).StartType -eq 'Automatic'");
    probe.windows_user_is_admin = ps_true(
        "$m = @(Get-LocalGroupMember -SID 'S-1-5-32-544' -ErrorAction SilentlyContinue); [bool]($m | Where-Object { $_.Name -like '*\\tunelito' })",
    );
    if let Ok(config) = fs::read_to_string(SSHD_CONFIG) {
        let parsed = parse_sshd_effective(&config);
        probe.sshd_config_read = true;
        probe.max_sessions = parsed.max_sessions;
        probe.sftp_enabled = parsed.sftp_enabled;
        probe.openssh_installed = true;
        probe.windows_capability_installed = true;
    }
    probe
}

fn ps_true(script: &str) -> bool {
    let Ok(output) = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim().eq_ignore_ascii_case("true")
}
