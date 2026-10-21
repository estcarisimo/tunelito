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
