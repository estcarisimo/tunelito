# 🚇 Tunelito

[![CI](https://github.com/estcarisimo/tunelito/actions/workflows/ci.yml/badge.svg)](https://github.com/estcarisimo/tunelito/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.83%2B-orange.svg)](https://www.rust-lang.org)

A one-click desktop app for letting **someone you know** SSH into this computer through NAT. The helper can then open a Claude Desktop SSH session (which installs Claude Code under the remote account) and use local devices, such as a TV attached over USB.

Tunelito is for a friend helping a friend. It is not a support tool for a stranger, a popup, or a phone call.

![Tunelito in English, with access on and someone connected](docs/images/tunelito-en.png)

![Tunelito en español](docs/images/tunelito-es.png)

The pictures above are the real interface with sample data. They are not a live tunnel.

## ✨ Features

- A large **on / off** switch, in English and Spanish.
- Paste the helper's public key (the one they sent you, for example on WhatsApp). Each key shows its SHA256 fingerprint, comment, and the date it was added.
- While the switch is on, Tunelito prepares OpenSSH with an SFTP subsystem and `MaxSessions` of at least 10, which Claude Desktop needs.
- Remote access uses a dedicated account named `tunelito`. That account is not an administrator.
- A Cloudflare **quick tunnel** (`cloudflared tunnel --url ssh://localhost:22`) with no Cloudflare account. The app shows the `trycloudflare.com` name, a Copy button, and a ready-to-paste `~/.ssh/config` block for ordinary OpenSSH.
- The switch turns itself off (2 hours by default, from 15 minutes to 24 hours). Nothing starts at boot.
- A **someone is connected** indicator, and a list of sessions with a green dot while the session is open and a gray dot after it ends.
- An optional helper "hello", sent by `tunelito-connect` if you want it, that reports a public IP, ASN, reverse DNS, and OS. The host labels those fields **reported by the client (unverified)**. A session with no hello still shows the key fingerprint and the times, and says **no client info**. Through the tunnel, sshd only sees `127.0.0.1`.

## 🛡️ Security

Turning this on gives the holder of an accepted key a shell as `tunelito`, and access to the devices that account can open. That is enough to read files that account can read, install Claude Code under that account, and drive something like a TV over USB when the permissions allow it.

**If someone you do not know asks you to install Tunelito or to turn the switch on, say no.** That request is the shape of a tech-support scam: urgency, a pasted key, and a tunnel name.

What Tunelito does about that:

- The warning stays on screen. Turning the switch on asks you to confirm.
- The app does not start at login. sshd is not left enabled at boot when Tunelito installed it. cloudflared is not installed as a service.
- The tunnel name is not a password. Without a key in the accepted list, the tunnel does not let anyone in. The key is the secret, and it should arrive from the person you already trust.
- Turning the switch off kills the tunnel and removes the managed lines from `authorized_keys`.
- The hello cannot prove who connected. sshd's own log shows the loopback address because cloudflared connects locally.

The longer write-up is [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md). To report a vulnerability, see [SECURITY.md](SECURITY.md).

## 🚀 Quick Start

You need two computers: this one (the **host**, with the window) and the helper's. The helper uses ordinary OpenSSH and `cloudflared`. The small `tunelito-connect` program is optional.

**On the helper's computer**

Install [cloudflared](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/) and leave it on `PATH` (`brew install cloudflared`, the Debian package from Cloudflare, or `winget install --id Cloudflare.cloudflared`).

If you do not already have an ed25519 key, create one with OpenSSH. Send only the public line. Never send the private file.

```bash
ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -C "helper"
cat ~/.ssh/id_ed25519.pub
```

**On the host**

1. Install Tunelito for your system. The longer notes are in the [docs](https://estcarisimo.github.io/tunelito/): Debian, Windows, and macOS.
2. Paste the public key and press **Add key**. Compare the fingerprint with your helper if you want to be sure the message was not altered.
3. Press the switch, read the confirmation, and turn it on. Approve the administrator prompt. On Linux, Tunelito uses it to install or start OpenSSH and to create the `tunelito` account.
4. Press **Copy SSH config** and send that block back.

**Back on the helper's computer**

Paste the block into `~/.ssh/config`. It sets `User tunelito` and `ProxyCommand cloudflared access ssh --hostname %h`. Then:

```bash
ssh the-name.trycloudflare.com
```

Or add that same name as an SSH environment in Claude Desktop. macOS and Linux Desktop use OpenSSH and read `~/.ssh/config`. On Windows, Desktop's built-in SSH client does not, and switching it to system OpenSSH **needs verification**. See [docs/CLAUDE_DESKTOP.md](docs/CLAUDE_DESKTOP.md).

If the public key you sent is not `~/.ssh/id_ed25519.pub`, change the `IdentityFile` line in the block so it points at the matching private key.

## 📖 Usage

### The host window

| Control | What it does |
| --- | --- |
| Switch | On prepares SSH and opens the tunnel. Off kills the tunnel and pulls the managed keys out of `authorized_keys`. |
| Timer | How long until it turns off by itself. The choice is kept. |
| Forget keys | When checked, turning off also removes the keys from this app. Off by default: the keys stay in the list, but they cannot be used until you turn the switch on again. |
| Copy | Copies the tunnel name, or the `~/.ssh/config` block. |
| Remove | Drops one key from the list. If the switch is on, that key is removed from `authorized_keys` as well. |
| Connections | One row per SSH session: green while it is open, gray after it ends, with the key fingerprint and the start and end times. With no hello, the row says **no client info**. |

Closing the window turns access off. Opening the app later does **not** turn it back on.

The Spanish copy uses Rioplatense voseo ("Dejá", "Encendelo", "conocés").

### Connecting

The block the Copy button produces is ordinary OpenSSH. The helper pastes it into `~/.ssh/config` and runs `ssh` with that name. `cloudflared` has to be on their `PATH`, because the block's `ProxyCommand` is `cloudflared access ssh --hostname %h`.

`StrictHostKeyChecking accept-new` trusts the host key the first time that name is seen. Quick-tunnel names are new each time the switch is turned on.

### Claude Desktop

Claude Desktop on macOS and Linux uses OpenSSH and honors `~/.ssh/config`, so the pasted block is the one Desktop will use. Add that hostname as an SSH environment and connect as `tunelito`.

On Windows, Claude Desktop ships its own SSH client and does not, by default, read OpenSSH's config. Switching it to the system OpenSSH client is required and **needs verification** against the Desktop build you have. See [docs/CLAUDE_DESKTOP.md](docs/CLAUDE_DESKTOP.md).

### Optional: `tunelito-connect`

You do not need this to connect. It is a small extra for two jobs: creating a key if you would rather not run `ssh-keygen` yourself, and sending the unverified hello (public IP, ASN, reverse DNS, OS).

```bash
cargo install --path crates/tunelito-connect
tunelito-connect
```

That prints a public key from `~/.ssh/tunelito_ed25519` (mode `0600`). It never overwrites an existing key. Send only the public line.

| Command | What it does |
| --- | --- |
| `tunelito-connect` | Same as `key`. |
| `tunelito-connect install HOST` | Writes the managed `Host` block, and adds a `LocalCommand` that sends the hello. |
| `tunelito-connect hello --hostname HOST` | Sends the unverified hello once. |
| `tunelito-connect connect HOST` | Installs the block and sends the hello. |
| `tunelito-connect install-cloudflared` | Downloads the pinned `cloudflared` release after checking its SHA256. |

The hello's `LocalCommand` ends with `|| true`, so a failed hello does not block SSH. `--once` skips a repeat hello for ten minutes. A plain `ssh` session that never runs it still shows up on the host, with **no client info**.

Claude Code's remote installer uses SFTP and several SSH channels. That is why Tunelito raises `MaxSessions` to at least 10 and enables the SFTP subsystem. The install lands under `/home/tunelito/.claude/remote/` (or the equivalent home on Windows and macOS), **not** in the personal home directory of the person at the keyboard.

### What the helper can reach

The `tunelito` account may be added to `plugdev` or `adbusers` when those groups already exist, so a USB device such as a TV can be opened. It is never added to `sudo`, `wheel`, `docker`, `disk`, `dialout`, or `input`. Tunelito does not install `adb` and does not write udev rules. A TV can still be permission-denied until those are set up by hand.

## 🔧 Configuration

The window stores state under the XDG state directory (`~/.local/share/tunelito` on Linux) unless `TUNELITO_STATE_DIR` is set. These variables are for development and tests. The window does not expose them.

| Variable | Effect |
| --- | --- |
| `TUNELITO_STATE_DIR` | Where `state.json`, the cloudflared binary, and the tunnel log are kept. |
| `TUNELITO_ROOT` | When set to a directory other than `/`, the app writes the drop-in and `authorized_keys` under that directory and does **not** run system commands. |
| `TUNELITO_PRIV_BIN` | Path to `tunelito-priv`, the small root helper. Also searched next to the app and in `/usr/lib/tunelito/`. |
| `TUNELITO_SSH_DIR` | Overrides `~/.ssh` for `tunelito-connect`. |
| `TUNELITO_SSH_CONFIG` | Overrides the config file `tunelito-connect` edits. |

On Debian, the OpenSSH drop-in is `/etc/ssh/sshd_config.d/00-tunelito.conf`. It only contains a `Match User tunelito` block (password login off, public key required, X11 and TCP forwarding off, `MaxSessions 10`) plus a global `MaxSessions 10` or an SFTP subsystem line when the running config is missing them. `sshd` uses the first value it sees. The file is checked against an allowlist before it is written. `ForceCommand` is not part of that allowlist.

`cloudflared` is pinned to **2026.10.2**. The SHA256 values live in `crates/tunelito-core`. A download that does not match is discarded.

## 🏗️ Architecture

```
helper laptop                         this computer
ssh + ~/.ssh/config                   Tunelito window (Tauri)
  cloudflared access ssh                Rust core: keys, timer, sessions
       |                                tunelito-priv (root, Linux)
       |                                  openssh + user tunelito
       +------------ trycloudflare ------- ssh://127.0.0.1:22
```

| Crate | Role |
| --- | --- |
| `tunelito-core` | Keys, sshd plans, the on/off state machine, log parsing, hello parsing, checksums. No service is started from here. |
| `tunelito-connect` | Optional helper CLI: key generation and the unverified hello. |
| `tunelito-priv` | Linux-only root helper. `--execute` is refused unless the root is `/` and the process is root. |
| `app` | The window. It applies the core's effects: prepare SSH, start or stop the tunnel, record sessions. |

Session rows come from OpenSSH logs (`journalctl` on Linux, `log show` on macOS, the OpenSSH Operational log on Windows) and from `ps` lines that look like `sshd: tunelito@pts` or `@notty`. A `[priv]` process is not a session. If the process list is available, an "accepted" line with no matching process is closed after a short grace period.

Windows and macOS **command lists** are unit-tested on Linux. The window does not run them yet: on those systems turning the switch on returns `needs_admin` and the coverage note. See [docs/PLATFORM_SUPPORT.md](docs/PLATFORM_SUPPORT.md).

## 🧪 Development

Linux is the system CI runs on. You need Rust 1.83 or newer (stable is what `rust-toolchain.toml` asks for), Node 22, and the Tauri 2 system libraries (`libwebkit2gtk-4.1-dev`, GTK 3, `librsvg`, `patchelf`).

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd app && npm ci && npm run build
python3 -m venv .docs-venv && .docs-venv/bin/pip install -r docs/requirements.txt
.docs-venv/bin/mkdocs build --strict
```

The frontend can be opened without the window:

```bash
cd app && npm run dev
# http://127.0.0.1:1420/?preview=connected
# http://127.0.0.1:1420/?preview=connected&lang=es
```

`?preview=connected` is sample data. It does not open a tunnel. Do not point a test at a real `trycloudflare.com` name from CI.

Icons are regenerated with `python3 scripts/make_icons.py`.

A local Linux run of the window needs `tunelito-priv` next to the binary (or `TUNELITO_PRIV_BIN`) and an administrator prompt (`pkexec` on a desktop, otherwise `sudo`).

## 📊 Example Output

The public key the helper sends:

```text
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... helper
```

Host, after the switch is on, the block **Copy SSH config** produces:

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

The helper then runs `ssh maple-river-42.trycloudflare.com`.

A session with no hello lists the fingerprint and the times, and says no client info. If `tunelito-connect hello` ran, the panel adds a block labeled reported by the client (unverified). The address OpenSSH recorded stays `127.0.0.1`. The hello JSON always stores:

```json
{
  "tunelito_hello": 1,
  "public_ip": "203.0.113.10",
  "asn": "AS64500",
  "os": "linux",
  "verification": "unverified"
}
```

## 🤝 Contributing

Issues and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) and the [code of conduct](CODE_OF_CONDUCT.md).

Please do not send changes that start the tunnel or sshd at boot, that run commands as the person sitting at the computer, or that treat the hello as proof of identity.

## 📄 License

[MIT](LICENSE) © 2026 Esteban Carisimo.

## 🙏 Acknowledgments

- [Cloudflare](https://github.com/cloudflare/cloudflared) for `cloudflared` quick tunnels.
- [Tauri](https://tauri.app/) for the desktop window.
- OpenSSH, for the server Claude Desktop already knows how to speak.
- The people who will refuse to turn this on for a stranger.
