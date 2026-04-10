$ErrorActionPreference = "Stop"

# Get version from Cargo.toml
$version = (Select-String -Path "crates\bricklogo\Cargo.toml" -Pattern '^version' | Select-Object -First 1).Line -replace '.*"(.*)".*', '$1'

$arch = if ([System.Environment]::Is64BitOperatingSystem) { "x64" } else { "x86" }
$zipName = "bricklogo-v${version}-windows-${arch}.zip"

Write-Host "Building BrickLogo v${version} for windows-${arch}..."
cargo build --release --bin bricklogo
if ($LASTEXITCODE -ne 0) { exit 1 }

Write-Host "Creating ${zipName}..."

$staging = Join-Path $env:TEMP "bricklogo-release"
if (Test-Path $staging) { Remove-Item $staging -Recurse -Force }
New-Item -ItemType Directory -Path "$staging\bricklogo" | Out-Null

Copy-Item "target\release\bricklogo.exe" "$staging\bricklogo\"
Copy-Item "bricklogo.config.json.example" "$staging\bricklogo\"
Copy-Item -Recurse "examples" "$staging\bricklogo\examples"
Copy-Item -Recurse "firmware" "$staging\bricklogo\firmware"
Copy-Item -Recurse "docs" "$staging\bricklogo\docs"

if (-not (Test-Path "releases")) { New-Item -ItemType Directory -Path "releases" | Out-Null }
$zipPath = Join-Path "releases" $zipName
if (Test-Path $zipPath) { Remove-Item $zipPath }
Compress-Archive -Path "$staging\bricklogo" -DestinationPath $zipPath

Remove-Item $staging -Recurse -Force

Write-Host "Done: releases\${zipName}"
