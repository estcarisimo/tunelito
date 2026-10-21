# Connect with SSH

You need `cloudflared` on your `PATH`, and a public key they already pasted.

```bash
ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -C helper
```

Send the `.pub` line only.

They press Copy or Share. Open **Details** if you want the SSH block, or write:

```text
Host the-name.trycloudflare.com
  HostName the-name.trycloudflare.com
  User tunelito
  IdentityFile ~/.ssh/id_ed25519
  IdentitiesOnly yes
  StrictHostKeyChecking accept-new
  ProxyCommand cloudflared access ssh --hostname %h
```

```bash
ssh the-name.trycloudflare.com
```

The name changes every time they turn it on. `StrictHostKeyChecking accept-new` trusts the host key the first time that name is seen.

## Claude Desktop

macOS and Linux Desktop read `~/.ssh/config`. Add that hostname and connect as `tunelito`.

Windows Desktop's built-in SSH ignores that file. Switching it to system OpenSSH **needs a check on your build**. A terminal `ssh` works. See the repo note `docs/CLAUDE_DESKTOP.md`.

## Optional

`tunelito-connect` can print a key and send an unverified hello. You do not need it. A session with no hello still shows up, labeled **no client info**.
