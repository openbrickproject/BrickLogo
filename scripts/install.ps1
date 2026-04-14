# BrickLogo installer for Windows
#
# Install or update BrickLogo with:
#
#   irm https://raw.githubusercontent.com/openbrickproject/BrickLogo/main/scripts/install.ps1 | iex
#
# Installs to ~\.bricklogo and adds it to your PATH. Preserves any existing
# bricklogo.config.json on upgrade.

$ErrorActionPreference = "Stop"

$Repo = "openbrickproject/BrickLogo"
$InstallDir = "$env:USERPROFILE\.bricklogo"

# ── Detect platform ──────────────────────────────

$Arch = $env:PROCESSOR_ARCHITECTURE
switch ($Arch) {
    "AMD64" { $Platform = "windows-x64" }
    default { Write-Error "Unsupported architecture: $Arch"; exit 1 }
}

# ── Fetch latest release tag ─────────────────────

Write-Host "Fetching latest release..."
$Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
$Version = $Release.tag_name

if (-not $Version) {
    Write-Error "Failed to determine latest release version."
    exit 1
}

Write-Host "Installing BrickLogo $Version for $Platform..."

# ── Download and extract ─────────────────────────

$Url = "https://github.com/$Repo/releases/download/$Version/bricklogo-$Version-$Platform.zip"
$TmpZip = Join-Path $env:TEMP "bricklogo-install.zip"
$TmpDir = Join-Path $env:TEMP "bricklogo-install"

Invoke-WebRequest -Uri $Url -OutFile $TmpZip

if (Test-Path $TmpDir) { Remove-Item $TmpDir -Recurse -Force }
Expand-Archive -Path $TmpZip -DestinationPath $TmpDir

# ── Preserve user config on upgrade ──────────────

$SavedConfig = $null
$ConfigPath = Join-Path $InstallDir "bricklogo.config.json"
if (Test-Path $ConfigPath) {
    $SavedConfig = Join-Path $env:TEMP "bricklogo.config.json.bak"
    Copy-Item $ConfigPath $SavedConfig
}

# ── Install to ~\.bricklogo ──────────────────────

if (Test-Path $InstallDir) { Remove-Item $InstallDir -Recurse -Force }
Move-Item (Join-Path $TmpDir "bricklogo") $InstallDir

if ($SavedConfig) {
    Copy-Item $SavedConfig $ConfigPath
}

# ── Clean up temp files ──────────────────────────

Remove-Item $TmpZip -Force -ErrorAction SilentlyContinue
Remove-Item $TmpDir -Recurse -Force -ErrorAction SilentlyContinue

# ── Add to PATH if not already present ───────────

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$InstallDir;$UserPath", "User")
    Write-Host ""
    Write-Host "Added $InstallDir to your PATH."
    Write-Host "Restart your terminal for the PATH change to take effect."
}

# ── Done ─────────────────────────────────────────

Write-Host ""
Write-Host "BrickLogo $Version installed successfully."
Write-Host ""
Write-Host "  Examples: $InstallDir\examples\"
Write-Host "  Docs:     $InstallDir\docs\"
Write-Host ""
Write-Host "Run 'bricklogo' to get started."
Write-Host ""
