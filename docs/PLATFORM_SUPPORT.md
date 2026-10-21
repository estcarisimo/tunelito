# Platform support

| | Linux (Debian and relatives) | macOS | Windows 10 1809+ / 11 |
| --- | --- | --- | --- |
| Helper CLI `tunelito-connect` | Tested in CI | Same code, not executed in CI | Same code, not executed in CI. `NUL` is used where Unix uses `/dev/null`. |
| Parse keys, logs, hellos, sshd plans | Tested in CI | Command text is unit-tested on Linux | Command text is unit-tested on Linux |
| Write drop-in and `authorized_keys` under a fake root | Tested in CI | Not run | Not run |
| Install and start OpenSSH, create `tunelito` | Implemented in `tunelito-priv`. CI does not run `apt-get`, `useradd`, or `systemctl`. | Planned (`systemsetup`, `sysadminctl`, `launchctl`). **The app refuses to execute this.** | Planned (`Add-WindowsCapability OpenSSH.Server~~~~0.0.1.0`, `Set-Service` Manual, `New-LocalUser`). **The app refuses to execute this.** |
| cloudflared quick tunnel | Implemented. CI does not open one. | Asset is a `.tgz`, checksum pinned. Not run in CI. | Asset checksum pinned. Not run in CI. Windows on ARM is not in the asset table. |
| Session list | `journalctl` or `/var/log/auth.log`, plus `ps` | `log show` and `ps`, compiled in, not run in CI | `wevtutil` query of the OpenSSH Operational log, compiled in, not run in CI |
| Installer | `.deb` on a version tag | `.dmg` on a version tag | `.msi` and NSIS `.exe` on a version tag |

Lines marked **UNTESTED ON CI** in the Rust sources are the ones this table calls out. A green CI run does not mean a Mac or a Windows PC was installed, logged into, or tunneled.

## Linux details CI does not exercise

`tunelito-priv apply --execute` only runs when euid is 0 and `--root /`. The sequence is: install `openssh-server` if needed, `systemctl disable ssh` so the package does not start at boot, `useradd` the `tunelito` account, `usermod -aG` only `plugdev` and `adbusers` when `getent` shows the group, write files, `chown` the account's `.ssh` to `tunelito`, then start or reload sshd. The app looks for `pkexec` when `DISPLAY` or `WAYLAND_DISPLAY` is set, otherwise `sudo`.

## Why macOS and Windows stop at the plan

Remote Login on macOS is a boot-persistent setting (`systemsetup -setremotelogin`). The plan turns it back off only if Tunelito was the one that enabled it. Running that, or `Add-WindowsCapability`, from an agent that cannot see the machine would be a guess. The window returns `needs_admin` plus the coverage note until those paths are executed on the real OS.

The release workflow still builds the `.dmg`, `.msi`, and `.exe` shells. Signing secrets are optional placeholders. An empty secret skips signing. The workflow does not run until a `vX.Y.Z` tag is pushed, and the release it opens is a draft.
