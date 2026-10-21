use crate::error::Error;
use crate::keys::{validate_user, DEDICATED_USER};

pub const MANAGED_BEGIN: &str = "# BEGIN tunelito";
pub const MANAGED_END: &str = "# END tunelito";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshHostEntry {
    pub hostname: String,
    pub user: String,
    pub identity_file: String,
    pub hello_hook: bool,
}

impl SshHostEntry {
    /// The block the host window copies. Plain OpenSSH: no hello hook.
    /// `tunelito-connect install` turns the hook on when the helper wants it.
    pub fn for_host(hostname: impl Into<String>) -> Self {
        Self {
            hostname: hostname.into(),
            user: DEDICATED_USER.to_string(),
            identity_file: "~/.ssh/id_ed25519".to_string(),
            hello_hook: false,
        }
    }
}

pub fn render_host_block(entry: &SshHostEntry) -> Result<String, Error> {
    crate::cloudflared::validate_hostname(&entry.hostname)?;
    validate_user(&entry.user)?;
    if entry
        .identity_file
        .chars()
        .any(|c| c.is_whitespace() || c == '\n')
    {
        return Err(Error::invalid_hostname());
    }
    let mut block = format!(
        "{MANAGED_BEGIN} {host}\n\
         Host {host}\n\
         HostName {host}\n\
         User {user}\n\
         IdentityFile {identity}\n\
         IdentitiesOnly yes\n\
         StrictHostKeyChecking accept-new\n\
         ProxyCommand cloudflared access ssh --hostname %h\n",
        host = entry.hostname,
        user = entry.user,
        identity = entry.identity_file,
    );
    if entry.hello_hook {
        block.push_str(
            "PermitLocalCommand yes\n\
             LocalCommand sh -c 'command -v tunelito-connect >/dev/null 2>&1 && tunelito-connect hello --hostname %h --quiet --once || true'\n",
        );
    }
    block.push_str(&format!("{MANAGED_END} {}\n", entry.hostname));
    Ok(block)
}

pub fn upsert_host_block(existing: &str, entry: &SshHostEntry) -> Result<String, Error> {
    let block = render_host_block(entry)?;
    let begin = format!("{MANAGED_BEGIN} {}", entry.hostname);
    let end = format!("{MANAGED_END} {}", entry.hostname);
    Ok(replace_marked_block(existing, &begin, &end, &block))
}

pub fn replace_marked_block(existing: &str, begin: &str, end: &str, block: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    let mut replaced = false;
    for line in existing.lines() {
        if line.trim() == begin {
            skipping = true;
            if !replaced {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(block);
                if !block.ends_with('\n') {
                    out.push('\n');
                }
                replaced = true;
            }
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
    if skipping {
        // Unclosed marker: drop the partial block we skipped and append a clean one.
    }
    if !replaced {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.is_empty() && !out.ends_with("\n\n") {
            // Keep a single separating blank line when the file already has content.
        }
        out.push_str(block);
        if !block.ends_with('\n') {
            out.push('\n');
        }
    }
    if !existing.is_empty() && !existing.ends_with('\n') && out.ends_with('\n') {
        // Preserve a trailing newline for files we rewrote line by line.
    }
    out
}

pub fn null_ssh_config_arg() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_has_the_required_proxy_command_and_is_idempotent() {
        let entry = SshHostEntry::for_host("lime-eagle-4821.trycloudflare.com");
        let block = render_host_block(&entry).unwrap();
        assert!(block.contains("ProxyCommand cloudflared access ssh --hostname %h\n"));
        assert!(block.contains("User tunelito\n"));
        assert!(block.contains("IdentityFile ~/.ssh/id_ed25519\n"));
        assert!(block.contains("IdentitiesOnly yes\n"));
        assert!(!block.contains("LocalCommand"));
        assert!(!block.contains("tunelito-connect"));
        let foreign = "Host example\n    User me\n";
        let once = upsert_host_block(foreign, &entry).unwrap();
        let twice = upsert_host_block(&once, &entry).unwrap();
        assert_eq!(once, twice);
        assert!(twice.contains("Host example\n"));
        assert_eq!(twice.matches("# BEGIN tunelito").count(), 1);
    }

    #[test]
    fn hello_hook_is_opt_in() {
        let mut entry = SshHostEntry::for_host("lime-eagle-4821.trycloudflare.com");
        entry.hello_hook = true;
        let block = render_host_block(&entry).unwrap();
        assert!(block.contains("PermitLocalCommand yes\n"));
        assert!(block.contains("tunelito-connect hello"));
        assert!(block.contains("|| true"));
    }

    #[test]
    fn rejects_root_as_the_remote_user() {
        let mut entry = SshHostEntry::for_host("lime-eagle-4821.trycloudflare.com");
        entry.user = "root".into();
        assert_eq!(render_host_block(&entry).unwrap_err().code, "refused_user");
    }
}
