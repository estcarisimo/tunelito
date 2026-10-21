use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::keys::parse_public_keys;
use crate::sshd::{upsert_authorized_keys, validate_dropin};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessRequest {
    pub authorized_key_lines: Vec<String>,
    #[serde(default)]
    pub dropin: Option<String>,
    /// When true, write the managed key block even if the line list is empty (switch off).
    pub write_keys: bool,
    /// Install OpenSSH and the dedicated user. Honored only when the helper is root on `/`.
    #[serde(default)]
    pub prepare: bool,
    #[serde(default)]
    pub start_sshd: bool,
    #[serde(default)]
    pub stop_sshd: bool,
    #[serde(default)]
    pub reload_sshd: bool,
    /// Used when writing the Windows layout without a live probe (tests, fake root).
    #[serde(default)]
    pub windows_account_is_admin: bool,
    #[serde(default)]
    pub windows_set_global_max_sessions: bool,
    #[serde(default)]
    pub windows_add_sftp: bool,
    /// SID of the Windows account that ran setup. It may drop requests for the
    /// SYSTEM task and start that task. Checked by `windows::validate_owner_sid`.
    #[serde(default)]
    pub windows_owner_sid: Option<String>,
}

/// Filesystem half of turning access on. System commands stay in the planner.
/// `root` is `/` in production and a temp directory in tests.
pub fn write_linux_access_files(
    root: &Path,
    request: &AccessRequest,
) -> Result<Vec<PathBuf>, Error> {
    write_unix_access_files(
        root,
        request,
        "home/tunelito/.ssh/authorized_keys",
        "var/lib/tunelito/hellos",
    )
}

/// The same files at the macOS paths in `crate::macos`.
/// UNTESTED ON CI on a Mac. The layout is checked here under a fake root.
pub fn write_macos_access_files(
    root: &Path,
    request: &AccessRequest,
) -> Result<Vec<PathBuf>, Error> {
    write_unix_access_files(
        root,
        request,
        crate::macos::AUTHORIZED_KEYS.trim_start_matches('/'),
        crate::macos::HELLO_DIR.trim_start_matches('/'),
    )
}

fn write_unix_access_files(
    root: &Path,
    request: &AccessRequest,
    authorized_keys: &str,
    hello_dir: &str,
) -> Result<Vec<PathBuf>, Error> {
    for line in &request.authorized_key_lines {
        if line.trim().is_empty() {
            continue;
        }
        let parsed = parse_public_keys(line)?;
        if parsed.len() != 1 {
            return Err(Error::invalid_public_key());
        }
    }
    if let Some(dropin) = &request.dropin {
        validate_dropin(dropin)?;
    }

    let mut written = Vec::new();
    if let Some(dropin) = &request.dropin {
        let path = root.join("etc/ssh/sshd_config.d/00-tunelito.conf");
        write_file(&path, dropin, 0o644)?;
        written.push(path);
    }
    if request.write_keys {
        let path = root.join(authorized_keys);
        // `tunelito` owns its home. Refuse a link it planted there, or this root
        // write would land wherever the link points.
        for candidate in [path.parent(), Some(path.as_path())].into_iter().flatten() {
            refuse_link(candidate)?;
        }
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let body = upsert_authorized_keys(&existing, &request.authorized_key_lines);
        write_file(&path, &body, 0o600)?;
        if let Some(dir) = path.parent() {
            set_mode(dir, 0o700)?;
        }
        written.push(path);
    }
    let hellos = root.join(hello_dir);
    refuse_link(&hellos)?;
    fs::create_dir_all(&hellos)?;
    set_mode(&hellos, 0o1777)?;
    written.push(hellos);
    Ok(written)
}

fn refuse_link(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(Error::prepare_failed(format!(
            "refusing to write through a link at {}",
            path.display()
        ))),
        _ => Ok(()),
    }
}

fn write_file(path: &Path, contents: &str, mode: u32) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    set_mode(path, mode)?;
    Ok(())
}

fn set_mode(path: &Path, mode: u32) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

