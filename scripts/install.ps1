# gui-gate installer (Windows): ONE zip from this repo's GitHub release holds the gate and the Cua Driver build it was
# tested with (official, unmodified, MIT; verified against Cua's SHA256SUMS + Sigstore when the release was made).
# Cua's own installer is never run (no scheduled task, no junctions, no extra Path entry):
#   %LOCALAPPDATA%\Programs\gui-gate\bin\gui-gate.exe      (this dir is added to your user Path)
#   %LOCALAPPDATA%\Programs\gui-gate\cua-driver\           (cua-driver.exe and its LICENSE)
#   uninstall: gui-gate uninstall
#
#   irm https://github.com/cszf1/gui-gate/releases/latest/download/install.ps1 | iex
#
# Options via env (irm | iex takes no arguments):
#   $env:GUI_GATE_CLIENT = "claude"        claude|codex|cursor|pi, comma separated
#   $env:GUI_GATE_VERSION = "0.6.1"        pin a gui-gate release
#   $env:GUI_GATE_INSTALL_DIR = "C:\..."   default %LOCALAPPDATA%\Programs\gui-gate\bin
#   $env:GUI_GATE_SKIP_DRIVER = "1"        do not install the bundled Cua Driver (use one installed separately)
#   $env:GUI_GATE_BASE_URL = "https://..." download gui-gate from a mirror instead of GitHub
# The gate needs a reviewer model (it refuses to start without one). Unless one is already configured the
# installer asks for it in an interactive console, or takes it from these (saved to ~\.config\gui-gate\reviewer.env):
#   $env:GUI_GATE_REVIEWER_URL = "https://api.deepseek.com/v1"   base URL or full endpoint
#   $env:GUI_GATE_REVIEWER_MODEL = "deepseek-chat"
#   $env:GUI_GATE_REVIEWER_KEY_FILE = "C:\...\key.txt"            a file holding only the API key
#     (or $env:GUI_GATE_REVIEWER_KEY = "...", saved to ~\.config\gui-gate\reviewer.key readable only by you)
#   $env:GUI_GATE_REVIEWER_PROTOCOL = "auto"                       auto|chat|responses|anthropic|gemini|systemone
#   $env:GUI_GATE_NO_PROMPT = "1"                                  never ask in the console
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

    $Root = Split-Path $BinDir
    $Manifest = Join-Path $Root 'install-manifest'
    $UserHome0 = if ($env:HOME) { $env:HOME } else { $env:USERPROFILE }
    # dirs this installer creates (under the user's home / LOCALAPPDATA, only if missing now): `gui-gate uninstall`
    # removes exactly those once empty; an upgrade keeps what the first install recorded
    $created = @()
    if (Test-Path $Manifest) { $created = @(Get-Content $Manifest | Where-Object { $_ -like 'created_dir=*' } | ForEach-Object { $_.Substring(12) }) }
    foreach ($d in @($BinDir, (Join-Path $UserHome0 '.config\gui-gate'))) {
        $x = $d
        while ($x -and -not (Test-Path $x) -and ($x.StartsWith($UserHome0) -or ($env:LOCALAPPDATA -and $x.StartsWith($env:LOCALAPPDATA)))) {
            if ($x -eq $UserHome0 -or $x -eq $env:LOCALAPPDATA) { break }
            $created += $x
            $x = Split-Path $x
        }
    }
    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
    $Exe = Join-Path $BinDir 'gui-gate.exe'
    if (Test-Path $Exe) {
        Remove-Item -Force "$Exe.old" -ErrorAction SilentlyContinue
        # delete the old exe; a running gate keeps it locked, then rename it (allowed on Windows) and
        # `gui-gate cleanup` / the next gate start removes gui-gate.exe.old once it is free
        try { Remove-Item -Force $Exe -ErrorAction Stop } catch { Rename-Item -Path $Exe -NewName 'gui-gate.exe.old' }
    }
    Copy-Item (Join-Path $Tmp 'gui-gate.exe') $Exe
    Say "installed gui-gate $(& $Exe --version) -> $Exe"

    # the bundled Cua Driver, next to the gate in gui-gate's own dir
    $DrvSrc = Join-Path $Tmp 'cua-driver'
    if ($env:GUI_GATE_SKIP_DRIVER -ne '1' -and (Test-Path $DrvSrc)) {
        $Drv = Join-Path $Root 'cua-driver'
        $New = Join-Path $Root '.cua-driver.new'
        Remove-Item -Recurse -Force $New -ErrorAction SilentlyContinue
        Copy-Item -Recurse $DrvSrc $New
        if (Test-Path $Drv) {
            # a running driver keeps it locked: rename it; `gui-gate uninstall` / the next install removes it
            try { Remove-Item -Recurse -Force $Drv -ErrorAction Stop }
            catch { Rename-Item -Path $Drv -NewName ("cua-driver.old-" + [guid]::NewGuid().ToString('N').Substring(0, 8)) }
        }
        Get-ChildItem $Root -Directory -Filter 'cua-driver.old-*' -ErrorAction SilentlyContinue | ForEach-Object { Remove-Item -Recurse -Force $_.FullName -ErrorAction SilentlyContinue }
        Rename-Item -Path $New -NewName 'cua-driver'
        Say "installed Cua Driver $((Get-Content (Join-Path $Drv 'VERSION') -ErrorAction SilentlyContinue) -join '') (bundled) -> $Drv"
        foreach ($c in @((Join-Path $env:LOCALAPPDATA 'Programs\Cua\cua-driver\bin\cua-driver.exe'), (Join-Path $env:USERPROFILE '.cua-driver\packages\current\cua-driver.exe'))) {
            if (Test-Path $c) { Say "note: a Cua Driver installed by Cua's own installer is at $c; it is left untouched (gui-gate uses its bundled one; GUI_GATE_DRIVER=external selects yours)"; break }
        }
    }
    $lines = @("# written by gui-gate's installer; read by gui-gate uninstall") + @($created | Select-Object -Unique | ForEach-Object { "created_dir=$_" })
    [IO.File]::WriteAllLines($Manifest, [string[]]$lines, (New-Object Text.UTF8Encoding($false)))
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($userPath -split ';') -contains $BinDir)) {
    [Environment]::SetEnvironmentVariable('Path', ((@($userPath, $BinDir) | Where-Object { $_ }) -join ';'), 'User')
    Say "added $BinDir to your PATH (open a new terminal)"
}
if (-not (($env:Path -split ';') -contains $BinDir)) { $env:Path = "$BinDir;$env:Path" }

