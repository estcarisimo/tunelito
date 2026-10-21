# Claude Desktop

One use of the SSH session. Not required.

On macOS and Linux, Desktop uses OpenSSH and reads `~/.ssh/config`. Add the hostname from the block and connect as `tunelito`. `cloudflared` has to be on `PATH`.

On Windows, Desktop ships its own SSH client and ignores `%USERPROFILE%\.ssh\config`, so the `ProxyCommand` is skipped. It can be switched to system `ssh.exe`. The menu has moved between versions. **Check the build you have.** This repo does not invent a menu path. `ssh` from a terminal works once OpenSSH and `cloudflared` are installed.

Desktop's remote installer uses SFTP and several channels, which is why `MaxSessions` is at least 10. Claude Code lands in the `tunelito` account's `~/.claude/remote/`, not in the personal home directory.

A session shows in Details: green dot, user, key fingerprint, `127.0.0.1`. No hello says "no client info". Turning the host off drops the session.
