# TexelBox release pipeline:
#   build -> (optional) sign -> stage dist\TexelBox\
#
# Usage:
#   powershell -File scripts\release.ps1          # build + stage
#   powershell -File scripts\release.ps1 -Sign    # also sign exe
#   powershell -File scripts\release.ps1 -SkipBuild  # restage existing binary

param(
    [switch]$Sign,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

$cargoToml = Get-Content -LiteralPath "$root\Cargo.toml" -Raw
if ($cargoToml -notmatch 'version\s*=\s*"([0-9][^"]*)"') {
    throw "could not read version from Cargo.toml"
}
$version = $Matches[1]
Write-Host "==> Releasing TexelBox $version"

if (-not $SkipBuild) {
    Write-Host "==> cargo build --release --bin texelbox"
    Push-Location $root
    $savedEA = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try { & cargo build --release --bin texelbox } finally {
        $ErrorActionPreference = $savedEA
        Pop-Location
    }
    if ($LASTEXITCODE -ne 0) { throw "release build failed" }
}

$exe = "$root\target\release\texelbox.exe"
if (-not (Test-Path -LiteralPath $exe)) { throw "missing $exe" }

if ($Sign) {
    Write-Host "==> signing exe"
    & "$PSScriptRoot\sign.ps1" -Binary $exe
}

$dist = "$root\dist\TexelBox"
if (Test-Path -LiteralPath $dist) { Remove-Item -LiteralPath $dist -Recurse -Force }
New-Item -ItemType Directory -Path $dist | Out-Null
Copy-Item -LiteralPath $exe -Destination "$dist\texelbox.exe"

Write-Host ""
Write-Host "==> Done! Portable binary staged at: $dist\texelbox.exe"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Zip dist\TexelBox\ manually"
Write-Host "  2. Upload to GitHub Releases as v$version"
Write-Host "  3. Update Cargo.toml version BEFORE THE BUILD and Update worker\wrangler.toml LATEST_VERSION and LATEST_DOWNLOAD_URL"
Write-Host "  4. cd worker; npx wrangler deploy"
