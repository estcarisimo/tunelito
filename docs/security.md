# Security

You are letting someone you know open a shell as `tunelito` on that computer.

| Who | What they can do |
| --- | --- |
| The person at the computer | Turn it on for the wrong person, or leave it on. |
| Someone they trust | Use that shell until it turns off. |
| A stranger | Ask them to paste a key. Tunelito cannot tell the caller is lying. |
| Someone who only has the address | Nothing, without the private key. |
| Cloudflare | See that an SSH session is proxied, and end the tunnel. |

The dangerous path is the phone call, not the cryptography.

- One line on screen. The button asks for a confirm.
- It turns off by itself. 24 hours is the maximum. A reboot is off.
- The address is not the secret. The key is.
- sshd sees `127.0.0.1`. A hello is labeled unverified. No hello still lists the session.

The account is not an administrator. Password login for `tunelito` stays off. The managed block does not allow TCP or X11 forwarding.

To report a vulnerability, write to esteban.carisimo@northwestern.edu. Do not open a public issue for an unfixed one.
