# Connect with SSH

This is the helper's path. You use the OpenSSH client that is already on your computer, plus `cloudflared`. You do not need `tunelito-connect` to get a shell.

## 1. Install cloudflared

Install the official binary and make sure `cloudflared` is on your `PATH`. Cloudflare publishes builds for macOS, Windows, and Linux. The usual package names are `brew install cloudflared` and `winget install --id Cloudflare.cloudflared`. On Debian, use Cloudflare's package repository rather than an unpinned download.

The host app downloads its own checksum-checked `cloudflared` for the tunnel it opens. That copy stays on the host. Your `ProxyCommand` uses the one on your machine.

## 2. Send a public key

If you do not already have an ed25519 key:

```bash
ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -C "helper"
```

Send the one line in `~/.ssh/id_ed25519.pub` to the person at the host, for example on WhatsApp. Never send the private file.

They paste it into Tunelito, check the SHA256 fingerprint with you if you want to be sure the message was not changed, and turn the switch on.

## 3. Paste the config block

They press **Copy SSH config** and send you a block like this:

```text
# BEGIN tunelito maple-river-42.trycloudflare.com
Host maple-river-42.trycloudflare.com
HostName maple-river-42.trycloudflare.com
User tunelito
IdentityFile ~/.ssh/id_ed25519
IdentitiesOnly yes
StrictHostKeyChecking accept-new
ProxyCommand cloudflared access ssh --hostname %h
# END tunelito maple-river-42.trycloudflare.com
```

Paste it into `~/.ssh/config`. If the public key you sent is not `~/.ssh/id_ed25519.pub`, change `IdentityFile` to the matching private key.

The name changes every time they turn the switch on. Paste the new block, or replace the old one. The `BEGIN` / `END` markers are how `tunelito-connect install` finds its own block. Editing the file by hand, you can replace the previous Tunelito block yourself.

`StrictHostKeyChecking accept-new` trusts the host key the first time that name is seen. It is not `StrictHostKeyChecking no`.

## 4. Connect

```bash
ssh maple-river-42.trycloudflare.com
```

Use the name from the block. You are logged in as `tunelito`. Claude Code, if Desktop installs it, lands in that account's `~/.claude/remote/`, not in the home directory of the person at the keyboard.

A session with no hello still shows on their window: a green dot while you are connected, the key fingerprint, the user, and the start time. The client-info line says **no client info**. The address their SSH server records is `127.0.0.1`, because `cloudflared` on their machine connects to localhost.

## Claude Desktop

On macOS and Linux, Claude Desktop uses OpenSSH and reads `~/.ssh/config`. Add the hostname from the block as an SSH environment and connect as `tunelito`. `cloudflared` still has to be on `PATH` when Desktop launches `ssh`.

On Windows, Claude Desktop uses a built-in SSH client by default. That client does not honor `%USERPROFILE%\.ssh\config`, so the `ProxyCommand` is ignored. Desktop can be switched to the system OpenSSH (`ssh.exe` from the Windows OpenSSH feature). The place where that switch lives has changed across Desktop versions. **This needs verification** on the build you have. This repository does not invent a menu path. Until you have confirmed it, do not assume Desktop on Windows will use the pasted block. `ssh` from a terminal will, once OpenSSH and `cloudflared` are installed.

SFTP and `MaxSessions` of at least 10 are there because Desktop's remote installer opens several channels at once.

## Optional: tunelito-connect

`tunelito-connect` does two extra jobs. You can skip both.

- `tunelito-connect` with no arguments creates `~/.ssh/tunelito_ed25519` (mode `0600`) if it is missing, and prints the public key. It never overwrites a key that is already there.
- `tunelito-connect install the-name.trycloudflare.com` writes the host block and adds a `LocalCommand` that sends a hello: a public IP, an ASN, a reverse DNS name, and an OS string. The host stores that hello as **unverified**. The command ends with `|| true`, so a failed hello does not stop SSH.

```bash
cargo install --path crates/tunelito-connect
tunelito-connect
```

If you use this key, set `IdentityFile` to `~/.ssh/tunelito_ed25519`. The block the host copies assumes `~/.ssh/id_ed25519`, which is the plain `ssh-keygen` path.
