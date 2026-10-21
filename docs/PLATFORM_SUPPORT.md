# Platform support

| | Debian | macOS 13+ | Windows 10 1809+ / 11 |
| --- | --- | --- | --- |
| What they open | `.deb` or AppImage | drag the app out of the `.dmg` | `tunelito.exe` |
| Admin prompt | each time the switch changes (`pkexec`) | each time the switch changes (the macOS password prompt) | one UAC prompt, then none |
| Executed in CI | file layout and command text. The Linux job does not install packages. | file layout under a fake root and command text, from the Linux job. Nothing runs on a Mac. | `windows-latest` builds the exe. Setup is not run; use `scripts/windows-integration.ps1` on a throwaway VM. |
| Installer | `.deb` | `.dmg` | optional NSIS. The exe is the file to download. |

Windows on ARM has no pinned `cloudflared`. Signing secrets are optional; empty means unsigned. See [SIGNING.md](SIGNING.md).

## macOS

- The `tunelito` account is created hidden, as a standard user, with a random password that is not stored.
- Keys go to `/Users/tunelito/.ssh/authorized_keys`. The sshd settings go to `/etc/ssh/sshd_config.d/00-tunelito.conf`. That needs the `Include` line macOS 13 ships, and setup refuses without it.
- If Remote Login was off, Tunelito turns it on with `launchctl` and limits it to `tunelito` through `com.apple.access_ssh` (Remote Login's "Only these users"). Turning the switch off undoes both. If Remote Login was already on, Tunelito leaves it on and only adds or removes `tunelito` from that group.
- Remote Login survives a reboot. If the Mac restarts while the switch is on, Remote Login stays on, still limited to `tunelito`, until the switch is next turned off.
- Not yet: the `.dmg` does not carry `tunelito-priv`. Set `TUNELITO_PRIV_BIN` or place it next to the app binary.
