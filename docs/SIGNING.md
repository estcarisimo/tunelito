# Signing the Windows exe

An unsigned `tunelito.exe` shows a SmartScreen warning. Reputation comes from a certificate users already trust, then from people running that signed file. This repo does not ship a certificate.

Two options, both optional. The build stays green when the secrets are empty.

## Azure Trusted Signing

[Trusted Signing](https://learn.microsoft.com/azure/trusted-signing/) (the service that replaced Azure Code Signing) signs with a short-lived Microsoft-backed cert.

Secrets, all empty by default:

- `AZURE_TENANT_ID`
- `AZURE_CLIENT_ID`
- `AZURE_CLIENT_SECRET`
- `AZURE_CODESIGN_ACCOUNT`
- `AZURE_CODESIGN_PROFILE`

`scripts/sign-windows.ps1` exits 0 when those are unset. It does not invent `signtool` flags.

## SignPath OSS

[SignPath Foundation](https://signpath.org/) signs open-source Windows binaries after a public review.

Secrets, all empty by default:

- `SIGNPATH_API_TOKEN`
- `SIGNPATH_ORG`
- `SIGNPATH_PROJECT`
- `SIGNPATH_POLICY`

Same script: unset means the exe is uploaded unsigned.

The tag workflow (`release.yml`) still has `WINDOWS_CERTIFICATE` and `WINDOWS_CERTIFICATE_PASSWORD` for a traditional PFX. Empty means unsigned. None of these workflows create a tag or a GitHub Release on their own.
