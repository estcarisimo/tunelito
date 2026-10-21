use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::ssh_config::null_ssh_config_arg;

pub const HELLO_VERSION: u32 = 1;
pub const VERIFICATION: &str = "unverified";
pub const HELLO_LABEL: &str = "reported by client (unverified)";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub tunelito_hello: u32,
    pub public_ip: Option<String>,
    pub asn: Option<String>,
    pub rdns: Option<String>,
    pub os: String,
    pub os_detail: Option<String>,
    pub key_fingerprint: String,
    pub reported_at: DateTime<Utc>,
    /// Always rewritten to [`VERIFICATION`] on parse. A client cannot claim otherwise.
    pub verification: String,
}

impl Hello {
    pub fn new(
        public_ip: Option<String>,
        asn: Option<String>,
        rdns: Option<String>,
        os: impl Into<String>,
        os_detail: Option<String>,
        key_fingerprint: impl Into<String>,
        reported_at: DateTime<Utc>,
    ) -> Self {
        Self {
            tunelito_hello: HELLO_VERSION,
            public_ip: public_ip.filter(|ip| !ip.trim().is_empty()),
            asn: asn.filter(|asn| !asn.trim().is_empty()),
            rdns: rdns.filter(|name| !name.trim().is_empty()),
            os: os.into(),
            os_detail,
            key_fingerprint: key_fingerprint.into(),
            reported_at,
            verification: VERIFICATION.to_string(),
        }
    }

    pub fn to_json(&self) -> Result<String, Error> {
        serde_json::to_string_pretty(self)
            .map_err(|err| Error::with_detail("invalid_hello", err.to_string()))
    }
}

pub fn parse_hello(bytes: &str) -> Result<Hello, Error> {
    let mut hello: Hello = serde_json::from_str(bytes)
        .map_err(|err| Error::with_detail("invalid_hello", err.to_string()))?;
    if hello.tunelito_hello != HELLO_VERSION || hello.key_fingerprint.is_empty() {
        return Err(Error::invalid_hello());
    }
    hello.verification = VERIFICATION.to_string();
    Ok(hello)
}

pub fn hello_filename(fingerprint: &str, at: DateTime<Utc>) -> String {
    let slug: String = fingerprint
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(80)
        .collect();
    format!("{}-{slug}.json", at.timestamp_millis())
}

pub fn hello_remote_command(filename: &str) -> Result<String, Error> {
    if filename.is_empty()
        || filename.len() > 120
        || !filename
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(Error::invalid_hello());
    }
    Ok(format!(
        "umask 022; if [ -d /var/lib/tunelito/hellos ] && [ -w /var/lib/tunelito/hellos ]; then cat > /var/lib/tunelito/hellos/{filename}; else mkdir -p \"$HOME/.tunelito/hellos\" && cat > \"$HOME/.tunelito/hellos/{filename}\"; fi"
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelloSsh {
    pub hostname: String,
    pub user: String,
    pub identity_file: String,
    pub remote_command: String,
}

pub fn hello_ssh_args(hello: &HelloSsh) -> Result<Vec<String>, Error> {
    crate::cloudflared::validate_hostname(&hello.hostname)?;
    crate::keys::validate_user(&hello.user)?;
    let proxy = format!("cloudflared access ssh --hostname {}", hello.hostname);
    Ok(vec![
        "-F".into(),
        null_ssh_config_arg().into(),
        "-o".into(),
        "IdentitiesOnly=yes".into(),
        "-o".into(),
        format!("IdentityFile={}", hello.identity_file),
        "-o".into(),
        format!("ProxyCommand={proxy}"),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "ConnectTimeout=20".into(),
        format!("{}@{}", hello.user, hello.hostname),
        hello.remote_command.clone(),
    ])
}

pub fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./:@%".contains(c))
            {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Skip a repeat hello when a stamp file is younger than `min_interval`.
pub fn should_send_once(stamp: &Path, now: SystemTime, min_interval: Duration) -> bool {
    let Ok(modified) = fs::metadata(stamp).and_then(|meta| meta.modified()) else {
        return true;
    };
    match now.duration_since(modified) {
        Ok(age) => age >= min_interval,
        Err(_) => true,
    }
}

pub fn read_hellos(dirs: &[impl AsRef<Path>]) -> Vec<Hello> {
    let mut found = Vec::new();
    for dir in dirs {
        let dir = dir.as_ref();
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if let Ok(hello) = parse_hello(&text) {
                found.push(hello);
            }
        }
    }
    found.sort_by_key(|hello| hello.reported_at);
    found.truncate(200);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claimed_verification_is_downgraded() {
        let raw = r#"{
            "tunelito_hello": 1,
            "public_ip": "203.0.113.9",
            "asn": "AS64500",
            "rdns": "helper.example",
            "os": "linux",
            "os_detail": "Linux 6.8",
            "key_fingerprint": "SHA256:abc",
            "reported_at": "2026-10-20T23:00:00Z",
            "verification": "verified"
        }"#;
        let hello = parse_hello(raw).unwrap();
        assert_eq!(hello.verification, "unverified");
        assert_eq!(hello.public_ip.as_deref(), Some("203.0.113.9"));
    }

    #[test]
    fn remote_command_rejects_path_traversal() {
        assert!(hello_remote_command("../evil").is_err());
        let name = hello_filename("SHA256:ab+c", Utc::now());
        let command = hello_remote_command(&name).unwrap();
        assert!(command.contains(&name));
        assert!(!command.contains(".."));
    }

    #[test]
    fn ssh_args_do_not_use_the_user_config() {
        let args = hello_ssh_args(&HelloSsh {
            hostname: "lime-eagle-4821.trycloudflare.com".into(),
            user: "tunelito".into(),
            identity_file: "~/.ssh/tunelito_ed25519".into(),
            remote_command: hello_remote_command("1-SHA256_abc.json").unwrap(),
        })
        .unwrap();
        assert_eq!(args[0], "-F");
        assert_eq!(args[1], null_ssh_config_arg());
        let joined = shell_join(&args);
        assert!(
            joined.contains("cloudflared access ssh --hostname lime-eagle-4821.trycloudflare.com")
        );
        assert!(!joined.contains("LocalCommand"));
    }

    #[test]
    fn once_stamp_suppresses_a_fresh_file() {
        let dir = std::env::temp_dir().join(format!("tunelito-stamp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let stamp = dir.join("host");
        assert!(should_send_once(
            &stamp,
            SystemTime::now(),
            Duration::from_secs(600)
        ));
        fs::write(&stamp, "sent").unwrap();
        assert!(!should_send_once(
            &stamp,
            SystemTime::now(),
            Duration::from_secs(600)
        ));
        let _ = fs::remove_dir_all(&dir);
    }
}
