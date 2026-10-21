# FAQ

## Do I need tunelito-connect?

No. Install `cloudflared`, send a public key from `ssh-keygen`, paste the block the host copies into `~/.ssh/config`, and run `ssh the-name.trycloudflare.com`.

`tunelito-connect` is optional. It prints a public key from `~/.ssh/tunelito_ed25519`, and it can send the unverified hello.

## The connections row says "no client info". Is the session broken?

No. That line means the helper did not send a hello. The row still has the green or gray dot, the user, the key fingerprint, and the start and end times. Those come from the SSH server's own log. A plain `ssh` session looks like this.

## Why is the address 127.0.0.1?

`cloudflared` on the host connects to `localhost:22`. The SSH server never sees the helper's real address. Tunelito does not invent one. A hello can carry an address the client looked up, and the window labels it unverified.

## Where does Claude Code install?

Under `~/.claude/remote/` of the `tunelito` account, for example `/home/tunelito/.claude/remote/` on Linux. It does not install into the personal home of the person at the keyboard. Desktop's installer needs SFTP and several channels, which is why the drop-in sets `MaxSessions` to at least 10.

## Why not SSH in as me?

The remote account is unprivileged on purpose. It is not in `sudo` or `wheel`. A helper you trust still has a shell as `tunelito` for as long as the switch is on. That is enough to change files that account can write and to open devices that account can open.

## Will a USB TV just work?

Maybe not. Tunelito may add `tunelito` to `plugdev` or `adbusers` if those groups already exist. It does not install `adb` and it does not write udev rules. Permission denied on the device is still possible.

## Does this start when I reboot?

No. The app does not autostart. On a fresh Debian install, Tunelito disables the ssh service the package enabled. It does not install `cloudflared` as a service. Windows plans set the service startup to Manual. Closing the window turns access off.

## Can someone connect if they guess the hostname?

They can reach the tunnel. They cannot log in unless they have a private key matching a public key you accepted. The hostname is not a password. The key is.

## The name changed.

It does, every time the switch is turned on. Copy the new block. The old name stops working when the tunnel process exits.

## Windows Claude Desktop ignores the config.

That matches what we know of Desktop's built-in SSH client: it does not read OpenSSH's config, so the `ProxyCommand` never runs. Switching Desktop to system OpenSSH is required and the menu path **needs verification**. `ssh` in a terminal still uses the block.

## The Spanish in the app says "encendelo".

The window and these pages use Rioplatense voseo, the same Spanish as the person who wrote them.
