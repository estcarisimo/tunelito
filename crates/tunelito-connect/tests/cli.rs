use std::fs;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tunelito-connect"))
}

fn scratch() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tunelito-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn with_dirs(cmd: &mut Command, dir: &std::path::Path) {
    cmd.env("HOME", dir)
        .env("USERPROFILE", dir)
        .env("TUNELITO_SSH_DIR", dir.join("ssh"))
        .env("TUNELITO_SSH_CONFIG", dir.join("ssh/config"))
        .env("TUNELITO_STATE_DIR", dir.join("state"));
}

#[test]
fn key_install_and_hello_dry_run() {
    let dir = scratch();
    let mut key_cmd = bin();
    with_dirs(&mut key_cmd, &dir);
    let key = key_cmd.arg("key").output().unwrap();
    assert!(
        key.status.success(),
        "{}",
        String::from_utf8_lossy(&key.stderr)
    );
    let public = String::from_utf8_lossy(&key.stdout);
    assert!(public.contains("ssh-ed25519 "));
    assert!(!public.contains("PRIVATE"));
    let private = fs::read_to_string(dir.join("ssh/tunelito_ed25519")).unwrap();
    assert!(private.contains("PRIVATE KEY"));

    let host = "lime-eagle-4821.trycloudflare.com";
    let mut install_cmd = bin();
    with_dirs(&mut install_cmd, &dir);
    let install = install_cmd.args(["install", host]).output().unwrap();
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );
    let config = fs::read_to_string(dir.join("ssh/config")).unwrap();
    assert!(config.contains("ProxyCommand cloudflared access ssh --hostname %h"));
    assert_eq!(config.matches("# BEGIN tunelito").count(), 1);
    let mut again_cmd = bin();
    with_dirs(&mut again_cmd, &dir);
    let again = again_cmd.args(["install", host]).output().unwrap();
    assert!(again.status.success());
    let config_again = fs::read_to_string(dir.join("ssh/config")).unwrap();
    assert_eq!(config, config_again);

    let foreign = "Host example\n    User me\n";
    fs::write(dir.join("ssh/config"), foreign).unwrap();
    let mut merged_cmd = bin();
    with_dirs(&mut merged_cmd, &dir);
    let merged = merged_cmd.args(["install", host]).output().unwrap();
    assert!(merged.status.success());
    let config = fs::read_to_string(dir.join("ssh/config")).unwrap();
    assert!(config.contains("Host example"));
    assert!(config.contains("ProxyCommand cloudflared access ssh --hostname %h"));

    let mut hello_cmd = bin();
    with_dirs(&mut hello_cmd, &dir);
    let hello = hello_cmd
        .args([
            "hello",
            "--hostname",
            host,
            "--dry-run",
            "--ip",
            "203.0.113.9",
            "--asn",
            "AS64500",
            "--rdns",
            "helper.example",
            "--os",
            "linux",
            "--os-detail",
            "Linux test",
        ])
        .output()
        .unwrap();
    assert!(
        hello.status.success(),
        "{}",
        String::from_utf8_lossy(&hello.stderr)
    );
    let stdout = String::from_utf8_lossy(&hello.stdout);
    assert!(stdout.contains("\"verification\": \"unverified\""));
    assert!(stdout.contains("203.0.113.9"));
    assert!(stdout.contains("AS64500"));
    let stderr = String::from_utf8_lossy(&hello.stderr);
    assert!(stderr.contains("cloudflared access ssh --hostname lime-eagle-4821.trycloudflare.com"));
    assert!(stderr.contains("-F /dev/null") || stderr.contains("-F NUL"));
    assert!(!dir.join("state/hello-stamps").join(host).exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_root_as_remote_user() {
    let dir = scratch();
    let mut cmd = bin();
    with_dirs(&mut cmd, &dir);
    let output = cmd
        .args([
            "install",
            "lime-eagle-4821.trycloudflare.com",
            "--user",
            "root",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let _ = fs::remove_dir_all(&dir);
}
