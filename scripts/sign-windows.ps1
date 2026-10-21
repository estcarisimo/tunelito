# No-op unless a signing secret is present. Empty secrets must not fail the build.
# See docs/SIGNING.md. This does not claim the binary is signed.
param([Parameter(Mandatory = $true)][string]$Exe)

$azure = $env:AZURE_TENANT_ID
$signpath = $env:SIGNPATH_API_TOKEN
if (-not $azure -and -not $signpath) {
    Write-Host "No Azure Trusted Signing or SignPath secret. Leaving $Exe unsigned."
    exit 0
}
Write-Host "A signing secret is set. This workflow does not call signtool unless the client is installed."
Write-Host "Azure Trusted Signing and SignPath OSS are documented in docs/SIGNING.md."
if ($azure -and (Get-Command signtool -ErrorAction SilentlyContinue)) {
    Write-Host "signtool is on PATH. Trusted Signing still needs the dlib and a certificate profile; skipping rather than guessing flags."
}
exit 0