# leave no residue: old copies (incl. gui-gate.exe.old), old Python package, duplicate/stale user Path entries
# (registry HKCU\Environment), stale MCP entries and flags, our own config backups, old caches
& $Exe cleanup --quiet

# ------------------------------------------------------------------ reviewer model (required)
# (Windows PowerShell 5.1 turns a redirected native stderr line into an error; with 'Stop' that would abort)
$ErrorActionPreference = 'Continue'
$UserHome = if ($env:HOME) { $env:HOME } else { $env:USERPROFILE }
$CfgDir = Join-Path $UserHome '.config\gui-gate'
$RevEnv = if ($env:GUI_GATE_REVIEWER_ENV) { $env:GUI_GATE_REVIEWER_ENV } else { Join-Path $CfgDir 'reviewer.env' }
function Test-Reviewer { & $Exe check-reviewer --config-only *> $null; return ($LASTEXITCODE -eq 0) }
function Protect-File($f) { try { icacls $f /inheritance:r /grant:r "$($env:USERNAME):F" *> $null } catch {} }
function Set-Rev($k, $v) {
    New-Item -ItemType Directory -Force -Path (Split-Path $RevEnv) | Out-Null
    $lines = @()
    if (Test-Path $RevEnv) { $lines = @(Get-Content $RevEnv | Where-Object { $_ -notmatch "^(export )?$k=" }) }
    $lines += "$k=$v"
    [IO.File]::WriteAllLines($RevEnv, [string[]]$lines, (New-Object Text.UTF8Encoding($false)))
    Protect-File $RevEnv
}
function Save-Key($key) {
    New-Item -ItemType Directory -Force -Path $CfgDir | Out-Null
    $kf = Join-Path $CfgDir 'reviewer.key'
    [IO.File]::WriteAllText($kf, $key, (New-Object Text.UTF8Encoding($false)))
    Protect-File $kf
    return $kf
}
$RUrl = $env:GUI_GATE_REVIEWER_URL; $RModel = $env:GUI_GATE_REVIEWER_MODEL
$RKeyFile = $env:GUI_GATE_REVIEWER_KEY_FILE; $RProto = $env:GUI_GATE_REVIEWER_PROTOCOL
if ($env:GUI_GATE_REVIEWER_KEY -and -not $RKeyFile) { $RKeyFile = Save-Key $env:GUI_GATE_REVIEWER_KEY }
$interactive = ($env:GUI_GATE_NO_PROMPT -ne '1') -and (-not $env:CI) -and [Environment]::UserInteractive -and (-not [Console]::IsInputRedirected)
if (-not ($RUrl -or $RModel -or $RKeyFile -or $RProto) -and -not (Test-Reviewer) -and $interactive) {
    Write-Host ''
    Write-Host 'gui-gate needs a reviewer model: an independent model that checks risky or doubtful steps'
    Write-Host '(any LLM API - OpenAI-compatible chat, OpenAI responses, Anthropic, Gemini - or a System One'
    Write-Host 'decision model). The gate does not start without one. Leave the URL empty to skip for now.'
    $RUrl = Read-Host 'Reviewer API URL (e.g. https://api.deepseek.com/v1)'
    if ($RUrl) {
        $RModel = Read-Host 'Model name (e.g. deepseek-chat)'
        $sec = Read-Host "API key (hidden; saved to $CfgDir\reviewer.key, readable only by you)" -AsSecureString
        $key = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec))
        if ($key) { $RKeyFile = Save-Key $key }
        Remove-Variable key, sec -ErrorAction SilentlyContinue
        $old = @{}; foreach ($k in 'GUI_GATE_REVIEWER_ENV','GUI_GATE_REVIEWER_URL','GUI_GATE_REVIEWER_MODEL','GUI_GATE_REVIEWER_KEY') { $old[$k] = [Environment]::GetEnvironmentVariable($k) }
        $env:GUI_GATE_REVIEWER_ENV = 'nonexistent'; $env:GUI_GATE_REVIEWER_URL = $RUrl; $env:GUI_GATE_REVIEWER_MODEL = $(if ($RModel) { $RModel } else { 'x' }); $env:GUI_GATE_REVIEWER_KEY = 'x'
        $guess = 'chat'
        try { $guess = ((& $Exe check-reviewer --config-only 2>$null) | ConvertFrom-Json).reviewer.Split(':')[1] } catch {}
        foreach ($k in $old.Keys) { [Environment]::SetEnvironmentVariable($k, $old[$k]) }
        $RProto = Read-Host "Protocol [auto = $guess] (chat|responses|anthropic|gemini|systemone)"
    }
}
if ($RUrl -or $RModel -or $RKeyFile -or $RProto) {
    if ($RUrl) { Set-Rev 'GUI_GATE_REVIEWER_URL' $RUrl }
    if ($RModel) { Set-Rev 'GUI_GATE_REVIEWER_MODEL' $RModel }
    if ($RKeyFile) { Set-Rev 'GUI_GATE_REVIEWER_KEY_FILE' ([IO.Path]::GetFullPath($RKeyFile)) }
    if ($RProto -and $RProto -ne 'auto') { Set-Rev 'GUI_GATE_REVIEWER_PROTOCOL' $RProto }
    Say "saved the reviewer settings to $RevEnv"
    # from here on check what was saved, not this session's environment (the key never stays in env)
    foreach ($k in 'GUI_GATE_REVIEWER','GUI_GATE_REVIEWER_URL','GUI_GATE_REVIEWER_MODEL','GUI_GATE_REVIEWER_KEY','GUI_GATE_REVIEWER_KEY_FILE','GUI_GATE_REVIEWER_PROTOCOL') { Remove-Item "Env:$k" -ErrorAction SilentlyContinue }
}
if (Test-Reviewer) {
    Say "checking the reviewer model"
    & $Exe check-reviewer
    if ($LASTEXITCODE -ne 0) { Write-Warning "the reviewer is configured but did not pass its check (see above); fix the settings in $RevEnv" }
} else {
    Write-Warning 'no reviewer model is configured: the gate will NOT start until one is set.'
    Write-Host "Set GUI_GATE_REVIEWER_URL / _MODEL / _KEY_FILE in $RevEnv (or re-run this installer in a console, or with"
    Write-Host '$env:GUI_GATE_REVIEWER_URL / _MODEL / _KEY_FILE set), then run: gui-gate check-reviewer'
}
$global:LASTEXITCODE = 0
$ErrorActionPreference = 'Stop'

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
Write-Host 'Remove everything this installed: gui-gate uninstall'
