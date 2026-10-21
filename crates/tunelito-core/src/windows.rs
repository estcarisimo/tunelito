//! Windows OpenSSH file layout and the command text the privileged helper runs.
//!
//! The Linux job unit-tests this module. It does not install a Windows capability.
//! `tunelito-priv --windows` writes the same layout under a fake root.

use std::fs;
use std::path::{Path, PathBuf};

use crate::cloudflared::OsKind;
use crate::error::Error;
use crate::keys::{parse_public_keys, DEDICATED_USER};
use crate::plan::{CommandStep, Probe};
use crate::sshd::{dropin_decisions, upsert_authorized_keys};

pub const MARKER_BEGIN: &str = "# BEGIN tunelito";
pub const MARKER_END: &str = "# END tunelito";

pub const SSHD_CONFIG: &str = r"C:\ProgramData\ssh\sshd_config";
pub const ADMIN_KEYS: &str = r"C:\ProgramData\ssh\administrators_authorized_keys";
pub const USER_HOME: &str = r"C:\Users\tunelito";
pub const USER_KEYS: &str = r"C:\Users\tunelito\.ssh\authorized_keys";
pub const USER_SSH_DIR: &str = r"C:\Users\tunelito\.ssh";
pub const HELLO_DIR: &str = r"C:\ProgramData\tunelito\hellos";
pub const STATE_DIR: &str = r"C:\ProgramData\tunelito";
pub const REQUEST_FILE: &str = r"C:\ProgramData\tunelito\request.json";
pub const RESULT_FILE: &str = r"C:\ProgramData\tunelito\result.json";
pub const PRIV_INSTALL: &str = r"C:\ProgramData\tunelito\bin\tunelito-priv.exe";
pub const TASK_NAME: &str = "TunelitoPriv";

/// OpenSSH ignores `~\.ssh\authorized_keys` when the account is an administrator
/// and reads `administrators_authorized_keys` instead. `tunelito` is created
/// outside that group. An account that already was an administrator keeps the
/// administrators file.
pub fn authorized_keys_path(account_is_admin: bool) -> &'static str {
    if account_is_admin {
        ADMIN_KEYS
    } else {
        USER_KEYS
    }
}

