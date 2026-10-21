# Contributing to Tunelito

Thanks for looking at the project. Tunelito is a small remote-access tool with a sharp edge, so the review bar on security-sensitive changes is higher than the bar on copy or tests.

## What to know first

- The host must not start at boot, and neither should sshd or cloudflared when Tunelito is what installed them.
- The remote account is `tunelito`. Do not aim the tunnel at the desktop user's account or at root.
- A hello is unverified. Do not let a client set `verification` to anything else.
- Windows and macOS service execution is not run in CI. If you change those command lists, keep the `UNTESTED ON CI` comments and extend the unit tests that lock the arguments.

## Setup

Rust stable (1.83+), Node 22, and the Tauri Linux libraries listed in the README.

```bash
cargo test --workspace
cd app && npm ci && npm run build
```

`cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` should be clean before you open a pull request.

## Tests that must stay offline

CI must not download `cloudflared`, must not start a quick tunnel, and must not install OpenSSH or create the `tunelito` user. File-layout tests use a temporary directory. Command arguments are checked as data.

The window can be screenshotted from `app/` with `npm run dev` and `?preview=connected`. That page is sample data.

## Pull requests

- Describe what changed and how you tested it.
- Say so if a Windows or macOS path is still untested.
- Do not add a tag or a GitHub release as part of the change. The release workflow runs only when a version tag is pushed, and it opens a draft.

## Reporting security issues

Write to esteban.carisimo@northwestern.edu. See [SECURITY.md](SECURITY.md). Please do not file a public issue for an unfixed vulnerability.

By participating you agree to the [code of conduct](CODE_OF_CONDUCT.md).
