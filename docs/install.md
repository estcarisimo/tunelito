# Install on this computer

This page is for the person sitting at the machine that will be reached. The helper's steps are in [Connect with SSH](connect.md).

Tunelito does not start at login. Turning the switch off, or closing the window, drops the tunnel. Opening the app later does not turn it back on.

!!! warning

    Only do this with someone you know. The switch gives that person a shell as the account `tunelito`, and access to the devices that account can open.

## Debian and other Linux

The path that CI tests, and the one the app actually runs, is Linux.

1. Install the `.deb` from a release when one is published, or build the window from a checkout (`cargo tauri dev` inside `app/`, with the Tauri system libraries). The `.deb` depends on `openssh-client`. The privileged helper `tunelito-priv` is built beside it and is not yet packed inside the installer. Put it next to the Tunelito binary, in `/usr/lib/tunelito/tunelito-priv`, or point `TUNELITO_PRIV_BIN` at it.
2. Open Tunelito. Paste the helper's public key and press **Add key**.
3. Press the switch and confirm. Approve the administrator prompt (`pkexec` when a desktop session is set, otherwise `sudo`).

What that prompt is allowed to do:

- `apt-get install -y openssh-server` if the server is missing, then `systemctl disable ssh` so the package does not start at boot.
- `useradd` an account named `tunelito`, with its own home and a login shell. It is not added to `sudo`, `wheel`, `docker`, or `disk`.
- If the groups `plugdev` or `adbusers` already exist, the account may be added to those, and only those, so a USB device can be opened. Tunelito does not install `adb` and does not write udev rules. A TV can still be permission-denied.
- Write `/etc/ssh/sshd_config.d/00-tunelito.conf` and `/home/tunelito/.ssh/authorized_keys`.
- Start `ssh` (or `sshd`) for this session. On the way back off, Tunelito stops that service only if Tunelito was the one that started it. An sshd that was already enabled at boot is left alone.

The drop-in allows only a `Match User tunelito` block (public key, no password, no X11 forwarding, no TCP forwarding, `MaxSessions 10`) plus a global `MaxSessions 10` or an SFTP line when the running config is missing them. `ForceCommand` is not in the allowlist.

CI checks those command arguments and the files written under a fake root. It does not run `apt-get`, `useradd`, or `systemctl` on the runner.

## Windows 10 1809+ and Windows 11

The release workflow can build an `.msi` and an NSIS `.exe`. Signing secrets are optional. An empty secret produces an unsigned installer.

**Turning the switch on does not yet run the Windows setup.** The command list is unit-tested as text, and the window returns `needs_admin` plus a coverage note instead of executing it. That list, when someone runs it on a real PC, is:

- `Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0`
- `Set-Service -Name sshd -StartupType Manual` so the service does not start at boot
- `New-LocalUser` for `tunelito`, not a member of Administrators
- start `sshd` for the session

Until that path is executed on Windows, treat the Windows host as an installer shell, not a working tunnel. Session lines would come from `wevtutil` and the OpenSSH Operational log. That reader is compiled in and is not run in CI.

Windows on ARM has no pinned `cloudflared` build in Tunelito's table.

## macOS

The release workflow can build a `.dmg`. Minimum system version in the bundle config is 10.15. Signing secrets are optional.

**Turning the switch on does not yet run the macOS setup.** The command list is unit-tested as text, and the window returns `needs_admin` plus a coverage note. Remote Login (`systemsetup -setremotelogin on`) is boot-persistent. The plan turns it back off only if Tunelito was the one that enabled it. The account would be created with `sysadminctl`, and `sshd` kicked with `launchctl`. None of that runs from the app today.

Session lines would come from `log show` and `ps`. That reader is compiled in and is not run in CI.

## After it is on

The window shows a `something.trycloudflare.com` name and a config block. Copy the block and send it to the helper. Leave the switch on only while you want them connected. The timer defaults to 2 hours and can be set from 15 minutes to 24 hours. There is no "until I turn it off".