pub fn under_root(root: &Path, absolute: &str) -> PathBuf {
    let trimmed = absolute
        .trim_start_matches(r"C:\")
        .trim_start_matches("C:/");
    let mut path = root.to_path_buf();
    for part in trimmed.split(['\\', '/']) {
        if !part.is_empty() {
            path.push(part);
        }
    }
    path
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsFilePlan {
    pub set_global_max_sessions: bool,
    pub add_sftp: bool,
    pub account_is_admin: bool,
    pub authorized_key_lines: Vec<String>,
    pub write_keys: bool,
}

impl WindowsFilePlan {
    pub fn from_probe(probe: &Probe, key_lines: Vec<String>, write_keys: bool) -> Self {
        let (set_global, add_sftp) = dropin_decisions(&probe.effective(), probe.openssh_installed);
        Self {
            set_global_max_sessions: set_global,
            add_sftp,
            account_is_admin: probe.windows_user_is_admin,
            authorized_key_lines: key_lines,
            write_keys,
        }
    }
}

/// Rewrite `sshd_config`. Foreign lines stay. `MaxSessions` is raised before the
/// first `Match` (OpenSSH keeps the first value). The `tunelito` Match is last.
pub fn rewrite_sshd_config(
    existing: &str,
    set_global_max_sessions: bool,
    add_sftp: bool,
) -> Result<String, Error> {
    let stripped = strip_marked_block(existing, MARKER_BEGIN, MARKER_END);
    let mut lines: Vec<String> = stripped.lines().map(|line| line.to_string()).collect();
    let first_match = lines
        .iter()
        .position(|line| is_match_line(line))
        .unwrap_or(lines.len());
    if set_global_max_sessions {
        if let Some(index) = lines[..first_match]
            .iter()
            .position(|line| is_global_key(line, "maxsessions"))
        {
            lines[index] = "MaxSessions 10".into();
        } else {
            lines.insert(first_match, "MaxSessions 10".into());
        }
    }
    let first_match = lines
        .iter()
        .position(|line| is_match_line(line))
        .unwrap_or(lines.len());
    if add_sftp
        && !lines[..first_match]
            .iter()
            .any(|line| is_global_key(line, "subsystem"))
    {
        lines.insert(first_match, "Subsystem sftp sftp-server.exe".into());
    }
    let mut body = lines.join("\n");
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    if !body.is_empty() && !body.ends_with("\n\n") {
        body.push('\n');
    }
    body.push_str(&match_block());
    validate_match_block(&match_block())?;
    if body.to_ascii_lowercase().contains("new-netfirewallrule") {
        return Err(Error::unsafe_sshd_dropin());
    }
    Ok(body)
}

pub fn strip_tunelito_block(existing: &str) -> String {
    let mut text = strip_marked_block(existing, MARKER_BEGIN, MARKER_END);
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    text
}

fn match_block() -> String {
    format!(
        "{MARKER_BEGIN}\n\
         # Managed by tunelito. Safe to delete; the app recreates it.\n\
         Match User {DEDICATED_USER}\n    \
         AuthorizedKeysFile C:/Users/tunelito/.ssh/authorized_keys\n    \
         PasswordAuthentication no\n    \
         KbdInteractiveAuthentication no\n    \
         PubkeyAuthentication yes\n    \
         AuthenticationMethods publickey\n    \
         MaxSessions 10\n    \
         X11Forwarding no\n    \
         AllowTcpForwarding no\n    \
         ClientAliveInterval 30\n    \
         ClientAliveCountMax 4\n\
         {MARKER_END}\n"
    )
}

fn validate_match_block(text: &str) -> Result<(), Error> {
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.contains('|') || line.contains(';') || line.contains('`') || line.contains('$') {
            return Err(Error::unsafe_sshd_dropin());
        }
        if line.eq_ignore_ascii_case(&format!("Match User {DEDICATED_USER}")) {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("").trim();
        let allowed = [
            "AuthorizedKeysFile",
            "PasswordAuthentication",
            "KbdInteractiveAuthentication",
            "PubkeyAuthentication",
            "AuthenticationMethods",
            "MaxSessions",
            "X11Forwarding",
            "AllowTcpForwarding",
            "ClientAliveInterval",
            "ClientAliveCountMax",
        ];
        if !allowed.contains(&key) || value.is_empty() {
            return Err(Error::unsafe_sshd_dropin());
        }
    }
    Ok(())
}

fn is_match_line(line: &str) -> bool {
    line.trim().to_ascii_lowercase().starts_with("match ")
}

fn is_global_key(line: &str, key: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }
    trimmed
        .split_whitespace()
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case(key))
}