pub fn read_limited(path: &Path, max_bytes: usize) -> Result<String, Error> {
    let bytes = fs::read(path)?;
    let start = bytes.len().saturating_sub(max_bytes);
    let slice = &bytes[start..];
    Ok(String::from_utf8_lossy(slice).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_dropin_keys_and_hello_dir_under_a_fake_root() {
        let root = std::env::temp_dir().join(format!("tunelito-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let key = generate_line();
        let request = AccessRequest {
            authorized_key_lines: vec![key.clone()],
            dropin: Some(crate::sshd::render_dropin(true, false)),
            write_keys: true,
            prepare: false,
            start_sshd: false,
            stop_sshd: false,
            reload_sshd: false,
            windows_account_is_admin: false,
            windows_set_global_max_sessions: false,
            windows_add_sftp: false,
            windows_owner_sid: None,
        };
        write_linux_access_files(&root, &request).unwrap();
        let dropin =
            fs::read_to_string(root.join("etc/ssh/sshd_config.d/00-tunelito.conf")).unwrap();
        assert!(dropin.contains("Match User tunelito"));
        let keys = fs::read_to_string(root.join("home/tunelito/.ssh/authorized_keys")).unwrap();
        assert!(keys.contains(&key));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(root.join("home/tunelito/.ssh/authorized_keys"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
            let hello_mode = fs::metadata(root.join("var/lib/tunelito/hellos"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777;
            assert_eq!(hello_mode, 0o1777);
        }

        let cleared = AccessRequest {
            authorized_key_lines: vec![],
            dropin: None,
            write_keys: true,
            prepare: false,
            start_sshd: false,
            stop_sshd: false,
            reload_sshd: false,
            windows_account_is_admin: false,
            windows_set_global_max_sessions: false,
            windows_add_sftp: false,
            windows_owner_sid: None,
        };
        write_linux_access_files(&root, &cleared).unwrap();
        let keys = fs::read_to_string(root.join("home/tunelito/.ssh/authorized_keys")).unwrap();
        assert!(!keys.contains(&key));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn macos_layout_uses_users_and_the_dropin_directory() {
        let root = std::env::temp_dir().join(format!("tunelito-root-mac-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let key = generate_line();
        let request = AccessRequest {
            authorized_key_lines: vec![key.clone()],
            dropin: Some(crate::sshd::render_dropin(false, false)),
            write_keys: true,
            prepare: false,
            start_sshd: false,
            stop_sshd: false,
            reload_sshd: false,
            windows_account_is_admin: false,
            windows_set_global_max_sessions: false,
            windows_add_sftp: false,
            windows_owner_sid: None,
        };
        write_macos_access_files(&root, &request).unwrap();
        let keys = fs::read_to_string(root.join("Users/tunelito/.ssh/authorized_keys")).unwrap();
        assert!(keys.contains(&key));
        assert!(root
            .join("etc/ssh/sshd_config.d/00-tunelito.conf")
            .is_file());
        assert!(!root.join("etc/ssh/sshd_config").exists());
        assert!(root
            .join("Library/Application Support/tunelito/hellos")
            .is_dir());
        assert!(!root.join("home").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_linked_ssh_directory() {
        let root = std::env::temp_dir().join(format!("tunelito-root-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let target = root.join("elsewhere");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(root.join("home/tunelito")).unwrap();
        std::os::unix::fs::symlink(&target, root.join("home/tunelito/.ssh")).unwrap();
        let request = AccessRequest {
            authorized_key_lines: vec![generate_line()],
            dropin: None,
            write_keys: true,
            prepare: false,
            start_sshd: false,
            stop_sshd: false,
            reload_sshd: false,
            windows_account_is_admin: false,
            windows_set_global_max_sessions: false,
            windows_add_sftp: false,
            windows_owner_sid: None,
        };
        let err = write_linux_access_files(&root, &request).unwrap_err();
        assert_eq!(err.code, "prepare_failed");
        assert!(!target.join("authorized_keys").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_a_dropin_the_planner_did_not_sign() {
        let root = std::env::temp_dir().join(format!("tunelito-root-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let request = AccessRequest {
            authorized_key_lines: vec![],
            dropin: Some("Match User *\n    PermitRootLogin yes\n".into()),
            write_keys: false,
            prepare: false,
            start_sshd: false,
            stop_sshd: false,
            reload_sshd: false,
            windows_account_is_admin: false,
            windows_set_global_max_sessions: false,
            windows_add_sftp: false,
            windows_owner_sid: None,
        };
        assert_eq!(
            write_linux_access_files(&root, &request).unwrap_err().code,
            "unsafe_sshd_dropin"
        );
        let _ = fs::remove_dir_all(&root);
    }

    fn generate_line() -> String {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("tunelito-access-key-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (parsed, _) = crate::keys::ensure_keypair(&dir.join("k"), "helper").unwrap();
        let _ = fs::remove_dir_all(&dir);
        parsed.public_key
    }
}
