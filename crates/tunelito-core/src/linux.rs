//! Linux distributions and the polkit pieces that keep the switch to one prompt.
//!
//! The Linux job checks this text. It does not install packages or polkit rules.

use crate::cloudflared::OsKind;
use crate::error::Error;
use crate::keys::DEDICATED_USER;
use crate::plan::CommandStep;

/// Where the `.deb` installs the helper. The polkit action names this exact path.
pub const PRIV_PATH: &str = "/usr/lib/tunelito/tunelito-priv";
pub const POLKIT_ACTION: &str = "io.github.estcarisimo.tunelito.priv";
/// Written after the first approved setup. Removed by the package's postrm.
pub const POLKIT_RULE: &str = "/etc/polkit-1/rules.d/49-tunelito.rules";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packages {
    Apt,
    Dnf,
    Zypper,
    Pacman,
    Unknown,
}

impl Packages {
    /// First package manager found, in this order. `exists` answers for a program name.
    pub fn detect(exists: impl Fn(&str) -> bool) -> Self {
        for (program, kind) in [
            ("apt-get", Self::Apt),
            ("dnf", Self::Dnf),
            ("zypper", Self::Zypper),
            ("pacman", Self::Pacman),
        ] {
            if exists(program) {
                return kind;
            }
        }
        Self::Unknown
    }

    /// Debian and Ubuntu call the unit `ssh`. Fedora, openSUSE and Arch call it `sshd`.
    pub fn ssh_unit(self) -> &'static str {
        match self {
            Self::Apt => "ssh",
            _ => "sshd",
        }
    }

    pub fn install_openssh(self) -> Result<CommandStep, Error> {
        let step = match self {
            Self::Apt => {
                let mut step = step("apt-get", &["install", "-y", "openssh-server"]);
                step.env
                    .push(("DEBIAN_FRONTEND".into(), "noninteractive".into()));
                step
            }
            Self::Dnf => step("dnf", &["install", "-y", "openssh-server"]),
            Self::Zypper => step(
                "zypper",
                &["--non-interactive", "install", "openssh-server"],
            ),
            Self::Pacman => step("pacman", &["-S", "--needed", "--noconfirm", "openssh"]),
            Self::Unknown => {
                return Err(Error::prepare_failed(
                    "install the OpenSSH server with your package manager, then try again",
                ))
            }
        };
        Ok(step)
    }
}

fn step(program: &str, args: &[&str]) -> CommandStep {
    CommandStep::new(
        OsKind::Linux,
        program,
        args.iter().map(|arg| (*arg).to_string()).collect(),
        "install_openssh",
    )
}

/// A login name that may get the polkit rule: a plain local user, not root and
/// not the remote account.
pub fn validate_login(name: &str) -> Result<(), Error> {
    let ok = !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b == b'_')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        && name != "root"
        && name != DEDICATED_USER;
    if ok {
        Ok(())
    } else {
        Err(Error::prepare_failed(
            "refusing that account for the polkit rule",
        ))
    }
}

/// Lets the person who approved setup switch access again without a password,
/// from an active local session only. Every other caller still gets the
/// action's default: an administrator password.
pub fn render_polkit_rule(login: &str) -> Result<String, Error> {
    validate_login(login)?;
    Ok(format!(
        "// Managed by tunelito. Lets {login} switch Tunelito access without a password.\n\
         // Delete this file to get the password prompt back.\n\
         polkit.addRule(function(action, subject) {{\n    \
         if (action.id == \"{POLKIT_ACTION}\" && subject.user == \"{login}\" &&\n        \
         subject.local && subject.active) {{\n        \
         return polkit.Result.YES;\n    \
         }}\n\
         }});\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: &str =
        include_str!("../../../app/src-tauri/linux/io.github.estcarisimo.tunelito.policy");
    const POSTRM: &str = include_str!("../../../app/src-tauri/linux/postrm.sh");

    #[test]
    fn each_package_manager_installs_the_server_without_enabling_it() {
        let found = |names: &'static [&'static str]| move |p: &str| names.contains(&p);
        assert_eq!(Packages::detect(found(&["apt-get", "dnf"])), Packages::Apt);
        assert_eq!(Packages::detect(found(&["dnf"])), Packages::Dnf);
        assert_eq!(Packages::detect(found(&["zypper"])), Packages::Zypper);
        assert_eq!(Packages::detect(found(&["pacman"])), Packages::Pacman);
        assert_eq!(Packages::detect(found(&[])), Packages::Unknown);
        assert_eq!(Packages::Apt.ssh_unit(), "ssh");
        for kind in [Packages::Dnf, Packages::Zypper, Packages::Pacman] {
            assert_eq!(kind.ssh_unit(), "sshd");
        }
        for kind in [
            Packages::Apt,
            Packages::Dnf,
            Packages::Zypper,
            Packages::Pacman,
        ] {
            let step = kind.install_openssh().unwrap();
            let line = step.line();
            assert!(line.contains("openssh"), "{line}");
            assert!(!step.is_forbidden_autostart(), "{line}");
            assert!(!line.contains("enable"), "{line}");
        }
        assert!(Packages::Unknown.install_openssh().is_err());
    }

    #[test]
    fn polkit_rule_names_one_local_active_user() {
        let rule = render_polkit_rule("alice").unwrap();
        assert!(rule.contains(&format!("action.id == \"{POLKIT_ACTION}\"")));
        assert!(rule.contains("subject.user == \"alice\""));
        assert!(rule.contains("subject.local && subject.active"));
        assert_eq!(rule.matches("polkit.Result.YES").count(), 1);
        assert!(!rule.contains("subject.isInGroup"));
        for bad in [
            "",
            "root",
            "tunelito",
            "Alice",
            "9lives",
            "a\" || true || \"",
            "a b",
            "alice;",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(render_polkit_rule(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn shipped_policy_matches_the_helper_path_and_asks_for_admin() {
        assert!(POLICY.contains(&format!("<action id=\"{POLKIT_ACTION}\">")));
        assert!(POLICY.contains(&format!(
            "<annotate key=\"org.freedesktop.policykit.exec.path\">{PRIV_PATH}</annotate>"
        )));
        assert!(POLICY.contains("<allow_any>no</allow_any>"));
        assert!(POLICY.contains("<allow_inactive>no</allow_inactive>"));
        assert!(POLICY.contains("<allow_active>auth_admin_keep</allow_active>"));
        assert!(!POLICY.contains("<allow_active>yes</allow_active>"));
        assert!(POSTRM.contains(POLKIT_RULE));
    }
}
