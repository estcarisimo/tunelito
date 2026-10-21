# Tunelito

Tunelito lets you connect to the computer of someone you want to help — a friend, your mom, your brother, your cousin, your partner — and fix it or work on it.

They don't need to know how any of this works. The computer sits behind NAT or CGNAT, so port forwarding isn't an option.

They download one file, open it, and paste the key you sent (WhatsApp is fine). The window shows an address. You connect with SSH.

Claude Code over SSH, and USB devices, are one way to use that session.

**Only with someone you know.** If a stranger asks them to paste a key, the answer is no.

![Tunelito](images/tunelito-en.png)

## On their computer

1. Download `tunelito.exe`, drag the app out of the `.dmg`, or install the `.deb` / AppImage.
2. Open it. Accept the one administrator prompt.
3. Paste the key. Press the button. Copy or share the address.

## On yours

Install `cloudflared`. Send only a public key (`ssh-keygen -t ed25519`). Then [connect with SSH](connect.md).

It turns itself off. Nothing starts at boot.

More: [Install](install.md) · [Security](security.md) · [FAQ](faq.md)
