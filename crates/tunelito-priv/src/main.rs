use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use tunelito_core::access::{write_linux_access_files, AccessRequest};
use tunelito_core::plan::{
    activate_commands, disable_commands, install_commands, reload_command, Probe,
};
use tunelito_core::Error;

/// Apply a validated access request. `--execute` is refused unless `--root /` and euid is 0.
#[derive(Debug, Parser)]
#[command(name = "tunelito-priv", version)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, clap::Subcommand)]
enum Cmd {
    Apply {
        #[arg(long)]
        request: PathBuf,
        #[arg(long, default_value = "/")]
        root: PathBuf,
        /// Run system commands. Refused when root is not `/` or the process is not root.
        #[arg(long)]
        execute: bool,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("tunelito-priv: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Apply {
            request,
            root,
            execute,
        } => apply(&request, &root, execute),
    }
}

fn apply(
    request_path: &std::path::Path,
    root: &std::path::Path,
    execute: bool,
) -> Result<(), Error> {
    let text = fs::read_to_string(request_path)?;
    let request: AccessRequest = serde_json::from_str(&text)
        .map_err(|err| Error::with_detail("prepare_failed", err.to_string()))?;
    if execute {
        if root != std::path::Path::new("/") {
            return Err(Error::prepare_failed(
                "refusing to run system commands against a fake root",
            ));
        }
        if !is_root() {
            return Err(Error::needs_admin());
        }
        if request.prepare {
            let probe = probe_linux();
            for step in install_commands(&probe)? {
                run_step(&step)?;
            }
        }
    }
    let written = write_linux_access_files(root, &request)?;
    for path in &written {
        println!("{}", path.display());
    }
    if execute {
        if Path::new("/home/tunelito/.ssh").exists() {
            let _ = Command::new("chown")
                .args(["-R", "tunelito:tunelito", "/home/tunelito/.ssh"])
                .status();
        }
        if let Some(dropin) = written
            .iter()
            .find(|path| path.ends_with("00-tunelito.conf"))
        {
            let _ = Command::new("chown").arg("root:root").arg(dropin).status();
        }
        let probe = probe_linux();
        if request.reload_sshd {
            run_step(&reload_command(&probe)?)?;
        }
        if request.start_sshd {
            let mut starting = probe.clone();
            starting.sshd_active = false;
            for step in activate_commands(&starting)? {
                run_step(&step)?;
            }
        }
        if request.stop_sshd {
            for step in disable_commands(&probe, true, true)? {
                run_step(&step)?;
            }
        }
    }
    Ok(())
}

fn run_step(step: &tunelito_core::plan::CommandStep) -> Result<(), Error> {
    let status = Command::new(&step.program)
        .args(&step.args)
        .envs(step.env.iter().cloned())
        .status()
        .map_err(|err| Error::prepare_failed(err.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::prepare_failed(format!(
            "{} {} failed",
            step.program,
            step.args.join(" ")
        )))
    }
}

fn systemctl_is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim() == "0")
        .unwrap_or(false)
}

fn probe_linux() -> Probe {
    let mut probe = Probe::linux_fresh();
    probe.groups_present.clear();
    probe.openssh_installed = Command::new("sh")
        .args([
            "-c",
            "command -v sshd >/dev/null || dpkg -s openssh-server >/dev/null 2>&1",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    probe.user_exists = Command::new("id")
        .args(["-u", "tunelito"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    probe.sshd_active = systemctl_is_active("ssh") || systemctl_is_active("sshd");
    for group in ["plugdev", "adbusers"] {
        let present = Command::new("getent")
            .args(["group", group])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if present {
            probe.groups_present.push(group.into());
        }
    }
    probe.groups_present.dedup();
    probe
}
