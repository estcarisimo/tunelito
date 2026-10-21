use crate::error::Error;
use crate::keys::DEDICATED_USER;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SshdEffective {
    pub max_sessions: Option<u32>,
    pub sftp_enabled: bool,
    pub config_read: bool,
}

pub fn parse_sshd_effective(text: &str) -> SshdEffective {
    let mut max_sessions = None;
    let mut sftp_enabled = false;
    for line in text.lines() {
        let line = line.trim().to_ascii_lowercase();
        if let Some(rest) = line.strip_prefix("maxsessions ") {
            max_sessions = rest.trim().parse().ok();
        }
        if line.starts_with("subsystem sftp") {
            sftp_enabled = true;
        }
    }
    SshdEffective {
        max_sessions,
        sftp_enabled,
        config_read: true,
    }
}

pub fn render_dropin(set_global_max_sessions: bool, add_sftp: bool) -> String {
    let mut out = String::from(
        "# Managed by tunelito. Safe to delete; the app recreates it.\n\
         # This file does not enable sshd at boot.\n",
    );
    if set_global_max_sessions {
        out.push_str("MaxSessions 10\n");
    }
    if add_sftp {
        out.push_str("Subsystem sftp internal-sftp\n");
    }
    out.push_str(&format!(
        "\nMatch User {DEDICATED_USER}\n    \
         PasswordAuthentication no\n    \
         KbdInteractiveAuthentication no\n    \
         PubkeyAuthentication yes\n    \
         AuthenticationMethods publickey\n    \
         MaxSessions 10\n    \
         X11Forwarding no\n    \
         AllowTcpForwarding no\n    \
         ClientAliveInterval 30\n    \
         ClientAliveCountMax 4\n"
    ));
    out
}

/// True when `sshd_config` includes `/etc/ssh/sshd_config.d/` before any
/// `Match`. Debian 11+, Fedora, current Arch, and macOS 13+ ship that line.
/// Without it the Tunelito drop-in is ignored, so the helper refuses.
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

pub fn dropin_decisions(effective: &SshdEffective, openssh_installed: bool) -> (bool, bool) {
    let set_global = match effective.max_sessions {
        Some(n) if n >= 10 => false,
        Some(_) => true,
        None if !openssh_installed => true,
        None => false,
    };
    let add_sftp = openssh_installed && effective.config_read && !effective.sftp_enabled;
    (set_global, add_sftp)
}

const ALLOWED_KEYS: &[&str] = &[
    "MaxSessions",
    "Subsystem",
    "PasswordAuthentication",
    "KbdInteractiveAuthentication",
    "PubkeyAuthentication",
    "AuthenticationMethods",
    "X11Forwarding",
    "AllowTcpForwarding",
    "ClientAliveInterval",
    "ClientAliveCountMax",
];

pub fn validate_dropin(text: &str) -> Result<(), Error> {
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.contains('|') || line.contains(';') || line.contains('`') || line.contains('$') {
            return Err(Error::unsafe_sshd_dropin());
        }
        if line.to_ascii_lowercase().starts_with("match ") {
            if line != format!("Match User {DEDICATED_USER}") {
                return Err(Error::unsafe_sshd_dropin());
            }
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("").trim();
        if !ALLOWED_KEYS.contains(&key) || value.is_empty() {
            return Err(Error::unsafe_sshd_dropin());
        }
        if key == "Subsystem" && value != "sftp internal-sftp" {
            return Err(Error::unsafe_sshd_dropin());
        }
        if key == "MaxSessions" && value != "10" {
            return Err(Error::unsafe_sshd_dropin());
        }
    }
    Ok(())
}

pub fn upsert_authorized_keys(existing: &str, key_lines: &[String]) -> String {
    let begin = "# BEGIN tunelito managed keys";
    let end = "# END tunelito managed keys";
    let mut body = String::from(begin);
    body.push('\n');
    for line in key_lines {
        body.push_str(line.trim());
        body.push('\n');
    }
    body.push_str(end);
    body.push('\n');
    crate::ssh_config::replace_marked_block(existing, begin, end, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sshd_t_and_only_raises_a_low_limit() {
        let text = "maxsessions 4\nsubsystem sftp internal-sftp\npubkeyauthentication yes\n";
        let parsed = parse_sshd_effective(text);
        assert_eq!(parsed.max_sessions, Some(4));
        assert!(parsed.sftp_enabled);
        let (global, sftp) = dropin_decisions(&parsed, true);
        assert!(global);
        assert!(!sftp);
        let dropin = render_dropin(global, sftp);
        validate_dropin(&dropin).unwrap();
        assert!(dropin.contains("MaxSessions 10\n"));
        assert!(dropin.contains("Match User tunelito\n"));
        assert!(!dropin.contains("systemctl"));
    }

    #[test]
    fn missing_sftp_on_an_installed_server_is_added() {
        let parsed = SshdEffective {
            max_sessions: Some(10),
            sftp_enabled: false,
            config_read: true,
        };
        let (global, sftp) = dropin_decisions(&parsed, true);
        assert!(!global);
        assert!(sftp);
        let dropin = render_dropin(false, true);
        assert!(dropin.contains("Subsystem sftp internal-sftp\n"));
        validate_dropin(&dropin).unwrap();
    }

    #[test]
    fn rejects_injected_directives() {
        let evil = "Match User tunelito\n    ForceCommand /bin/sh\n";
        assert_eq!(
            validate_dropin(evil).unwrap_err().code,
            "unsafe_sshd_dropin"
        );
        let star = "Match User *\n    PasswordAuthentication yes\n";
        assert!(validate_dropin(star).is_err());
    }

    #[test]
    fn authorized_keys_block_preserves_foreign_lines() {
        let existing = "ssh-ed25519 AAAAforeign me\n";
        let managed = vec!["ssh-ed25519 AAAAmanaged helper".to_string()];
        let once = upsert_authorized_keys(existing, &managed);
        let twice = upsert_authorized_keys(&once, &managed);
        assert_eq!(once, twice);
        assert!(twice.contains("AAAAforeign"));
        assert!(twice.contains("AAAAmanaged"));
        let cleared = upsert_authorized_keys(&twice, &[]);
        assert!(cleared.contains("AAAAforeign"));
        assert!(!cleared.contains("AAAAmanaged"));
    }
}
