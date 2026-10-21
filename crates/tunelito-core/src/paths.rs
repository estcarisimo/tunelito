use std::path::PathBuf;

/// Directories the host app uses. Override with `TUNELITO_STATE_DIR` in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub state_dir: PathBuf,
    pub bin_dir: PathBuf,
    pub log_file: PathBuf,
    pub hellos_read_dirs: Vec<PathBuf>,
}

impl AppPaths {
    pub fn resolve() -> Self {
        if let Some(dir) = std::env::var_os("TUNELITO_STATE_DIR") {
            return Self::under(PathBuf::from(dir));
        }
        Self::under(default_state_dir())
    }

    pub fn under(state_dir: PathBuf) -> Self {
        let mut hellos_read_dirs = vec![
            PathBuf::from("/var/lib/tunelito/hellos"),
            state_dir.join("hellos"),
            PathBuf::from("/home/tunelito/.tunelito/hellos"),
        ];
        if cfg!(target_os = "macos") {
            hellos_read_dirs.push(PathBuf::from("/Users/tunelito/.tunelito/hellos"));
        }
        if cfg!(windows) {
            hellos_read_dirs.push(PathBuf::from(r"C:\ProgramData\tunelito\hellos"));
        }
        Self {
            bin_dir: state_dir.join("bin"),
            log_file: state_dir.join("tunelito.log"),
            hellos_read_dirs,
            state_dir,
        }
    }

    pub fn cloudflared_path(&self) -> PathBuf {
        let name = if cfg!(windows) {
            "cloudflared.exe"
        } else {
            "cloudflared"
        };
        self.bin_dir.join(name)
    }

    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join("state.json")
    }

    pub fn stamp_dir(&self) -> PathBuf {
        self.state_dir.join("hello-stamps")
    }
}

pub fn default_state_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("tunelito");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Library/Application Support/tunelito");
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("tunelito");
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/state/tunelito");
    }
    std::env::temp_dir().join("tunelito")
}

pub fn ssh_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TUNELITO_SSH_DIR") {
        return PathBuf::from(dir);
    }
    home_dir().join(".ssh")
}

pub fn identity_path() -> PathBuf {
    ssh_dir().join("tunelito_ed25519")
}

pub fn ssh_config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("TUNELITO_SSH_CONFIG") {
        return PathBuf::from(path);
    }
    ssh_dir().join("config")
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_override_is_honored_by_under() {
        let paths = AppPaths::under(PathBuf::from("/tmp/tunelito-test-state"));
        assert_eq!(
            paths.state_file(),
            PathBuf::from("/tmp/tunelito-test-state/state.json")
        );
        assert!(paths
            .hellos_read_dirs
            .iter()
            .any(|p| p.ends_with("var/lib/tunelito/hellos") || p.ends_with("hellos")));
    }
}
