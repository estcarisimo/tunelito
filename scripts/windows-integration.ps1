# Run by hand, as Administrator, on a disposable Windows VM. Not a release.
# CI does not run this: it creates the tunelito account, starts sshd, and with
# -Tunnel opens a public quick tunnel. See AGENTS.md.
# Proves OpenSSH, the tunelito account, and a localhost key login.
# -Tunnel tries one quick tunnel. A tunnel failure is reported and does not fail
# the run when TUNELITO_TUNNEL_BEST_EFFORT=1. Cleanup always runs.
param([switch]$Tunnel)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$priv = Join-Path $root 'target\release\tunelito-priv.exe'
$cloudflared = Join-Path $root 'embed\cloudflared.exe'
$work = Join-Path $env:TEMP "tunelito-ci-$PID"
New-Item -ItemType Directory -Force -Path $work | Out-Null
$tunnelProc = $null
$tunnelNote = 'skipped'

function Show-Setup {
    Write-Host '--- setup ---'
    $user = Get-LocalUser -Name 'tunelito' -ErrorAction SilentlyContinue
    if ($user) { Write-Host "user enabled=$($user.Enabled) sid=$($user.SID)" } else { Write-Host 'user missing' }
    Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList' -ErrorAction SilentlyContinue | ForEach-Object {
        $image = (Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue).ProfileImagePath
        if ($image -like '*tunelito*') { Write-Host "profile $image" }
    }
    foreach ($path in @(
            'C:\Users\tunelito',
            'C:\Users\tunelito\.ssh',
            'C:\Users\tunelito\.ssh\authorized_keys',
            'C:\ProgramData\ssh\tunelito_authorized_keys',
            'C:\ProgramData\ssh\administrators_authorized_keys'
        )) {
        if (Test-Path -LiteralPath $path) {
            Write-Host "present $path"
            $owner = (Get-Acl -LiteralPath $path).Owner
            Write-Host "owner $owner"
            if (-not (Get-Item -LiteralPath $path).PSIsContainer) {
                Get-Content -LiteralPath $path | ForEach-Object { Write-Host $_ }
            }
            icacls $path
        }
        else {
            Write-Host "absent $path"
        }
    }
    $sshd = Join-Path $env:SystemRoot 'System32\OpenSSH\sshd.exe'
    if (Test-Path -LiteralPath $sshd) {
        Write-Host '--- sshd -T tunelito ---'
        & $sshd -T -C 'user=tunelito,host=127.0.0.1,addr=127.0.0.1' 2>&1 |
            Select-String -Pattern 'authorizedkeysfile|pubkeyauthentication|passwordauthentication|authenticationmethods|strictmodes' |
            ForEach-Object { Write-Host $_.Line }
    }
    $cfg = 'C:\ProgramData\ssh\sshd_config'
    if (Test-Path $cfg) {
        Select-String -Path $cfg -Pattern 'tunelito|AuthorizedKeysFile|AuthenticationMethods' |
            ForEach-Object { Write-Host $_.Line.Trim() }
    }
}

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
    # tunelito-priv is a windowed executable. The call operator does not wait
    # for those, so a later check can run before setup has written anything.
    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo.FileName = $priv
    $proc.StartInfo.Arguments = "apply --request `"$requestPath`" --root C:\ --execute --windows --result `"$resultPath`""
    $proc.StartInfo.UseShellExecute = $false
    $proc.StartInfo.RedirectStandardOutput = $true
    $proc.StartInfo.RedirectStandardError = $true
    $proc.StartInfo.CreateNoWindow = $true
    [void]$proc.Start()
    $outTask = $proc.StandardOutput.ReadToEndAsync()
    $errTask = $proc.StandardError.ReadToEndAsync()
    $proc.WaitForExit()
    $privOut = $outTask.Result
    $privErr = $errTask.Result
    if ($privOut) { Write-Host $privOut }
    if ($privErr) { Write-Host $privErr }
    $resultText = if (Test-Path $resultPath) { Get-Content $resultPath -Raw } else { '' }
    Write-Host "priv exit=$($proc.ExitCode) result=$resultText"
    if (-not $resultText) { throw "tunelito-priv produced no result (exit $($proc.ExitCode))" }
    $parsed = $resultText | ConvertFrom-Json
    if (-not $parsed.ok) { throw "tunelito-priv failed: $($parsed.detail)" }
    if (-not (Get-Command ssh -ErrorAction SilentlyContinue)) {
        throw 'OpenSSH client (ssh) is not on PATH'
    }
    Show-Setup
    $sshArgs = @('-v', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=accept-new', '-o', 'ConnectTimeout=20', '-i', $key, '-p', '22', 'tunelito@127.0.0.1', 'echo', 'tunelito-ok')
    $sshOut = & ssh @sshArgs 2>&1 | Out-String
    $sshCode = $LASTEXITCODE
    if ($sshCode -ne 0) {
        Write-Host $sshOut
        Show-Setup
        $cfg = 'C:\ProgramData\ssh\sshd_config'
        $raw = Get-Content -LiteralPath $cfg -Raw
        if ($raw -notmatch '(?m)^LogLevel\s') {
            Set-Content -LiteralPath $cfg -Value ("LogLevel DEBUG3`r`n" + $raw) -Encoding ascii
            Restart-Service sshd
            $sshOut = & ssh @sshArgs 2>&1 | Out-String
            $sshCode = $LASTEXITCODE
            Write-Host $sshOut
        }
        Get-ChildItem 'C:\ProgramData\ssh\logs' -ErrorAction SilentlyContinue | ForEach-Object {
            Write-Host "--- $($_.FullName) ---"
            Get-Content -LiteralPath $_.FullName -Tail 80
        }
        foreach ($logName in @('OpenSSH/Operational', 'OpenSSH/Admin')) {
            Get-WinEvent -LogName $logName -MaxEvents 40 -ErrorAction SilentlyContinue |
                ForEach-Object { Write-Host "$logName $($_.TimeCreated.ToString('o')) $($_.Message)" }
        }
        if ($sshCode -ne 0) { throw "localhost ssh failed: $sshCode" }
    }
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
