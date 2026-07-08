# usbtree installer — https://github.com/gnomeria/usbtree
#
# Usage:
#   irm https://raw.githubusercontent.com/gnomeria/usbtree/main/scripts/install.ps1 | iex
#
# Environment variables:
#   USBTREE_VERSION      install a specific version (e.g. "0.1.0"), default: latest release
#   USBTREE_INSTALL_DIR  install directory, default: %LOCALAPPDATA%\usbtree\bin
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Repo = 'gnomeria/usbtree'
$Bin  = 'usbtree'

# Windows PowerShell 5.1 negotiates SSLv3/TLS1.0 by default; GitHub needs TLS 1.2.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

function Info($m) { Write-Host "· $m" -ForegroundColor DarkGray }
function Ok($m)   { Write-Host "* $m" -ForegroundColor Green }
function Warn($m) { Write-Host "! $m" -ForegroundColor Yellow }
function Die($m)  { Write-Host "x $m" -ForegroundColor Red; exit 1 }

# ---- platform check --------------------------------------------------------
# Only windows-amd64 is built. On arm64 the x64 binary runs under emulation.
# PROCESSOR_ARCHITEW6432 is set (to the OS arch) when a 32-bit shell runs on a
# 64-bit OS — prefer it so a 32-bit host doesn't misreport as x86 and bail.
$arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
if ($arch -notin 'AMD64', 'ARM64') {
    Die "unsupported architecture: $arch (only windows-amd64 is published)"
}
if ($arch -eq 'ARM64') {
    Warn 'no native arm64 build — installing the amd64 binary (runs under x64 emulation)'
}

# ---- resolve version -------------------------------------------------------
$Version = $env:USBTREE_VERSION
if (-not $Version) {
    Info 'resolving latest release...'
    try {
        $rel = Invoke-RestMethod -UseBasicParsing "https://api.github.com/repos/$Repo/releases/latest"
        $Version = $rel.tag_name
    } catch {
        Die "couldn't reach the GitHub API — set USBTREE_VERSION or check https://github.com/$Repo/releases"
    }
    if (-not $Version) {
        Die "couldn't determine the latest release — is one published? Set USBTREE_VERSION or check https://github.com/$Repo/releases"
    }
}
$Version = $Version -replace '^v', ''

$Asset   = "${Bin}_${Version}_windows-amd64.zip"
$BaseUrl = "https://github.com/$Repo/releases/download/v$Version"

Info "installing $Bin v$Version (windows-amd64)"

# ---- download + verify -----------------------------------------------------
$Tmp = Join-Path ([IO.Path]::GetTempPath()) ("usbtree-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    $zip = Join-Path $Tmp $Asset
    Info "downloading $Asset..."
    try {
        Invoke-WebRequest -UseBasicParsing "$BaseUrl/$Asset" -OutFile $zip
    } catch {
        Die "download failed: $BaseUrl/$Asset"
    }

    try {
        $sums = (Invoke-WebRequest -UseBasicParsing "$BaseUrl/checksums.txt").Content
        $line = $sums -split "`n" | Where-Object { $_ -match "\s$([regex]::Escape($Asset))\s*$" } | Select-Object -First 1
        if ($line) {
            $expected = ($line -split '\s+')[0].ToLower()
            $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
            if ($actual -ne $expected) {
                Die "checksum mismatch for $Asset (expected $expected, got $actual)"
            }
            Ok 'checksum verified'
        } else {
            Warn "$Asset not listed in checksums.txt — skipping verification"
        }
    } catch {
        Warn 'checksums.txt not found in release — skipping verification'
    }

    Expand-Archive -Path $zip -DestinationPath $Tmp -Force
    $src = Join-Path $Tmp "$Bin.exe"
    if (-not (Test-Path $src)) { Die "archive didn't contain $Bin.exe" }

    # ---- install ----------------------------------------------------------
    $InstallDir = $env:USBTREE_INSTALL_DIR
    if (-not $InstallDir) { $InstallDir = Join-Path $env:LOCALAPPDATA 'usbtree\bin' }
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item $src (Join-Path $InstallDir "$Bin.exe") -Force
    Ok "installed $InstallDir\$Bin.exe"

    # ---- ensure it's on PATH ----------------------------------------------
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $onPath = ($userPath -split ';') -contains $InstallDir
    if (-not $onPath) {
        $newPath = if ($userPath) { "$userPath;$InstallDir" } else { $InstallDir }
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        $env:Path = "$env:Path;$InstallDir"
        Ok "added $InstallDir to your user PATH — restart your terminal for it to take effect"
    }
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}

Write-Host ''
Warn 'Windows binaries are unsigned — SmartScreen may warn on first run.'
Write-Host "Run $Bin for the TUI, $Bin --dump to print the tree once, or $Bin --updatelist to refresh the usb.ids database."
