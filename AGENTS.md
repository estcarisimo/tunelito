# Notes for agents working on Tunelito

- Do not open a real Cloudflare quick tunnel from CI or from a coding agent.
  The hostname is a public endpoint.
- Do not enable sshd, cloudflared, or the app at boot. `systemctl enable`,
  `cloudflared service install`, and an Automatic Windows service are rejected
  by `CommandStep::is_forbidden_autostart`.
- The remote account is `tunelito`, never root and never the person sitting at
  the computer. Claude Code then lands in `/home/tunelito/.claude/remote/`.
- Windows and macOS service commands are planned and unit-tested as data.
  CI does not execute them. Keep the `UNTESTED ON CI` comments when you touch
  those paths.
- Linux CI checks parsers, the state machine, the helper CLI, and file writes
  under a fake root. It does not `apt-get`, `useradd`, or start sshd.
- A hello is always stored as `verification: unverified`. Do not let a client
  set that field.
- Checksums for cloudflared `2026.10.2` live in `crates/tunelito-core`. Update
  the pin and the tests together.
