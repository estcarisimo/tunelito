## What changed

-

## How it was tested

- [ ] `cargo test --workspace` on Linux
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `npm run build` in `app/`
- [ ] Windows or macOS paths touched here were not executed in CI, and the description says so

## Security

- [ ] This does not start Tunelito, sshd, or cloudflared at boot
- [ ] This does not treat a client hello as verified
