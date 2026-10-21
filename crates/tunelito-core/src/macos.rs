//! macOS Remote Login, the `tunelito` account, and the paths the helper writes.
//!
//! UNTESTED ON CI: the Linux job checks this command text. Nothing here runs on a Mac.
//!
//! Remote Login is a launchd job that survives reboot once enabled. Tunelito turns it
//! on only when it was off, records that in a root-owned marker, and turns it back
//! off when access is switched off. While Tunelito owns Remote Login, only `tunelito`
//! is in `com.apple.access_ssh` ("Only these users"), so other accounts on the Mac
//! do not become reachable over SSH.

use crate::cloudflared::OsKind;
use crate::keys::DEDICATED_USER;
use crate::plan::{CommandStep, Probe};

pub const SSHD_CONFIG: &str = "/etc/ssh/sshd_config";
pub const DROPIN: &str = "/etc/ssh/sshd_config.d/00-tunelito.conf";
pub const USER_HOME: &str = "/Users/tunelito";
pub const AUTHORIZED_KEYS: &str = "/Users/tunelito/.ssh/authorized_keys";
pub const STATE_DIR: &str = "/Library/Application Support/tunelito";
pub const HELLO_DIR: &str = "/Library/Application Support/tunelito/hellos";
/// Present when Tunelito turned Remote Login on, so turning off undoes only that.
pub const REMOTE_LOGIN_MARK: &str = "/Library/Application Support/tunelito/remote-login-ours";
/// Present when Tunelito created the SSH access group, so turning off deletes it.
pub const ACCESS_GROUP_MARK: &str = "/Library/Application Support/tunelito/access-group-ours";
pub const ACCESS_GROUP: &str = "com.apple.access_ssh";
pub const SSHD_LABEL: &str = "system/com.openssh.sshd";
pub const SSHD_PLIST: &str = "/System/Library/LaunchDaemons/ssh.plist";

/// macOS 13 and later ship `Include /etc/ssh/sshd_config.d/*` at the top of
/// `sshd_config`. Without it the drop-in would be ignored, so setup refuses.
pub fn includes_dropins(config: &str) -> bool {
    for raw in config.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("match ") {
            return false;
        }
        if lower.starts_with("include ") && line.contains("/etc/ssh/sshd_config.d/") {
            return true;
        }
    }
    false
}

/// Create the hidden, non-admin `tunelito` account. The password is random and
/// never stored: the account logs in with a key only.
pub fn install_commands(probe: &Probe) -> Vec<CommandStep> {
    let mut steps = Vec::new();
    if !probe.user_exists {
        steps.push(sh(
            "create_dedicated_user",
            &format!(
                "set -e; pw=$(/usr/bin/openssl rand -base64 30); \
                 /usr/sbin/sysadminctl -addUser {DEDICATED_USER} -fullName Tunelito \
                 -shell /bin/zsh -home {USER_HOME} -password \"$pw\"; unset pw; \
                 /usr/bin/id -u {DEDICATED_USER} >/dev/null; \
                 /usr/sbin/createhomedir -c -u {DEDICATED_USER} >/dev/null; \
                 /usr/bin/dscl . -create /Users/{DEDICATED_USER} IsHidden 1"
            ),
        ));
    }
    steps
}

/// Turn access on. The access group is set before Remote Login starts, so no
/// other account is reachable even for a moment.
pub fn enable_commands(probe: &Probe) -> Vec<CommandStep> {
    let mut steps = Vec::new();
    if probe.macos_access_group {
        steps.push(sh(
            "allow_tunelito_ssh",
            &format!("/usr/sbin/dseditgroup -o edit -a {DEDICATED_USER} -t user {ACCESS_GROUP}"),
        ));
    } else if !probe.remote_login_on {
        // Remote Login was off and allowed everyone. Limit it to tunelito.
        steps.push(sh(
            "limit_ssh_to_tunelito",
            &format!(
                "set -e; /bin/mkdir -p '{STATE_DIR}'; \
                 /usr/sbin/dseditgroup -o create -q {ACCESS_GROUP}; \
                 /usr/bin/touch '{ACCESS_GROUP_MARK}'; \
                 /usr/sbin/dseditgroup -o edit -a {DEDICATED_USER} -t user {ACCESS_GROUP}"
            ),
        ));
    }
    if !probe.remote_login_on {
        let mut step = sh(
            "enable_remote_login",
            &format!(
                "set -e; /bin/mkdir -p '{STATE_DIR}'; \
                 /usr/bin/touch '{REMOTE_LOGIN_MARK}'; \
                 /bin/launchctl enable {SSHD_LABEL}; \
                 /bin/launchctl print {SSHD_LABEL} >/dev/null 2>&1 || \
                 /bin/launchctl bootstrap system {SSHD_PLIST}"
            ),
        );
        step.boot_persistent = true;
        steps.push(step);
    }
    steps
}

