# Security

Tunelito is a consent switch for a shell. The person at the keyboard decides that a helper may connect. The helper is assumed to be someone they already know.

## Who can do what

| Who | What they can try |
| --- | --- |
| The person at the host | Turn the switch on for the wrong person, or leave it on longer than they meant. |
| A trusted helper | Use the shell as `tunelito` for the whole time the switch is on. That includes anything that account can do. |
| A stranger running a tech-support scam | Pressure the person into pasting the stranger's public key and turning the switch on. |
| Someone who only sees the hostname | Connect to the tunnel. They still need a private key that matches an accepted public key. |
| Cloudflare | Terminate the quick tunnel. They see that an SSH connection is being proxied. |
| A replaced `cloudflared` binary | Run instead of the real one. Tunelito checks the pinned SHA256 before it saves a download on the host. |

The dangerous path is social, not cryptographic. A caller says there is a problem, asks the person to install an app, paste a key, and turn a switch on. Tunelito cannot tell that the caller is lying. It can make the decision hard to miss:

- The warning stays on the main screen, in the language the window is showing.
- The switch asks for confirmation.
- It turns off by itself. The longest timer is 24 hours. There is no indefinite option.
- It does not start at boot, so a reboot is an off switch.
- Each accepted key is shown with a fingerprint the two people can compare out of band.

## What the tunnel hides

`cloudflared` on the host dials out and carries SSH to `localhost:22`. The SSH server's peer address is `127.0.0.1` or `::1`. Tunelito does not invent a public IP from that.

A plain `ssh` session sends no hello. The connections row still shows the key fingerprint, the user, and the start and end times, and it says **no client info**.

If the helper runs `tunelito-connect hello`, the host stores a public IP, an ASN, a reverse DNS name, and an OS string. The `verification` field is forced to `unverified` even if the JSON claimed otherwise. The window labels the block **reported by the client (unverified)**. Anyone who can run that command can put any string in those fields.

## What Tunelito refuses to do

- Run the remote session as the desktop user or as an administrator.
- Add `tunelito` to `sudo`, `wheel`, `docker`, `disk`, `dialout`, or `input`.
- Enable password authentication for `tunelito`.
- Allow TCP or X11 forwarding in the managed `Match` block. A helper who needs a forwarded port has to ask for a different setup.
- Install `cloudflared` as a system service, or enable sshd at boot when Tunelito is what installed the package. On Debian the package's own enablement is undone with `systemctl disable` after install. If sshd was already enabled before Tunelito, that boot policy is left alone.
- Trust a `cloudflared` download on the host that does not match the pinned checksum.
- Treat a hostname, or a hello, as proof of who connected.

## What remains

A trusted helper is trusted. For the time the switch is on they can change files the `tunelito` account can write, and they can use devices that account can open. Tunelito does not sandbox the shell beyond the account and the sshd `Match` block.

The hello directory on Linux is mode `1777` so the desktop user can read hellos without being in the `tunelito` group. The files are world-readable. They contain only what the client chose to send.

The administrator prompt is the same risk as any other privileged installer. Read it.

To report a vulnerability, write to esteban.carisimo@northwestern.edu. Please do not open a public issue for an unfixed one. The repository copy of this discussion is `docs/THREAT_MODEL.md`, and the policy is `SECURITY.md`.
