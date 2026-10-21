use crate::cloudflared::OsKind;
use crate::error::Error;
use crate::keys::{filter_device_groups, validate_user, DEDICATED_USER};
use crate::sshd::{dropin_decisions, render_dropin, validate_dropin, SshdEffective};

/// A command the privileged helper may run. Arguments are fixed by the planner.
/// Windows and macOS steps are never executed in CI; see `executed_in_ci`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandStep {
    pub os: OsKind,
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub purpose: &'static str,
    /// Changes a setting that survives reboot. Linux plans keep this false.
    pub boot_persistent: bool,
    /// CI on Linux executes none of the service-control commands. The arguments are unit-tested.
    pub executed_in_ci: bool,
}

impl CommandStep {
    pub(crate) fn new(os: OsKind, program: &str, args: Vec<String>, purpose: &'static str) -> Self {
        Self {
            os,
            program: program.into(),
            args,
            env: Vec::new(),
            purpose,
            boot_persistent: false,
            executed_in_ci: false,
        }
    }

    pub fn line(&self) -> String {
        if let Some((_, script)) = self.env.iter().find(|(key, _)| key == "TUNELITO_PS") {
            return format!("{} {script}", self.program);
        }
        format!("{} {}", self.program, self.args.join(" "))
    }

    pub fn is_forbidden_autostart(&self) -> bool {
        let line = self.line().to_lowercase();
        line.contains("systemctl enable")
            || line.contains("cloudflared service install")
            || line.contains("startuptype automatic")
            || line.contains("--startup-type automatic")
            || (line.contains("set-service") && line.contains("automatic"))
            || line.contains("launchctl load")
            || line.contains("autostart")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub os: OsKind,
    pub ssh_unit: String,
    pub openssh_installed: bool,
    pub sshd_active: bool,
    pub sshd_enabled_at_boot: Option<bool>,
    pub sshd_config_read: bool,
    pub max_sessions: Option<u32>,
    pub sftp_enabled: bool,
    pub user_exists: bool,
    pub groups_present: Vec<String>,
    pub remote_login_on: bool,
    pub windows_capability_installed: bool,
    pub windows_startup_automatic: bool,
    /// True when the dedicated account is already in Administrators.
    /// OpenSSH then ignores `~\.ssh\authorized_keys`.
    pub windows_user_is_admin: bool,
}

impl Probe {
    pub fn linux_fresh() -> Self {
        Self {
            os: OsKind::Linux,
            ssh_unit: "ssh".into(),
            openssh_installed: false,
            sshd_active: false,
            sshd_enabled_at_boot: None,
            sshd_config_read: false,
            max_sessions: None,
            sftp_enabled: false,
            user_exists: false,
            groups_present: vec!["plugdev".into()],
            remote_login_on: false,
            windows_capability_installed: false,
            windows_startup_automatic: false,
            windows_user_is_admin: false,
        }
    }

