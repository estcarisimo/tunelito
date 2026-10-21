# Runs on windows-latest as Administrator. Not a release.
# Proves OpenSSH, the tunelito account, and a localhost key login.
# -Tunnel tries one quick tunnel. A tunnel failure is reported and does not fail
# the run when TUNELITO_TUNNEL_BEST_EFFORT=1. Cleanup always runs.
param([switch]$Tunnel)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$priv = Join-Path $root 'target\release\tunelito-priv.exe'
$cloudflared = Join-Path $root 'embed\cloudflared.exe'
$work = Join-Path $env:TEMP "tunelito-ci-$PID"
New-Item -ItemType Directory -Force -Path $work | Out-Null
$tunnelProc = $null
$tunnelNote = 'skipped'

function Invoke-Cleanup {
    if ($script:tunnelProc -and -not $script:tunnelProc.HasExited) {
        Stop-Process -Id $script:tunnelProc.Id -Force -ErrorAction SilentlyContinue
    }
    Get-Process cloudflared -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    if (Test-Path $priv) {
        & $priv uninstall --execute
    }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

try {
    if (-not (Test-Path $priv)) { throw "missing $priv" }
    $key = Join-Path $work 'id_ed25519'
    & ssh-keygen -t ed25519 -f $key -N "" -C tunelito-ci | Out-Null
    $pub = (Get-Content "$key.pub" -Raw).Trim()
    $request = @{
        authorized_key_lines = @($pub)
        write_keys = $true
        prepare = $true
        start_sshd = $true
        reload_sshd = $true
        stop_sshd = $false
        windows_account_is_admin = $false
        windows_set_global_max_sessions = $true
        windows_add_sftp = $true
    } | ConvertTo-Json -Compress
    $requestPath = Join-Path $work 'request.json'
    $resultPath = Join-Path $work 'result.json'
    [System.IO.File]::WriteAllText($requestPath, $request)
    & $priv apply --request $requestPath --root C:\ --execute --windows --result $resultPath
    if ($LASTEXITCODE -ne 0) {
        $detail = if (Test-Path $resultPath) { Get-Content $resultPath -Raw } else { 'no result file' }
        throw "tunelito-priv failed ($LASTEXITCODE): $detail"
    }
    if (-not (Get-Command ssh -ErrorAction SilentlyContinue)) {
        throw 'OpenSSH client (ssh) is not on PATH'
    }
    & ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o ConnectTimeout=20 -i $key -p 22 tunelito@127.0.0.1 echo tunelito-ok
    if ($LASTEXITCODE -ne 0) { throw "localhost ssh failed: $LASTEXITCODE" }
    Write-Host 'verified: OpenSSH capability, tunelito user, sshd, localhost pubkey login'

    if ($Tunnel) {
        if (-not (Test-Path $cloudflared)) { throw "missing $cloudflared" }
        $logOut = Join-Path $work 'tunnel.out'
        $logErr = Join-Path $work 'tunnel.err'
        $script:tunnelProc = Start-Process -FilePath $cloudflared -ArgumentList @('tunnel','--no-autoupdate','--url','ssh://localhost:22') -RedirectStandardOutput $logOut -RedirectStandardError $logErr -PassThru -WindowStyle Hidden
        $hostname = $null
        foreach ($i in 1..30) {
            Start-Sleep -Seconds 2
            $blob = ''
            if (Test-Path $logOut) { $blob += Get-Content $logOut -Raw -ErrorAction SilentlyContinue }
            if (Test-Path $logErr) { $blob += Get-Content $logErr -Raw -ErrorAction SilentlyContinue }
            $match = [regex]::Match($blob, '([a-z0-9-]+\.trycloudflare\.com)')
            if ($match.Success -and $match.Groups[1].Value -ne 'trycloudflare.com') {
                $hostname = $match.Groups[1].Value
                break
            }
        }
        if (-not $hostname) {
            throw "quick tunnel did not print a hostname"
        }
        Write-Host "tunnel hostname: $hostname"
        $env:PATH = "$(Split-Path $cloudflared);$env:PATH"
        & ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o ConnectTimeout=25 -o "ProxyCommand=cloudflared access ssh --hostname %h" -i $key "tunelito@$hostname" echo tunelito-tunnel-ok
        if ($LASTEXITCODE -ne 0) { throw "ssh over the quick tunnel failed: $LASTEXITCODE" }
        $tunnelNote = "ok $hostname"
        Write-Host 'verified: ssh over a cloudflared quick tunnel'
    }
}
catch {
    if ($Tunnel -and $env:TUNELITO_TUNNEL_BEST_EFFORT -eq '1' -and $_.Exception.Message -match 'tunnel|trycloudflare|ProxyCommand') {
        $tunnelNote = $_.Exception.Message
        Write-Warning "TUNNEL NOT VERIFIED: $tunnelNote"
    }
    else {
        throw
    }
}
finally {
    if ($env:GITHUB_STEP_SUMMARY) {
        Add-Content -Path $env:GITHUB_STEP_SUMMARY -Value "- localhost SSH: checked in this step`n- quick tunnel: $tunnelNote"
    }
    Invoke-Cleanup
}
