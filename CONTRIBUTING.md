# Contributing to Tunelito

Thanks for looking at the project. Tunelito is a small remote-access tool with a sharp edge, so the review bar on security-sensitive changes is higher than the bar on copy or tests.

## What to know first

- The host must not start at boot, and neither should sshd or cloudflared when Tunelito is what installed them.
- The remote account is `tunelito`. Do not aim the tunnel at the desktop user's account or at root.
- A hello is unverified. Do not let a client set `verification` to anything else.
- Windows setup is exercised on `windows-latest`. macOS service execution is not. If you change a command list, extend the unit test that locks the arguments.
- Code signing secrets are optional. See [docs/SIGNING.md](docs/SIGNING.md). Empty secrets leave the exe unsigned.

## Setup

Rust stable (1.83+), Node 22, and the Tauri Linux libraries listed in the README.

```bash
cargo test --workspace
cd app && npm ci && npm run build
```

`cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` should be clean before you open a pull request.

## Tests that must stay offline

The Linux job must not download `cloudflared`, start a quick tunnel, install OpenSSH, or create the `tunelito` user. File-layout tests use a temporary directory. The Windows job does the real setup on the runner and deletes the account afterwards.

The window can be screenshotted from `app/` with `npm run dev` and `?preview=connected`. That page is sample data.

## Pull requests

- Describe what changed and how you tested it.
- Say so if a Windows or macOS path is still untested.
- Do not add a tag or a GitHub release as part of the change. The release workflow runs only when a version tag is pushed, and it opens a draft.

## Reporting security issues

Write to esteban.carisimo@northwestern.edu. See [SECURITY.md](SECURITY.md). Please do not file a public issue for an unfixed vulnerability.

By participating you agree to the [code of conduct](CODE_OF_CONDUCT.md).
