# Threat model

Tunelito is how you reach the computer of someone you want to help — a friend, your mom, your brother, your cousin, your partner — when that computer is behind NAT or CGNAT and they are not going to set up port forwarding. The person at the keyboard decides a helper may connect. The helper is someone they already know. Claude Code and USB devices are one use of the shell, not a separate trust decision.

## Assets

- The `tunelito` account, its home directory, and Claude Code installed under `~/.claude/remote/` there.
- Devices that account can open (USB, adb) when it is in `plugdev` or `adbusers`.
- The accepted public keys and the private key on the helper's machine (`~/.ssh/tunelito_ed25519`).
- The quick-tunnel hostname, which is unguessable in practice but is not a credential.

## Actors

| Actor | What they can try |
| --- | --- |
| The person at the host | Turn the switch on for the wrong person, or leave it on longer than they meant. |
| A trusted helper | Use the shell as `tunelito` for the whole time the switch is on. That includes anything that account can do. |
| A stranger running a tech-support scam | Pressure the person into pasting the stranger's public key and turning the switch on. |
| Someone who sees the hostname | Connect to the tunnel. They still need a private key matching an accepted public key. |
| Cloudflare | Terminate the quick tunnel. They see that an SSH connection is being proxied, not the decrypted session, unless the helper's client is compromised. |
| A malicious `cloudflared` build | Replace the binary. Tunelito checks the pinned SHA256 before saving a download. |

## Scam shape, and what we do about it

The dangerous path is social, not cryptographic. A caller says there is a problem, asks the person to install an app, paste a key, and turn a switch on. Tunelito cannot detect a lie. It can make the decision hard to miss:

- The warning is on the main screen, in the language the app is showing, and says the switch gives control of the `tunelito` account.
- The switch asks for confirmation.
- It turns off by itself. There is no "until I turn it off" option. The longest timer is 24 hours.
- It does not start at boot, so a reboot is an off switch.
- Keys are shown with a fingerprint the two people can compare out of band.

If you are writing docs, UI copy, or a default, do not soften that warning.

## What the tunnel hides

`cloudflared` on the host dials out and carries SSH to `localhost:22`. sshd's peer address is `127.0.0.1` or `::1`. Tunelito does not invent a public IP from that. The helper CLI may send a hello with the address it looked up (Cloudflare trace, a RIPE or Team Cymru ASN, reverse DNS). The host stores `verification` as `unverified` even if the JSON claimed otherwise, and the window uses the label "reported by the client (unverified)".

A helper, or anyone who can run the hello command, can put any string in those fields.

## What we refuse to do

- Run the remote session as the desktop user or as an administrator.
- Add `tunelito` to `sudo`, `wheel`, `docker`, `disk`, `dialout`, or `input`.
- Enable password authentication for `tunelito`.
- Allow TCP or X11 forwarding in the managed `Match` block. A helper who needs a forwarded port has to ask for a different setup. That is a deliberate limit.
- Install cloudflared as a system service, or enable sshd at boot when Tunelito is what installed the package. On Debian the package's own enablement is undone with `systemctl disable` after install. If sshd was already enabled before Tunelito, that boot policy is left alone.
- Trust a `cloudflared` download that does not match the pinned checksum.
- Treat one missed ping, a hostname, or a hello as proof of who connected.

## Residual risk

A trusted helper is trusted. For the time the switch is on they can change files the `tunelito` account can write, and they can use devices that account can open. Tunelito does not sandbox the shell beyond the account and the sshd `Match` block.

`StrictHostKeyChecking accept-new` trusts the host key the first time a name is seen. Quick-tunnel names are new every time the switch is turned on, so this is trust-on-first-use for each name. It is there so Claude Desktop is not stuck on a prompt. It is not `StrictHostKeyChecking no`.

The hello directory on Linux is mode `1777` so the desktop user can read hellos without being in the `tunelito` group (group membership would need a new login). The files are world-readable. They contain only what the client chose to send.

The administrator prompt (`pkexec` or `sudo` on Linux) is required to create the account and to start sshd. A person who approves a prompt they do not understand is the same risk as any other privileged installer.
