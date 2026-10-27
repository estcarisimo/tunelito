# Changelog

## Unreleased

- Host ed25519 keys are generated with `rand_core` 0.10.
- The host's copyable SSH block is plain OpenSSH (`User tunelito`, `ProxyCommand cloudflared access ssh --hostname %h`). `tunelito-connect` is optional, for a key and the unverified hello.
- A session with no hello still lists the fingerprint and the times, and says no client info.
- Docs site (MkDocs Material, English and Spanish) for install, connecting, the threat model, and an FAQ.

## 0.1.0

First public cut of Tunelito.

- Host window in English and Spanish, with an on/off switch, accepted keys, a timer, and a connections list.
- Linux path that prepares OpenSSH and the `tunelito` account through `tunelito-priv`.
- Helper CLI `tunelito-connect`.
- Windows opens from one portable `tunelito.exe`: one administrator prompt, then paste a key.
- macOS service commands are planned and unit-tested, and are not executed by the app yet.
