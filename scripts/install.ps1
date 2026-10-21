# gui-gate installer (Windows): the gate binary from this repo's GitHub release,
# plus the latest Cua Driver through Cua's own official installer.
#
#   irm https://github.com/cszf1/gui-gate/releases/latest/download/install.ps1 | iex
#
# Options via env (irm | iex takes no arguments):
#   $env:GUI_GATE_CLIENT = "claude"        claude|codex|cursor|pi, comma separated
#   $env:GUI_GATE_VERSION = "0.6.1"        pin a gui-gate release
#   $env:GUI_GATE_INSTALL_DIR = "C:\..."   default %LOCALAPPDATA%\Programs\gui-gate\bin
#   $env:GUI_GATE_SKIP_DRIVER = "1"        do not install / update Cua Driver
#   $env:GUI_GATE_BASE_URL = "https://..." download gui-gate from a mirror instead of GitHub
# Cua Driver's own env vars (e.g. $env:CUA_DRIVER_RS_VERSION) pass through to its installer.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

function Say($m) { Write-Host "==> $m" -ForegroundColor Cyan }

$Repo = 'cszf1/gui-gate'
$BinDir = if ($env:GUI_GATE_INSTALL_DIR) { $env:GUI_GATE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\gui-gate\bin' }
$Base = if ($env:GUI_GATE_BASE_URL) { $env:GUI_GATE_BASE_URL } elseif ($env:GUI_GATE_VERSION) { "https://github.com/$Repo/releases/download/v$($env:GUI_GATE_VERSION.TrimStart('v'))" } else { "https://github.com/$Repo/releases/latest/download" }
# x64 build; Windows on ARM runs it under emulation.
$Asset = 'gui-gate-windows-x86_64.zip'

$Tmp = Join-Path ([IO.Path]::GetTempPath()) ("gui-gate-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    Say "downloading $Asset"
    Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Asset" -OutFile (Join-Path $Tmp $Asset)
    Invoke-WebRequest -UseBasicParsing -Uri "$Base/SHA256SUMS" -OutFile (Join-Path $Tmp 'SHA256SUMS')
    $want = (Get-Content (Join-Path $Tmp 'SHA256SUMS') | Where-Object { ($_ -split '\s+')[1] -replace '^\*', '' -eq $Asset } | ForEach-Object { ($_ -split '\s+')[0] }) | Select-Object -First 1
    if (-not $want) { throw "$Asset is not listed in SHA256SUMS" }
    $got = (Get-FileHash -Algorithm SHA256 (Join-Path $Tmp $Asset)).Hash
    if ($got.ToLower() -ne $want.ToLower()) { throw "checksum mismatch for $Asset" }
    Expand-Archive -Path (Join-Path $Tmp $Asset) -DestinationPath $Tmp -Force

    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
    $Exe = Join-Path $BinDir 'gui-gate.exe'
    if (Test-Path $Exe) {
        # a running gate keeps the exe locked; renaming a running exe is allowed on Windows
        $Old = "$Exe.old"
        Remove-Item -Force $Old -ErrorAction SilentlyContinue
        Rename-Item -Path $Exe -NewName 'gui-gate.exe.old'
    }
    Copy-Item (Join-Path $Tmp 'gui-gate.exe') $Exe
    Say "installed gui-gate $(& $Exe --version) -> $Exe"
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($userPath -split ';') -contains $BinDir)) {
    [Environment]::SetEnvironmentVariable('Path', ((@($userPath, $BinDir) | Where-Object { $_ }) -join ';'), 'User')
    Say "added $BinDir to your PATH (open a new terminal)"
}
if (-not (($env:Path -split ';') -contains $BinDir)) { $env:Path = "$BinDir;$env:Path" }

if ($env:GUI_GATE_SKIP_DRIVER -ne '1') {
    Say "installing the latest Cua Driver with Cua's official installer"
    $Shell = (Get-Process -Id $PID).Path
    & $Shell -NoProfile -ExecutionPolicy Bypass -Command "irm https://cua.ai/driver/install.ps1 | iex"
    if ($LASTEXITCODE -ne 0) { throw "Cua Driver's installer failed (see above). gui-gate itself is installed; re-run this command to retry." }
    # pick up the driver's PATH entry in this session too
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $env:Path = "$BinDir;" + [Environment]::GetEnvironmentVariable('Path', 'User') + ";$machine"
}

if ($env:GUI_GATE_CLIENT) {
    $args2 = @('setup')
    foreach ($c in ($env:GUI_GATE_CLIENT -split ',')) { if ($c.Trim()) { $args2 += @('--client', $c.Trim()) } }
    Say "adding gui-gate to $($env:GUI_GATE_CLIENT)"
    & $Exe @args2
} else {
    Say 'checking the install'
    & $Exe doctor
    Write-Host ''
    Write-Host 'Next: add it to your agent, e.g.   gui-gate setup --client claude   (or codex / cursor / pi)'
}
