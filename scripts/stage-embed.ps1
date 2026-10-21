# Download the pinned cloudflared.exe and copy tunelito-priv.exe next to it.
# TUNELITO_EMBED_DIR / ./embed is what the portable exe compiles in.
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$dest = if ($env:TUNELITO_EMBED_DIR) { $env:TUNELITO_EMBED_DIR } else { Join-Path $root 'embed' }
New-Item -ItemType Directory -Force -Path $dest | Out-Null

# Keep this hash in sync with crates/tunelito-core/src/cloudflared.rs (Windows amd64).
$hash = '12c7c887f8aaab779aef1dfa9b60673530a5adb88ecc55dd6df8f56e0b70f199'
$url = 'https://github.com/cloudflare/cloudflared/releases/download/2026.10.2/cloudflared-windows-amd64.exe'
$cloudflared = Join-Path $dest 'cloudflared.exe'
Write-Host "downloading $url"
Invoke-WebRequest -Uri $url -OutFile $cloudflared
$got = (Get-FileHash -Algorithm SHA256 -Path $cloudflared).Hash.ToLower()
if ($got -ne $hash) {
    throw "cloudflared checksum mismatch: $got"
}
$priv = Join-Path $root 'target\release\tunelito-priv.exe'
if (-not (Test-Path $priv)) {
    throw "missing $priv — build tunelito-priv --release first"
}
Copy-Item $priv (Join-Path $dest 'tunelito-priv.exe') -Force
Write-Host "embed dir: $dest"
