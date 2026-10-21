//! macOS execution. Built on every Unix so the Linux job compiles it, and
//! refused at run time anywhere but macOS.
//!
//! UNTESTED ON CI: nothing here runs on a Mac in CI. The command text comes from
//! `tunelito_core::macos` and is unit-tested there.

use std::fs;
use std::path::Path;
use std::process::Command;

use tunelito_core::access::{write_macos_access_files, AccessRequest};
use tunelito_core::cloudflared::OsKind;
use tunelito_core::macos::{
    includes_dropins, ACCESS_GROUP, ACCESS_GROUP_MARK, DROPIN, REMOTE_LOGIN_MARK, SSHD_CONFIG,
    SSHD_LABEL, STATE_DIR, USER_HOME,
};
use tunelito_core::plan::{
    activate_commands, disable_commands, install_commands, reload_command, Probe,
};
use tunelito_core::sshd::parse_sshd_effective;
use tunelito_core::Error;

pub fn execute(request: &AccessRequest) -> Result<(), Error> {
    if !cfg!(target_os = "macos") {
        return Err(Error::prepare_failed(
            "refusing to run macOS commands on this operating system",
        ));
    }
    if !super::is_root() {
        return Err(Error::needs_admin());
    }
    let config = fs::read_to_string(SSHD_CONFIG).unwrap_or_default();
    if !includes_dropins(&config) {
        return Err(Error::prepare_failed(
            "this macOS sshd_config does not include sshd_config.d; macOS 13 or later is needed",
        ));
    }
    fs::create_dir_all(STATE_DIR)?;
    if request.prepare {
        for step in install_commands(&probe_macos())? {
            super::run_step(&step)?;
        }
    }
    let written = write_macos_access_files(Path::new("/"), request)?;
    for path in &written {
        println!("{}", path.display());
    }
    let ssh_dir = Path::new(USER_HOME).join(".ssh");
    if ssh_dir.exists() {
        let _ = Command::new("/usr/sbin/chown")
            .args(["-R", "tunelito:staff"])
            .arg(&ssh_dir)
            .status();
    }
    if Path::new(DROPIN).exists() {
        let _ = Command::new("/usr/sbin/chown")
            .args(["root:wheel", DROPIN])
            .status();
    }
    let probe = probe_macos();
    if request.reload_sshd {
        super::run_step(&reload_command(&probe)?)?;
    }
    if request.start_sshd {
        for step in activate_commands(&probe)? {
            super::run_step(&step)?;
        }
    }
    if request.stop_sshd {
        // Read the markers again: a start in this same request may have set them.
        for step in disable_commands(&probe_macos(), true, true)? {
            super::run_step(&step)?;
        }
    }
    Ok(())
}

fn probe_macos() -> Probe {
    let mut probe = Probe::linux_fresh();
    probe.os = OsKind::Macos;
    probe.ssh_unit = "sshd".into();
    probe.groups_present.clear();
    probe.openssh_installed = true;
    probe.user_exists = succeeds("/usr/bin/id", &["-u", "tunelito"]);
    probe.remote_login_on = succeeds("/bin/launchctl", &["print", SSHD_LABEL]);
    probe.macos_access_group = succeeds(
        "/usr/bin/dscl",
        &[".", "-read", &format!("/Groups/{ACCESS_GROUP}")],
    );
    probe.macos_remote_login_ours = Path::new(REMOTE_LOGIN_MARK).exists();
    probe.macos_access_group_ours = Path::new(ACCESS_GROUP_MARK).exists();
    if let Ok(config) = fs::read_to_string(SSHD_CONFIG) {
        let parsed = parse_sshd_effective(&config);
        probe.sshd_config_read = true;
        probe.max_sessions = parsed.max_sessions;
        probe.sftp_enabled = parsed.sftp_enabled;
    }
    probe.sshd_active = probe.remote_login_on;
    probe
}

fn succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
