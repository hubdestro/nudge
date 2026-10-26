# Nudge installer for Windows.
#
#   irm https://raw.githubusercontent.com/hubdestro/nudge/main/install.ps1 | iex
#
# Environment overrides: NUDGE_REPO, NUDGE_VERSION, NUDGE_INSTALL_DIR, NUDGE_DOWNLOAD_URL
$ErrorActionPreference = 'Stop'

$Repo = if ($env:NUDGE_REPO) { $env:NUDGE_REPO } else { 'hubdestro/nudge' }
$Version = if ($env:NUDGE_VERSION) { $env:NUDGE_VERSION } else { 'latest' }
$InstallDir = if ($env:NUDGE_INSTALL_DIR) { $env:NUDGE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\nudge' }

$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') { 'aarch64' } else { 'x86_64' }
$target = "$arch-pc-windows-msvc"

$url = if ($env:NUDGE_DOWNLOAD_URL) { $env:NUDGE_DOWNLOAD_URL }
       elseif ($Version -eq 'latest') { "https://github.com/$Repo/releases/latest/download/nudge-$target.zip" }
       else { "https://github.com/$Repo/releases/download/$Version/nudge-$target.zip" }

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("nudge-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Write-Host "Downloading nudge for $target..."
    Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp 'nudge.zip') -UseBasicParsing
    Expand-Archive -Path (Join-Path $tmp 'nudge.zip') -DestinationPath $tmp -Force

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $exe = Join-Path $InstallDir 'nudge.exe'
    if (Test-Path $exe) { & $exe stop *> $null }
    Copy-Item (Join-Path $tmp 'nudge.exe') $exe -Force
} finally {
    Remove-Item -Recurse -Force $tmp
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$InstallDir", 'User')
    $env:Path = "$env:Path;$InstallDir"
    Write-Host "Added $InstallDir to your PATH (open a new terminal to pick it up)."
}

Write-Host "Installed $(& $exe --version) to $exe"
Write-Host ""
Write-Host "Get started:"
Write-Host "  nudge start      # keep your status green"
Write-Host "  nudge status"