/// Turn access off. Remote Login goes off first, then the access group is
/// restored, so the group never widens while sshd is listening.
pub fn disable_commands(probe: &Probe) -> Vec<CommandStep> {
    let mut steps = Vec::new();
    if probe.macos_remote_login_ours {
        let mut step = sh(
            "disable_remote_login",
            &format!(
                "/bin/launchctl bootout {SSHD_LABEL} 2>/dev/null || true; \
                 /bin/launchctl disable {SSHD_LABEL}; \
                 /bin/rm -f '{REMOTE_LOGIN_MARK}'"
            ),
        );
        step.boot_persistent = true;
        steps.push(step);
    }
    if probe.macos_access_group_ours {
        steps.push(sh(
            "restore_ssh_access_group",
            &format!(
                "/usr/sbin/dseditgroup -o delete {ACCESS_GROUP} 2>/dev/null || true; \
                 /bin/rm -f '{ACCESS_GROUP_MARK}'"
            ),
        ));
    } else if probe.macos_access_group {
        steps.push(sh(
            "remove_tunelito_ssh",
            &format!(
                "/usr/sbin/dseditgroup -o edit -d {DEDICATED_USER} -t user {ACCESS_GROUP} 2>/dev/null || true"
            ),
        ));
    }
    steps
}

/// sshd is started by launchd for each connection and reads its config then,
/// so a changed drop-in needs no restart.
pub fn reload_command() -> CommandStep {
    CommandStep::new(OsKind::Macos, "/usr/bin/true", Vec::new(), "reload_sshd")
}

fn sh(purpose: &'static str, script: &str) -> CommandStep {
    CommandStep::new(
        OsKind::Macos,
        "/bin/sh",
        vec!["-c".into(), script.to_string()],
        purpose,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac() -> Probe {
        let mut probe = Probe::linux_fresh();
        probe.os = OsKind::Macos;
        probe.ssh_unit = "sshd".into();
        probe
    }

    fn blob(steps: &[CommandStep]) -> String {
        steps
            .iter()
            .map(|s| s.line())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn needs_the_stock_include_line_before_any_match() {
        let stock = "# comment\nInclude /etc/ssh/sshd_config.d/*\n\nUsePAM yes\n";
        assert!(includes_dropins(stock));
        assert!(!includes_dropins("UsePAM yes\n"));
        assert!(!includes_dropins(
            "Match User x\nInclude /etc/ssh/sshd_config.d/*\n"
        ));
        assert!(!includes_dropins("#Include /etc/ssh/sshd_config.d/*\n"));
        assert!(DROPIN.starts_with("/etc/ssh/sshd_config.d/"));
    }

    #[test]
    fn new_account_is_hidden_and_not_an_admin() {
        let steps = install_commands(&mac());
        let text = blob(&steps).to_ascii_lowercase();
        assert!(text.contains("sysadminctl -adduser tunelito"));
        assert!(text.contains("ishidden 1"));
        assert!(text.contains("openssl rand"));
        assert!(!text.contains("-admin"));
        assert!(!text.contains("admin tunelito"));
        assert!(!text.contains("sudo"));
        let mut existing = mac();
        existing.user_exists = true;
        assert!(install_commands(&existing).is_empty());
    }

    #[test]
    fn remote_login_off_before_means_only_tunelito_and_undo_on_off() {
        let probe = mac();
        let on = enable_commands(&probe);
        let purposes: Vec<_> = on.iter().map(|s| s.purpose).collect();
        assert_eq!(purposes, ["limit_ssh_to_tunelito", "enable_remote_login"]);
        let text = blob(&on);
        assert!(text.contains(REMOTE_LOGIN_MARK));
        assert!(text.contains(ACCESS_GROUP_MARK));
        assert!(text.contains("launchctl enable system/com.openssh.sshd"));
        assert!(!text.contains("systemsetup"));
        assert!(on.iter().all(|s| !s.is_forbidden_autostart()));
        assert!(on.iter().any(|s| s.boot_persistent));

        let mut after = mac();
        after.remote_login_on = true;
        after.macos_access_group = true;
        after.macos_remote_login_ours = true;
        after.macos_access_group_ours = true;
        let off = disable_commands(&after);
        let purposes: Vec<_> = off.iter().map(|s| s.purpose).collect();
        assert_eq!(
            purposes,
            ["disable_remote_login", "restore_ssh_access_group"]
        );
        let text = blob(&off);
        assert!(text.contains("launchctl disable system/com.openssh.sshd"));
        assert!(text.contains("dseditgroup -o delete com.apple.access_ssh"));
    }

    #[test]
    fn remote_login_already_on_is_left_alone() {
        let mut probe = mac();
        probe.remote_login_on = true;
        assert!(enable_commands(&probe).is_empty());
        assert!(disable_commands(&probe).is_empty());

        probe.macos_access_group = true;
        let on = enable_commands(&probe);
        assert_eq!(on.len(), 1);
        assert_eq!(on[0].purpose, "allow_tunelito_ssh");
        let off = disable_commands(&probe);
        assert_eq!(off.len(), 1);
        assert_eq!(off[0].purpose, "remove_tunelito_ssh");
        assert!(!blob(&off).contains("launchctl"));
        assert!(!blob(&off).contains("-o delete"));
    }

    #[test]
    fn reload_does_not_restart_sshd() {
        let step = reload_command();
        assert!(!step.line().contains("kickstart"));
        assert!(!step.line().contains("kill"));
    }
}
