# Security policy

Tunelito gives a trusted person a shell on this computer. Treat a request to
turn it on from someone you do not know as a tech-support scam, and say no.

## Reporting a vulnerability

Email [esteban.carisimo@northwestern.edu](mailto:esteban.carisimo@northwestern.edu)
with the subject "Tunelito security". Please include a description, the
version, and a way to reproduce the issue. Do not open a public issue for an
unfixed vulnerability.

## What this project promises

- The host app does not start at login or boot, and it does not install
  cloudflared as a service.
- Remote login uses a dedicated unprivileged account named `tunelito`. The
  account is not added to sudo, wheel, docker, or disk groups.
- Keys live in that account's `authorized_keys`. Password login for that
  account is turned off.
- `cloudflared` is downloaded only when the pinned checksum matches.
- Client-reported IP, ASN, reverse DNS, and OS are labeled unverified. The
  SSH server itself only sees the tunnel's loopback address.
- Turning the switch off removes the managed keys from `authorized_keys` and
  kills the tunnel.

The longer discussion of who can harm whom is in
[docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).