    pub fn effective(&self) -> SshdEffective {
        SshdEffective {
            max_sessions: self.max_sessions,
            sftp_enabled: self.sftp_enabled,
            config_read: self.sshd_config_read,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedPaths {
    pub authorized_keys: String,
    pub dropin: String,
    pub hello_dir: String,
    pub user_home: String,
}

pub fn managed_paths(os: OsKind) -> ManagedPaths {
    match os {
        OsKind::Linux => ManagedPaths {
            authorized_keys: "/home/tunelito/.ssh/authorized_keys".into(),
            dropin: "/etc/ssh/sshd_config.d/00-tunelito.conf".into(),
            hello_dir: "/var/lib/tunelito/hellos".into(),
            user_home: "/home/tunelito".into(),
        },
        // UNTESTED ON CI: path layout is the documented Apple default, not checked on a Mac.
        OsKind::Macos => ManagedPaths {
            authorized_keys: "/Users/tunelito/.ssh/authorized_keys".into(),
            dropin: "/etc/ssh/sshd_config".into(),
            hello_dir: "/Library/Application Support/tunelito/hellos".into(),
            user_home: "/Users/tunelito".into(),
        },
        // UNTESTED ON CI: Windows OpenSSH optional-capability paths.
        OsKind::Windows => ManagedPaths {
            authorized_keys: r"C:\Users\tunelito\.ssh\authorized_keys".into(),
            dropin: r"C:\ProgramData\ssh\sshd_config".into(),
            hello_dir: r"C:\ProgramData\tunelito\hellos".into(),
            user_home: r"C:\Users\tunelito".into(),
        },
    }
}

pub fn install_commands(probe: &Probe) -> Result<Vec<CommandStep>, Error> {
    validate_unit(&probe.ssh_unit)?;
    let mut steps = Vec::new();
    match probe.os {
        OsKind::Linux => {
            if !probe.openssh_installed {
                let mut install = CommandStep::new(
                    OsKind::Linux,
                    "apt-get",
                    strs(&["install", "-y", "openssh-server"]),
                    "install_openssh",
                );
                install
                    .env
                    .push(("DEBIAN_FRONTEND".into(), "noninteractive".into()));
                steps.push(install);
                // The Debian package enables ssh at boot. Undo that; Tunelito starts it on demand.
                steps.push(CommandStep::new(
                    OsKind::Linux,
                    "systemctl",
                    vec!["disable".into(), probe.ssh_unit.clone()],
                    "prevent_boot_autostart",
                ));
            }
            if !probe.user_exists {
                steps.push(CommandStep::new(
                    OsKind::Linux,
                    "useradd",
                    strs(&[
                        "--create-home",
                        "--shell",
                        "/bin/bash",
                        "--comment",
                        "Tunelito remote helper",
                        "--user-group",
                        DEDICATED_USER,
                    ]),
                    "create_dedicated_user",
                ));
            }
            let groups = filter_device_groups(probe.groups_present.iter().map(String::as_str));
            if !groups.is_empty() {
                steps.push(CommandStep::new(
                    OsKind::Linux,
                    "usermod",
                    vec!["-aG".into(), groups.join(","), DEDICATED_USER.to_string()],
                    "device_groups",
                ));
            }
        }
        OsKind::Macos => {
            // UNTESTED ON CI: macOS execution.
            if !probe.remote_login_on {
                let mut persistent = CommandStep::new(
                    OsKind::Macos,
                    "systemsetup",
                    strs(&["-setremotelogin", "on"]),
                    "enable_remote_login",
                );
                persistent.boot_persistent = true;
                steps.push(persistent);
            }
            if !probe.user_exists {
                steps.push(CommandStep::new(
                    OsKind::Macos,
                    "sysadminctl",
                    strs(&[
                        "-addUser",
                        DEDICATED_USER,
                        "-fullName",
                        "Tunelito",
                        "-shell",
                        "/bin/zsh",
                        "-password",
                        "-",
                    ]),
                    "create_dedicated_user",
                ));
            }
        }
        OsKind::Windows => {
            // UNTESTED ON CI: Windows execution.
            if !probe.windows_capability_installed {
                steps.push(windows_ps(
                    "install_openssh",
                    "Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0",
                ));
                steps.push(windows_ps(
                    "prevent_boot_autostart",
                    "Set-Service -Name sshd -StartupType Manual",
                ));
                // The capability opens port 22 on the network. The tunnel is local.
                steps.push(crate::windows::close_firewall_command());
            } else if probe.windows_startup_automatic {
                steps.push(windows_ps(
                    "prevent_boot_autostart",
                    "Set-Service -Name sshd -StartupType Manual",
                ));
            }
            // A blank password is refused for network logon, including public-key SSH.
            // Always ensure the account: an ADSI existence check reports a missing user as present.
            steps.push(windows_ps(
                "create_dedicated_user",
                "$net = Join-Path $env:SystemRoot 'System32\\net.exe'; $log = Join-Path $env:TEMP 'tunelito-net-user.txt'; & $net user tunelito > $log 2>&1; if ($LASTEXITCODE -ne 0) { $alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789'.ToCharArray(); $bytes = New-Object byte[] 32; [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes); $plain = -join ($bytes | ForEach-Object { $alphabet[$_ % $alphabet.Length] }); if ($plain.Length -lt 20) { throw 'password was not generated' }; & $net user tunelito $plain /add > $log 2>&1; $code = $LASTEXITCODE; $add = [string](Get-Content -Path $log -Raw -ErrorAction SilentlyContinue); if ($code -ne 0) { throw \"net user /add failed ($code): $add\" } }; & $net user tunelito /active:yes > $log 2>&1; $activeCode = $LASTEXITCODE; $activeText = [string](Get-Content -Path $log -Raw -ErrorAction SilentlyContinue); if ($activeCode -ne 0) { throw \"tunelito is not an active account ($activeCode): $activeText\" }; $account = Get-CimInstance -ClassName Win32_UserAccount -Filter \"Name='tunelito' and LocalAccount=True\"; if (-not $account) { throw \"account missing after net user: $activeText\" }; try { $user = [ADSI]('WinNT://' + $env:COMPUTERNAME + '/tunelito,user'); $flags = [int]$user.Get('UserFlags'); $user.Put('UserFlags', ($flags -bor 65536)); $user.SetInfo() } catch {}",
            ));
            steps.push(crate::windows::create_profile_command());
        }
    }
    Ok(steps)
}

pub fn activate_commands(probe: &Probe) -> Result<Vec<CommandStep>, Error> {
    validate_unit(&probe.ssh_unit)?;
    let mut steps = Vec::new();
    match probe.os {
        OsKind::Linux => {
            if !probe.sshd_active {
                steps.push(CommandStep::new(
                    OsKind::Linux,
                    "systemctl",
                    vec!["start".into(), probe.ssh_unit.clone()],
                    "start_sshd",
                ));
            }
        }
        OsKind::Macos => {
            steps.push(CommandStep::new(
                OsKind::Macos,
                "launchctl",
                strs(&["kickstart", "-k", "system/com.openssh.sshd"]),
                "start_sshd",
            ));
        }
        OsKind::Windows => {
            steps.push(windows_ps("start_sshd", "Start-Service sshd"));
        }
    }
    Ok(steps)
}

pub fn reload_command(probe: &Probe) -> Result<CommandStep, Error> {
    validate_unit(&probe.ssh_unit)?;
    Ok(match probe.os {
        OsKind::Linux => CommandStep::new(
            OsKind::Linux,
            "systemctl",
            vec!["reload".into(), probe.ssh_unit.clone()],
            "reload_sshd",
        ),
        OsKind::Macos => CommandStep::new(
            OsKind::Macos,
            "launchctl",
            strs(&["kickstart", "-k", "system/com.openssh.sshd"]),
            "reload_sshd",
        ),
        OsKind::Windows => windows_ps("reload_sshd", "Restart-Service sshd"),
    })
}

pub fn disable_commands(
    probe: &Probe,
    started_by_us: bool,
    stop_if_started: bool,
) -> Result<Vec<CommandStep>, Error> {
    validate_unit(&probe.ssh_unit)?;
    let mut steps = Vec::new();
    if started_by_us && stop_if_started {
        match probe.os {
            OsKind::Linux => steps.push(CommandStep::new(
                OsKind::Linux,
                "systemctl",
                vec!["stop".into(), probe.ssh_unit.clone()],
                "stop_sshd",
            )),
            OsKind::Macos => steps.push(CommandStep::new(
                OsKind::Macos,
                "launchctl",
                strs(&["kill", "SIGTERM", "system/com.openssh.sshd"]),
                "stop_sshd",
            )),
            OsKind::Windows => {
                steps.push(windows_ps("stop_sshd", "Stop-Service sshd"));
            }
        }
    }
    // If we were the ones who turned Remote Login on, turn it back off.
    if probe.os == OsKind::Macos && started_by_us && !probe.remote_login_on {
        let mut step = CommandStep::new(
            OsKind::Macos,
            "systemsetup",
            strs(&["-setremotelogin", "off"]),
            "disable_remote_login",
        );
        step.boot_persistent = true;
        steps.push(step);
    }
    Ok(steps)
}

pub fn planned_dropin(probe: &Probe) -> Result<Option<String>, Error> {
    if probe.os != OsKind::Linux && probe.os != OsKind::Macos && probe.os != OsKind::Windows {
        return Ok(None);
    }
    let (global, sftp) = dropin_decisions(&probe.effective(), probe.openssh_installed);
    let text = render_dropin(global, sftp);
    validate_dropin(&text)?;
    Ok(Some(text))
}

pub fn validate_unit(unit: &str) -> Result<(), Error> {
    if unit == "ssh" || unit == "sshd" {
        Ok(())
    } else {
        Err(Error::prepare_failed("ssh unit must be ssh or sshd"))
    }
}

pub fn coverage_note(os: OsKind) -> &'static str {
    match os {
        OsKind::Linux => {
            "Linux command arguments are unit-tested. CI does not install packages, create users, or start sshd."
        }
        OsKind::Macos => {
            "UNTESTED ON CI: macOS Remote Login, sysadminctl, and launchctl are not executed here."
        }
        OsKind::Windows => {
            "Windows OpenSSH setup is executed on windows-latest. The Linux job only checks this command text."
        }
    }
}

pub fn assert_user_constant() -> Result<(), Error> {
    validate_user(DEDICATED_USER)
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

fn windows_ps(purpose: &'static str, script: &str) -> CommandStep {
    powershell_step(purpose, script)
}

/// `-EncodedCommand` is the script itself. A `-Command` string is reparsed by
/// powershell.exe, which drops quotes and breaks module autoload.
pub(crate) fn powershell_step(purpose: &'static str, script: &str) -> CommandStep {
    let body = format!(
        "$ProgressPreference = 'SilentlyContinue'; $ErrorActionPreference = 'Stop'; try {{ {script} }} catch {{ [Console]::Out.WriteLine($_.Exception.Message); exit 1 }}"
    );
    let mut step = CommandStep::new(
        OsKind::Windows,
        "powershell.exe",
        strs(&[
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-EncodedCommand",
            &encode_powershell_command(&body),
        ]),
        purpose,
    );
    step.env.push(("TUNELITO_PS".into(), body));
    step
}

pub fn encode_powershell_command(script: &str) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = script
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    let mut out = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = if index + 1 < bytes.len() {
            bytes[index + 1]
        } else {
            0
        };
        let b2 = if index + 2 < bytes.len() {
            bytes[index + 2]
        } else {
            0
        };
        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(TABLE[((triple >> 18) & 63) as usize] as char);
        out.push(TABLE[((triple >> 12) & 63) as usize] as char);
        if index + 1 < bytes.len() {
            out.push(TABLE[((triple >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if index + 2 < bytes.len() {
            out.push(TABLE[(triple & 63) as usize] as char);
        } else {
            out.push('=');
        }
        index += 3;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_no_autostart(steps: &[CommandStep]) {
        for step in steps {
            assert!(!step.is_forbidden_autostart(), "{}", step.line());
            assert!(!step.executed_in_ci);
        }
    }

    #[test]
    fn linux_fresh_install_does_not_enable_sshd_at_boot() {
        let probe = Probe::linux_fresh();
        let steps = install_commands(&probe).unwrap();
        assert_no_autostart(&steps);
        assert!(steps.iter().any(|s| s.purpose == "install_openssh"));
        assert!(steps.iter().any(|s| s.purpose == "prevent_boot_autostart"));
        assert!(steps
            .iter()
            .any(|s| s.line().contains("systemctl disable ssh")));
        assert!(steps.iter().all(|s| !s.boot_persistent));
        assert!(steps
            .iter()
            .all(|s| !s.line().to_lowercase().contains("sudo")));
        let groups = steps.iter().find(|s| s.purpose == "device_groups").unwrap();
        assert!(groups.line().contains("plugdev"));
        assert!(!groups.line().contains("sudo"));
        assert!(!groups.line().contains("input"));
    }

    #[test]
    fn linux_already_enabled_is_left_alone() {
        let mut probe = Probe::linux_fresh();
        probe.openssh_installed = true;
        probe.sshd_enabled_at_boot = Some(true);
        probe.sshd_active = true;
        probe.user_exists = true;
        probe.groups_present.clear();
        let steps = install_commands(&probe).unwrap();
        assert!(steps.is_empty());
        assert!(activate_commands(&probe).unwrap().is_empty());
    }

    #[test]
    fn poisoned_groups_are_dropped() {
        let mut probe = Probe::linux_fresh();
        probe.user_exists = true;
        probe.openssh_installed = true;
        probe.groups_present = vec![
            "sudo".into(),
            "input".into(),
            "wheel".into(),
            "plugdev".into(),
        ];
        let steps = install_commands(&probe).unwrap();
        let line = steps
            .iter()
            .find(|s| s.purpose == "device_groups")
            .unwrap()
            .line();
        assert_eq!(line, "usermod -aG plugdev tunelito");
    }

    #[test]
    fn windows_install_sets_manual_startup_and_skips_administrators() {
        let mut probe = Probe::linux_fresh();
        probe.os = OsKind::Windows;
        let steps = install_commands(&probe).unwrap();
        assert_no_autostart(&steps);
        let blob = steps
            .iter()
            .map(|s| s.line())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(blob.contains("OpenSSH.Server~~~~0.0.1.0"));
        assert!(blob.contains("StartupType Manual"));
        assert!(blob.contains("net.exe"));
        assert!(blob.contains("65536"));
        assert!(blob.contains("CreateProfile"));
        assert!(blob.contains("ProfileImagePath"));
        assert!(!blob.contains("-NoPassword"));
        let create = steps
            .iter()
            .find(|step| step.purpose == "create_dedicated_user")
            .unwrap();
        assert!(create.args.iter().any(|arg| arg == "-EncodedCommand"));
        assert!(create.args.iter().all(|arg| !arg.contains('"')));
        assert!(create
            .env
            .iter()
            .any(|(key, value)| key == "TUNELITO_PS" && value.contains("net.exe")));
        let encoded = create
            .args
            .iter()
            .position(|arg| arg == "-EncodedCommand")
            .and_then(|index| create.args.get(index + 1))
            .unwrap();
        let script = create
            .env
            .iter()
            .find(|(key, _)| key == "TUNELITO_PS")
            .unwrap()
            .1
            .as_str();
        assert_eq!(encoded, &encode_powershell_command(script));
        assert!(blob.contains("Remove-NetFirewallRule"));
        assert!(!blob.to_lowercase().contains("administrators"));
        assert!(!blob.to_lowercase().contains("automatic"));
        assert!(!blob.to_lowercase().contains("new-netfirewallrule"));
        assert!(coverage_note(OsKind::Windows).contains("windows-latest"));
        assert!(steps.iter().all(|s| !s.is_forbidden_autostart()));
    }

    #[test]
    fn macos_remote_login_is_undone_when_we_enabled_it() {
        let mut probe = Probe::linux_fresh();
        probe.os = OsKind::Macos;
        probe.remote_login_on = false;
        let enable = install_commands(&probe).unwrap();
        assert!(enable
            .iter()
            .any(|s| s.boot_persistent && s.purpose == "enable_remote_login"));
        assert_no_autostart(&enable);
        let disable = disable_commands(&probe, true, true).unwrap();
        assert!(disable.iter().any(|s| s.purpose == "disable_remote_login"));
        assert!(disable.iter().any(|s| s.purpose == "stop_sshd"));
        assert!(coverage_note(OsKind::Macos).contains("UNTESTED ON CI"));
    }

    #[test]
    fn dropin_for_a_low_maxsessions_validates() {
        let mut probe = Probe::linux_fresh();
        probe.openssh_installed = true;
        probe.sshd_config_read = true;
        probe.max_sessions = Some(4);
        probe.sftp_enabled = true;
        let dropin = planned_dropin(&probe).unwrap().unwrap();
        assert!(dropin.contains("MaxSessions 10"));
        validate_dropin(&dropin).unwrap();
    }

    #[test]
    fn rejects_a_weird_unit_name() {
        let mut probe = Probe::linux_fresh();
        probe.ssh_unit = "ssh;rm".into();
        assert!(install_commands(&probe).is_err());
    }
}
