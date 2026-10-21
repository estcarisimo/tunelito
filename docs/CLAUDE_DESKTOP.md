# Claude Desktop SSH sessions

Tunelito's host side is an OpenSSH server reached through `cloudflared`. The helper connects with ordinary OpenSSH. Claude Desktop can use that same host as an SSH environment. Its remote installer places Claude Code under `~/.claude/remote/` of the account it logged into. With Tunelito that account is `tunelito`, so the path is `/home/tunelito/.claude/remote/` on Linux (the platform home of `tunelito` on macOS and Windows).

The installer speaks SFTP and opens more than one SSH channel. Tunelito's drop-in turns the SFTP subsystem on when `sshd -T` shows it missing, and it sets `MaxSessions 10` when the effective value is lower.

## The config block

On the host, copy the SSH config block and paste it into `~/.ssh/config` on the helper's computer. `cloudflared` has to be on `PATH`. The block's proxy line is:

```text
ProxyCommand cloudflared access ssh --hostname %h
```

The user is `tunelito`. Then `ssh the-name.trycloudflare.com` is the same path Desktop will use, once Desktop is actually launching OpenSSH.

## macOS and Linux

Claude Desktop uses the system OpenSSH client. OpenSSH reads `~/.ssh/config`. Add the hostname from the pasted block as an SSH environment.

This matches OpenSSH's documented config loading. It still depends on Desktop not being pointed at a different SSH binary. If a session ignores the `ProxyCommand`, check which `ssh` Desktop is launching.

## Windows

**This needs verification against a current Claude Desktop build.**

On Windows, Claude Desktop uses a built-in SSH client by default. That client does not honor `%USERPROFILE%\.ssh\config`, so the `ProxyCommand` in the pasted block is ignored and the connection will not go through the tunnel.

Desktop can be switched to the system OpenSSH client (the optional Windows OpenSSH feature, `ssh.exe`). The menu or settings path for that switch has changed across Desktop versions, and this repository does not invent one. Confirm it in the build you are using, then connect to the hostname in the pasted block.

Until that switch is confirmed, treat "it works on Windows Desktop" as untested.

## What you should see

A successful session shows up in the host's Connections panel with a green dot, the `tunelito` user, the key fingerprint from the OpenSSH log, and the start time. The observed address is `127.0.0.1`.

A plain `ssh` or Desktop session does not send a hello. The row then says **no client info**. That is expected. If someone also ran `tunelito-connect hello`, a second block appears, labeled as reported by the client and unverified.

Turning the host switch off drops the tunnel. The Desktop session ends with it.
