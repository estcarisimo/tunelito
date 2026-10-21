# 🚇 Tunelito

[![CI](https://github.com/estcarisimo/tunelito/actions/workflows/ci.yml/badge.svg)](https://github.com/estcarisimo/tunelito/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.83%2B-orange.svg)](https://www.rust-lang.org)

Tunelito lets you connect to the computer of someone you want to help — a friend, your mom, your brother, your cousin, your partner — and fix it or work on it.

They don't need to know how any of this works. The computer sits behind NAT or CGNAT, so port forwarding isn't an option. They download one file, open it, and paste the key you sent them.

Claude Code over SSH, and USB devices, are one way to use that session. They are not the point.

**Only with someone you know.** A stranger who asks them to install this and paste a key is a scam.

Send them [Para vos](https://estcarisimo.github.io/tunelito/es/guia/): one page, in Spanish, with the screens. This file is for people working on Tunelito.

![Tunelito in English, access on](docs/images/tunelito-en.png)

![Tunelito en español](docs/images/tunelito-es.png)

Sample data. Not a live tunnel.

## ✨ Features

- One file to open. Windows: `tunelito.exe`. macOS: the app inside the `.dmg`. Debian: `.deb` or AppImage.
- One administrator prompt the first time. Setup stays quiet after that.
- One field: paste the key. One button. The window shows an address with Copy and Share.
- Keys, sessions, the timer, and the SSH snippet sit behind Details.
- A dedicated account named `tunelito`, not an administrator.
- A Cloudflare quick tunnel, no Cloudflare account. Plain OpenSSH on the helper's side.
- Turns itself off (15 minutes to 24 hours, default 2 hours). Nothing starts at boot.

## 🛡️ Security

Turning it on gives the holder of that key a shell as `tunelito`.

- The warning is one line. The button asks you to confirm.
- The address is not a password. Without the key, the tunnel does not let anyone in.
- Off kills the tunnel and removes the managed lines from `authorized_keys`.
- sshd only sees `127.0.0.1`. A client hello is labeled unverified. No hello still shows the session, as "no client info".

Details: [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md). Reports: [SECURITY.md](SECURITY.md).

## 🚀 Quick Start

**On their computer.** Send them [Para vos](https://estcarisimo.github.io/tunelito/es/guia/). They download `tunelito.exe`, open it, accept the Windows screens, paste the public key, and send the address back.

**On yours.** Install `cloudflared`. Send them only the public key (`ssh-keygen -t ed25519`). When the address arrives, use the SSH block under Details, then:

```bash
ssh the-name.trycloudflare.com
```

The block sets `User tunelito` and `ProxyCommand cloudflared access ssh --hostname %h`. `tunelito-connect` is optional. It can print a key and send an unverified hello. You do not need it to connect.

macOS and Linux Claude Desktop read `~/.ssh/config`. Windows Desktop's built-in SSH client does not, and switching it to system OpenSSH needs verification. See [docs/CLAUDE_DESKTOP.md](docs/CLAUDE_DESKTOP.md).

## 📖 Usage

| What you see | What it does |
| --- | --- |
| The field and the button | A valid key plus the button turns access on, after the one-line confirm. |
| Copy / Share | The `trycloudflare.com` address. |
| Details | Keys, sessions, timer, SSH snippet, forget-keys checkbox. |
| Off, or closing the window | Kills the tunnel. Opening the app later does not turn it back on. |

The Spanish UI uses Rioplatense voseo ("Pegá", "encendés").

`tunelito-connect` commands, if you want them: `key` (default), `install HOST`, `hello`, `connect`, `install-cloudflared`. The host's copied block does not call it.

The account may join `plugdev` or `adbusers` when those groups already exist. Never `sudo`, `wheel`, `docker`, `disk`, `dialout`, or `input`. Tunelito does not install `adb` or write udev rules.

## 🔧 Configuration

State lives under the XDG state directory (`~/.local/share/tunelito` on Linux) unless `TUNELITO_STATE_DIR` is set. The window does not show these.

| Variable | Effect |
| --- | --- |
| `TUNELITO_STATE_DIR` | `state.json`, cloudflared, tunnel log. |
| `TUNELITO_ROOT` | A directory other than `/` gets files only. No system commands. |
| `TUNELITO_PRIV_BIN` | Path to `tunelito-priv`. |
| `TUNELITO_SSH_DIR` / `TUNELITO_SSH_CONFIG` | Overrides for `tunelito-connect`. |
| `TUNELITO_EMBED_DIR` | At build time, on Windows, directory with `cloudflared.exe` and `tunelito-priv.exe` to compile into the exe. |

Debian writes `/etc/ssh/sshd_config.d/00-tunelito.conf`: `Match User tunelito`, no password, no X11, no TCP forwarding, `MaxSessions` at least 10. Windows upserts a marked block in `C:\ProgramData\ssh\sshd_config` ahead of the other Match blocks and does not open port 22. The key is written to `C:\ProgramData\ssh\tunelito_authorized_keys` and to `C:\Users\tunelito\.ssh\authorized_keys`, because stock sshd names the profile file before any Match block. If the account is already an administrator, OpenSSH reads `administrators_authorized_keys` instead. New accounts are not added to that group.

`cloudflared` is pinned to **2026.10.2**. A bad checksum is discarded.

## 🏗️ Architecture

```
their computer                         your computer
tunelito.exe                           ssh + cloudflared
  one prompt, paste key                  ProxyCommand cloudflared access ssh
  tunelito-priv                          ssh the-name.trycloudflare.com
  cloudflared quick tunnel
       +------------ trycloudflare ------- ssh://127.0.0.1:22
```

| Crate | Role |
| --- | --- |
| `tunelito-core` | Keys, plans, on/off, logs, checksums. No service starts here. |
| `tunelito-connect` | Optional helper CLI. |
| `tunelito-priv` | Privileged helper. `--execute` only as root on `/`, or elevated on `C:\`. |
| `app` | The window. |

Sessions come from the OpenSSH log plus `ps` (`sshd: user@pts` or `@notty`, not `[priv]`). Windows reads the OpenSSH Operational log.

The portable exe is the primary Windows artifact. An NSIS installer is optional. Signing: [docs/SIGNING.md](docs/SIGNING.md).

## 🧪 Development

Linux CI runs fmt, clippy, and tests. It does not install OpenSSH or open a tunnel. The Windows job builds `tunelito.exe` and runs setup on the runner.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd app && npm ci && npm run build
```

UI, sample data only:

```bash
cd app && npm run dev
# http://127.0.0.1:1420/?preview=connected
# http://127.0.0.1:1420/?preview=connected&lang=es
```

Icons: `python3 scripts/make_icons.py`.

## 📊 Example Output

```text
$ ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -C helper
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... helper
```

After the button:

```text
maple-river-42.trycloudflare.com
```

```text
Host maple-river-42.trycloudflare.com
  HostName maple-river-42.trycloudflare.com
  User tunelito
  IdentityFile ~/.ssh/id_ed25519
  IdentitiesOnly yes
  StrictHostKeyChecking accept-new
  ProxyCommand cloudflared access ssh --hostname %h
```

A hello, when the optional CLI sends one, is stored with `"verification": "unverified"`. The address OpenSSH recorded stays `127.0.0.1`.

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [code of conduct](CODE_OF_CONDUCT.md).

Do not start the tunnel or sshd at boot, do not run the session as the person at the keyboard, and do not treat a hello as proof of identity.

## 📄 License

[MIT](LICENSE) © 2026 Esteban Carisimo.

## 🙏 Acknowledgments

- [Cloudflare](https://github.com/cloudflare/cloudflared) for quick tunnels.
- [Tauri](https://tauri.app/) for the window.
- OpenSSH.
- Anyone who refuses to turn this on for a stranger.
