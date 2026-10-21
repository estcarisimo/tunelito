use std::fs;
use std::process::Command;

#[test]
fn files_only_apply_writes_under_the_given_root() {
    let dir = std::env::temp_dir().join(format!("tunelito-priv-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let key_dir = dir.join("key");
    fs::create_dir_all(&key_dir).unwrap();
    let (parsed, _) = tunelito_core::keys::ensure_keypair(&key_dir.join("k"), "helper").unwrap();
    let request = serde_json::json!({
        "authorized_key_lines": [parsed.public_key],
        "dropin": tunelito_core::sshd::render_dropin(true, false),
        "write_keys": true
    });
    let request_path = dir.join("request.json");
    fs::write(&request_path, serde_json::to_string(&request).unwrap()).unwrap();
    let root = dir.join("root");
    let output = Command::new(env!("CARGO_BIN_EXE_tunelito-priv"))
        .args([
            "apply",
            "--request",
            request_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let keys = fs::read_to_string(root.join("home/tunelito/.ssh/authorized_keys")).unwrap();
    assert!(keys.contains("ssh-ed25519"));
    let dropin = fs::read_to_string(root.join("etc/ssh/sshd_config.d/00-tunelito.conf")).unwrap();
    assert!(dropin.contains("MaxSessions 10"));
    assert!(root.join("var/lib/tunelito/hellos").is_dir());

    let execute = Command::new(env!("CARGO_BIN_EXE_tunelito-priv"))
        .args([
            "apply",
            "--request",
            request_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
            "--execute",
        ])
        .output()
        .unwrap();
    assert!(!execute.status.success());
    let err = String::from_utf8_lossy(&execute.stderr);
    assert!(
        err.contains("fake root") || err.contains("needs_admin"),
        "{err}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn windows_layout_under_a_fake_root_refuses_execute() {
    let dir = std::env::temp_dir().join(format!("tunelito-priv-win-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let key_dir = dir.join("key");
    fs::create_dir_all(&key_dir).unwrap();
    let (parsed, _) = tunelito_core::keys::ensure_keypair(&key_dir.join("k"), "helper").unwrap();
    let request = serde_json::json!({
        "authorized_key_lines": [parsed.public_key],
        "write_keys": true,
        "windows_set_global_max_sessions": true,
        "windows_add_sftp": true
    });
    let request_path = dir.join("request.json");
    fs::write(&request_path, request.to_string()).unwrap();
    let root = dir.join("root");
    let result = dir.join("result.json");
    let output = Command::new(env!("CARGO_BIN_EXE_tunelito-priv"))
        .args([
            "apply",
            "--request",
            request_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
            "--windows",
            "--result",
            result.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = fs::read_to_string(root.join("ProgramData/ssh/sshd_config")).unwrap();
    assert!(config.contains("Match User tunelito"));
    assert!(config.contains("Subsystem sftp sftp-server.exe"));
    assert!(config.contains("MaxSessions 10"));
    let keys = fs::read_to_string(root.join("ProgramData/ssh/tunelito_authorized_keys")).unwrap();
    assert!(keys.contains("ssh-ed25519"));
    let profile = fs::read_to_string(root.join("Users/tunelito/.ssh/authorized_keys")).unwrap();
    assert!(profile.contains("ssh-ed25519"));
    let body: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&result).unwrap()).unwrap();
    assert_eq!(body["ok"], true);

    let execute = Command::new(env!("CARGO_BIN_EXE_tunelito-priv"))
        .args([
            "apply",
            "--request",
            request_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
            "--windows",
            "--execute",
        ])
        .output()
        .unwrap();
    assert!(!execute.status.success());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn macos_flag_writes_the_macos_layout_and_refuses_execute_off_a_mac() {
    let dir = std::env::temp_dir().join(format!("tunelito-priv-mac-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let key_dir = dir.join("key");
    fs::create_dir_all(&key_dir).unwrap();
    let (parsed, _) = tunelito_core::keys::ensure_keypair(&key_dir.join("k"), "helper").unwrap();
    let request = serde_json::json!({
        "authorized_key_lines": [parsed.public_key],
        "dropin": tunelito_core::sshd::render_dropin(false, false),
        "write_keys": true
    });
    let request_path = dir.join("request.json");
    fs::write(&request_path, serde_json::to_string(&request).unwrap()).unwrap();
    let root = dir.join("root");
    let output = Command::new(env!("CARGO_BIN_EXE_tunelito-priv"))
        .args([
            "apply",
            "--macos",
            "--request",
            request_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let keys = fs::read_to_string(root.join("Users/tunelito/.ssh/authorized_keys")).unwrap();
    assert!(keys.contains("ssh-ed25519"));
    assert!(root
        .join("etc/ssh/sshd_config.d/00-tunelito.conf")
        .is_file());
    assert!(root
        .join("Library/Application Support/tunelito/hellos")
        .is_dir());
    assert!(!root.join("home").exists());

    let execute = Command::new(env!("CARGO_BIN_EXE_tunelito-priv"))
        .args([
            "apply",
            "--macos",
            "--request",
            request_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
            "--execute",
        ])
        .output()
        .unwrap();
    assert!(!execute.status.success());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn through_pkexec_files_only_mode_is_refused() {
    let dir = std::env::temp_dir().join(format!("tunelito-priv-pk-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let request_path = dir.join("request.json");
    fs::write(
        &request_path,
        r#"{"authorized_key_lines":[],"write_keys":false}"#,
    )
    .unwrap();
    let root = dir.join("root");
    let output = Command::new(env!("CARGO_BIN_EXE_tunelito-priv"))
        .env("PKEXEC_UID", "1000")
        .args([
            "apply",
            "--request",
            request_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("pkexec"));
    assert!(!root.exists());
    let _ = fs::remove_dir_all(&dir);
}