fn strip_marked_block(existing: &str, begin: &str, end: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in existing.lines() {
        if line.trim() == begin {
            skipping = true;
            continue;
        }
        if skipping {
            if line.trim() == end {
                skipping = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

pub fn write_windows_access_files(
    root: &Path,
    plan: &WindowsFilePlan,
) -> Result<Vec<PathBuf>, Error> {
    for line in &plan.authorized_key_lines {
        if line.trim().is_empty() {
            continue;
        }
        let parsed = parse_public_keys(line)?;
        if parsed.len() != 1 {
            return Err(Error::invalid_public_key());
        }
    }
    let config_path = under_root(root, SSHD_CONFIG);
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    let rewritten = rewrite_sshd_config(&existing, plan.set_global_max_sessions, plan.add_sftp)?;
    write_text(&config_path, &rewritten)?;
    let mut written = vec![config_path];
    if plan.write_keys {
        let path = under_root(root, authorized_keys_path(plan.account_is_admin));
        let existing_keys = fs::read_to_string(&path).unwrap_or_default();
        let body = upsert_authorized_keys(&existing_keys, &plan.authorized_key_lines);
        write_text(&path, &body)?;
        written.push(path);
    }
    let hellos = under_root(root, HELLO_DIR);
    fs::create_dir_all(&hellos)?;
    written.push(hellos);
    Ok(written)
}

fn write_text(path: &Path, contents: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

pub fn acl_commands(account_is_admin: bool) -> Vec<CommandStep> {
    let mut steps = Vec::new();
    if account_is_admin {
        steps.push(ps(
            "acl_authorized_keys",
            &acl_script(ADMIN_KEYS, &["*S-1-5-32-544:(F)"]),
        ));
    } else {
        // The profile directory must keep Administrators. OpenSSH rejects an
        // authorized_keys file that other accounts can write.
        steps.push(ps(
            "acl_profile",
            &acl_script(
                USER_HOME,
                &["*S-1-5-32-544:(OI)(CI)F", "tunelito:(OI)(CI)F"],
            ),
        ));
        steps.push(ps(
            "acl_ssh_dir",
            &acl_script(
                USER_SSH_DIR,
                &["*S-1-5-32-544:(OI)(CI)F", "tunelito:(OI)(CI)F"],
            ),
        ));
        steps.push(ps(
            "acl_authorized_keys",
            &acl_script(USER_KEYS, &["*S-1-5-32-544:(F)", "tunelito:(R)"]),
        ));
    }
    steps.push(ps(
        "acl_request_dir",
        &acl_script(STATE_DIR, &["*S-1-5-32-545:(OI)(CI)F"]),
    ));
    steps
}

fn acl_script(path: &str, grants: &[&str]) -> String {
    let mut args = String::from("'*S-1-5-18:(OI)(CI)F'");
    for grant in grants {
        args.push_str(" '");
        args.push_str(&grant.replace('\'', ""));
        args.push('\'');
    }
    format!(
        "$path = '{path}'; \
         if (-not (Test-Path -LiteralPath $path)) {{ throw \"missing $path\" }}; \
         $icacls = Join-Path $env:SystemRoot 'System32\\icacls.exe'; \
         & $icacls $path /inheritance:r; \
         if ($LASTEXITCODE -ne 0) {{ throw \"icacls inheritance failed on $path\" }}; \
         & $icacls $path /grant:r {args}; \
         if ($LASTEXITCODE -ne 0) {{ throw \"icacls grant failed on $path\" }}",
        path = path.replace('\'', ""),
    )
}

/// Create the Windows profile before any file is written under `C:\Users\tunelito`.
/// A plain directory there makes Windows put the real profile somewhere else,
/// and sshd then ignores the key file.
pub fn create_profile_command() -> CommandStep {
    ps(
        "create_user_profile",
        r#"$user = [ADSI]'WinNT://./tunelito,user'; $sid = (New-Object System.Security.Principal.SecurityIdentifier($user.InvokeGet('objectSID'), 0)).Value; if ([string]::IsNullOrWhiteSpace($sid)) { throw 'tunelito has no SID' }; $reg = "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$sid"; $dest = 'C:\Users\tunelito'; if ((Test-Path -LiteralPath $dest) -and -not (Test-Path -LiteralPath $reg)) { Remove-Item -LiteralPath $dest -Recurse -Force }; if (-not (Test-Path -LiteralPath $reg)) { if (-not ('Tunelito.NativeProfile' -as [type])) { Add-Type -Namespace Tunelito -Name NativeProfile -MemberDefinition '[DllImport("userenv.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern int CreateProfile(string pszUserSid, string pszUserName, System.Text.StringBuilder pszProfilePath, uint cchProfilePath);' }; $buf = New-Object System.Text.StringBuilder 260; $code = [Tunelito.NativeProfile]::CreateProfile($sid, 'tunelito', $buf, [uint32]260); if ($code -ne 0 -and $code -ne -2147024713) { throw "CreateProfile failed: $code path=$($buf.ToString())" } }; $image = (Get-ItemProperty -LiteralPath $reg -Name ProfileImagePath).ProfileImagePath; if ($image -ne $dest) { throw "profile path is [$image]" }; if (-not (Test-Path -LiteralPath $dest)) { throw 'profile directory missing' }"#,
    )
}

pub fn register_task_command(priv_exe: &str) -> Result<CommandStep, Error> {
    if priv_exe.trim().is_empty()
        || priv_exe
            .chars()
            .any(|c| c == '"' || c == '\n' || c == '\r' || c == '&' || c == '|')
    {
        return Err(Error::prepare_failed(
            "refusing to register that helper path",
        ));
    }
    let run = format!(
        "\"{priv_exe}\" apply --request \"{REQUEST_FILE}\" --root C:\\ --execute --result \"{RESULT_FILE}\" --windows"
    );
    Ok(CommandStep::new(
        OsKind::Windows,
        "schtasks",
        vec![
            "/Create".into(),
            "/F".into(),
            "/TN".into(),
            TASK_NAME.into(),
            "/SC".into(),
            "ONCE".into(),
            "/ST".into(),
            "00:00".into(),
            "/SD".into(),
            "01/01/2099".into(),
            "/RL".into(),
            "HIGHEST".into(),
            "/RU".into(),
            "SYSTEM".into(),
            "/TR".into(),
            run,
        ],
        "register_priv_task",
    ))
}

pub fn run_task_command() -> CommandStep {
    CommandStep::new(
        OsKind::Windows,
        "schtasks",
        vec!["/Run".into(), "/TN".into(), TASK_NAME.into()],
        "run_priv_task",
    )
}

pub fn uninstall_commands() -> Vec<CommandStep> {
    vec![
        CommandStep::new(
            OsKind::Windows,
            "schtasks",
            vec![
                "/Delete".into(),
                "/F".into(),
                "/TN".into(),
                TASK_NAME.into(),
            ],
            "delete_priv_task",
        ),
        ps(
            "remove_dedicated_user",
            "Remove-LocalUser -Name 'tunelito' -ErrorAction SilentlyContinue",
        ),
    ]
}

pub fn close_firewall_command() -> CommandStep {
    ps(
        "close_public_ssh",
        "try { Remove-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -ErrorAction SilentlyContinue; Get-NetFirewallRule -DisplayName 'OpenSSH SSH Server*' -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction SilentlyContinue } catch {}",
    )
}

fn ps(purpose: &'static str, script: &str) -> CommandStep {
    crate::plan::powershell_step(purpose, script)
}

pub fn commands_open_firewall(steps: &[CommandStep]) -> bool {
    steps.iter().any(|step| {
        let line = step.line().to_ascii_lowercase();
        line.contains("new-netfirewallrule") || line.contains("advfirewall") && line.contains("add")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STOCK: &str = "\
Port 22\n\
MaxSessions 4\n\
Subsystem sftp sftp-server.exe\n\
\n\
Match Group administrators\n\
    AuthorizedKeysFile __PROGRAMDATA__/ssh/administrators_authorized_keys\n";

    #[test]
    fn raises_maxsessions_before_match_and_appends_tunelito_last() {
        let once = rewrite_sshd_config(STOCK, true, false).unwrap();
        let twice = rewrite_sshd_config(&once, true, false).unwrap();
        assert_eq!(once, twice);
        assert!(once.contains("Port 22\n"));
        assert!(once.contains("MaxSessions 10\n"));
        assert!(!once.contains("MaxSessions 4"));
        assert!(once.contains("Match Group administrators\n"));
        let tunelito = once.find("Match User tunelito").unwrap();
        let admins = once.find("Match Group administrators").unwrap();
        assert!(admins < tunelito);
        assert!(once.trim_end().ends_with("# END tunelito"));
        assert!(!once.to_ascii_lowercase().contains("forcecommand"));
        assert!(!once.to_ascii_lowercase().contains("new-netfirewallrule"));
    }

    #[test]
    fn adds_windows_sftp_only_when_missing() {
        let bare = "Port 22\n";
        let with = rewrite_sshd_config(bare, false, true).unwrap();
        assert!(with.contains("Subsystem sftp sftp-server.exe\n"));
        let again = rewrite_sshd_config(&with, false, true).unwrap();
        assert_eq!(again.matches("Subsystem sftp").count(), 1);
        assert!(!again.contains("internal-sftp"));
    }

    #[test]
    fn admin_account_uses_the_administrators_key_file() {
        assert_eq!(authorized_keys_path(false), USER_KEYS);
        assert_eq!(authorized_keys_path(true), ADMIN_KEYS);
        let user = acl_commands(false);
        let admin = acl_commands(true);
        let user_blob = user.iter().map(|s| s.line()).collect::<Vec<_>>().join("\n");
        let admin_blob = admin
            .iter()
            .map(|s| s.line())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(user_blob.contains(USER_KEYS));
        assert!(user_blob.contains("S-1-5-18"));
        assert!(user_blob.contains("tunelito:(R)"));
        assert!(user_blob.contains("S-1-5-32-545"));
        assert!(user_blob.contains("S-1-5-32-544"));
        assert!(!user_blob.contains(ADMIN_KEYS));
        assert!(admin_blob.contains(ADMIN_KEYS));
        assert!(admin_blob.contains("S-1-5-32-544"));
        assert!(!admin_blob.contains(USER_KEYS));
        assert!(!user_blob.to_ascii_lowercase().contains("everyone"));
        assert!(!admin_blob.to_ascii_lowercase().contains("everyone"));
        assert!(!commands_open_firewall(&user));
        assert!(!commands_open_firewall(&admin));
    }

    #[test]
    fn task_is_on_demand_and_uninstall_keeps_the_capability() {
        let task = register_task_command(PRIV_INSTALL).unwrap();
        let line = task.line().to_ascii_lowercase();
        assert!(line.contains("once"));
        assert!(!line.contains("onstart"));
        assert!(!line.contains("onlogon"));
        assert!(!task.is_forbidden_autostart());
        assert!(register_task_command("C:\\evil\" & calc.exe").is_err());
        let uninstall = uninstall_commands();
        let blob = uninstall
            .iter()
            .map(|s| s.line())
            .collect::<Vec<_>>()
            .join("\n")
            .to_ascii_lowercase();
        assert!(blob.contains("remove-localuser"));
        assert!(!blob.contains("remove-windowscapability"));
        assert!(!blob.contains("add-localgroupmember"));
        assert!(!commands_open_firewall(&uninstall));
    }

    #[test]
    fn fake_root_preserves_a_foreign_key_and_the_stock_config() {
        let root = std::env::temp_dir().join(format!("tunelito-win-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let config = under_root(&root, SSHD_CONFIG);
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, STOCK).unwrap();
        let key_path = under_root(&root, USER_KEYS);
        fs::create_dir_all(key_path.parent().unwrap()).unwrap();
        fs::write(&key_path, "ssh-ed25519 AAAAforeign kept\n").unwrap();
        let dir = root.join("key");
        fs::create_dir_all(&dir).unwrap();
        let (parsed, _) = crate::keys::ensure_keypair(&dir.join("k"), "helper").unwrap();
        let plan = WindowsFilePlan {
            set_global_max_sessions: true,
            add_sftp: false,
            account_is_admin: false,
            authorized_key_lines: vec![parsed.public_key.clone()],
            write_keys: true,
        };
        write_windows_access_files(&root, &plan).unwrap();
        let config_text = fs::read_to_string(&config).unwrap();
        assert!(config_text.contains("Match Group administrators"));
        assert!(config_text.contains("Match User tunelito"));
        let keys = fs::read_to_string(under_root(&root, USER_KEYS)).unwrap();
        assert!(keys.contains("AAAAforeign"));
        assert!(keys.contains(&parsed.public_key));
        assert!(!under_root(&root, ADMIN_KEYS).exists());
        let _ = fs::remove_dir_all(&root);
    }
}
