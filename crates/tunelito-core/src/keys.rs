use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey, PublicKey};

use crate::error::Error;

pub const DEDICATED_USER: &str = "tunelito";

/// Groups the dedicated user may join so USB and adb devices are reachable.
/// Never extended with sudo, admin, wheel, docker, disk, or input.
pub const DEVICE_GROUPS: &[&str] = &["plugdev", "adbusers"];

pub const FORBIDDEN_GROUPS: &[&str] = &[
    "sudo", "admin", "wheel", "docker", "disk", "input", "lpadmin", "root",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedKey {
    pub public_key: String,
    pub fingerprint: String,
    pub comment: String,
    pub key_type: String,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedKey {
    pub public_key: String,
    pub fingerprint: String,
    pub comment: String,
    pub key_type: String,
}

pub fn parse_public_keys(text: &str) -> Result<Vec<ParsedKey>, Error> {
    if text.contains("PRIVATE KEY") {
        return Err(Error::private_key_rejected());
    }
    let mut keys = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if looks_like_option_prefix(line) {
            return Err(Error::invalid_public_key());
        }
        keys.push(parse_public_key_line(line)?);
    }
    let mut seen = Vec::new();
    for key in &keys {
        if seen.contains(&key.fingerprint) {
            return Err(Error::duplicate_key());
        }
        seen.push(key.fingerprint.clone());
    }
    Ok(keys)
}

fn looks_like_option_prefix(line: &str) -> bool {
    let Some(first) = line.split_whitespace().next() else {
        return false;
    };
    first.contains('=') || first == "restrict" || first == "command"
}

pub fn parse_public_key_line(line: &str) -> Result<ParsedKey, Error> {
    let key = PublicKey::from_openssh(line).map_err(Error::ssh_key)?;
    parsed_from_public(&key)
}

fn parsed_from_public(key: &PublicKey) -> Result<ParsedKey, Error> {
    let public_key = key.to_openssh().map_err(Error::ssh_key)?;
    Ok(ParsedKey {
        public_key: public_key.trim().to_string(),
        fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
        comment: key.comment().to_string(),
        key_type: key.algorithm().to_string(),
    })
}

pub fn fingerprint_of(public_key_line: &str) -> Result<String, Error> {
    Ok(parse_public_key_line(public_key_line)?.fingerprint)
}

/// Create an ed25519 keypair if `private_path` is missing. Never overwrites an existing key.
pub fn ensure_keypair(private_path: &Path, comment: &str) -> Result<(ParsedKey, bool), Error> {
    if private_path.exists() {
        let data = fs::read_to_string(private_path)?;
        let key = PrivateKey::from_openssh(&data).map_err(Error::ssh_key)?;
        return Ok((parsed_from_public(key.public_key())?, false));
    }
    if let Some(parent) = private_path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }
    let mut key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).map_err(Error::ssh_key)?;
    key.set_comment(comment);
    let encoded = key.to_openssh(LineEnding::LF).map_err(Error::ssh_key)?;
    fs::write(private_path, encoded.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(private_path, fs::Permissions::from_mode(0o600))?;
    }
    let public_line = parsed_from_public(key.public_key())?;
    let pub_path = public_path_for(private_path);
    fs::write(&pub_path, format!("{}\n", public_line.public_key))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&pub_path, fs::Permissions::from_mode(0o644));
    }
    Ok((public_line, true))
}

pub fn public_path_for(private_path: &Path) -> std::path::PathBuf {
    let mut name = private_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tunelito_ed25519".into());
    name.push_str(".pub");
    private_path.with_file_name(name)
}

pub fn validate_user(user: &str) -> Result<(), Error> {
    if user == "root" || user.eq_ignore_ascii_case("administrator") || user == "admin" {
        return Err(Error::refused_user());
    }
    let mut chars = user.chars();
    let Some(first) = chars.next() else {
        return Err(Error::refused_user());
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(Error::refused_user());
    }
    if user.len() > 32
        || !user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(Error::refused_user());
    }
    Ok(())
}

pub fn filter_device_groups<'a>(requested: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    requested
        .into_iter()
        .filter(|group| DEVICE_GROUPS.contains(group) && !FORBIDDEN_GROUPS.contains(group))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_generated_key_and_file_mode() {
        let dir = std::env::temp_dir().join(format!("tunelito-key-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("id");
        let (parsed, created) = ensure_keypair(&path, "helper").unwrap();
        assert!(created);
        assert!(parsed.public_key.starts_with("ssh-ed25519 "));
        assert!(parsed.public_key.contains("helper"));
        assert!(parsed.fingerprint.starts_with("SHA256:"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let (again, created_again) = ensure_keypair(&path, "other").unwrap();
        assert!(!created_again);
        assert_eq!(again.fingerprint, parsed.fingerprint);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fingerprint_matches_openssh_when_available() {
        let dir = std::env::temp_dir().join(format!("tunelito-fp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("id");
        let (parsed, _) = ensure_keypair(&path, "cmp").unwrap();
        let pub_path = public_path_for(&path);
        let output = std::process::Command::new("ssh-keygen")
            .args(["-lf", pub_path.to_str().unwrap()])
            .output();
        let Ok(output) = output else {
            let _ = fs::remove_dir_all(&dir);
            return;
        };
        if !output.status.success() {
            let _ = fs::remove_dir_all(&dir);
            return;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(&parsed.fingerprint),
            "ssh-keygen said {stdout}, we said {}",
            parsed.fingerprint
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_private_key_and_options() {
        let pem = "-----BEGIN OPENSSH PRIVATE KEY-----\nAAA\n-----END OPENSSH PRIVATE KEY-----";
        assert_eq!(
            parse_public_keys(pem).unwrap_err().code,
            "private_key_rejected"
        );
        let opted =
            "restrict ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFakeKeyForTestOnlyNotReal comment";
        assert_eq!(
            parse_public_keys(opted).unwrap_err().code,
            "invalid_public_key"
        );
    }

    #[test]
    fn forbidden_groups_never_pass_the_filter() {
        let groups =
            filter_device_groups(["sudo", "plugdev", "input", "wheel", "adbusers", "docker"]);
        assert_eq!(groups, vec!["plugdev".to_string(), "adbusers".to_string()]);
    }

    #[test]
    fn rejects_root_user() {
        assert!(validate_user("root").is_err());
        assert!(validate_user("Administrator").is_err());
        assert!(validate_user("tunelito").is_ok());
    }
}
