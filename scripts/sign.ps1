# Code-sign the release binary (spec §4.5 — SmartScreen/user trust).
# Requires a code-signing certificate (OV/EV). Set the paths below or pass
# them as environment variables before running:
#
#   $env:SIGN_CERT = "C:\certs\texelbox.pfx"
#   $env:SIGN_CERT_PASSWORD = "..."
#   powershell -File scripts\sign.ps1
#
# EV certificates on hardware tokens need signtool's /csp + /kc options
# instead of /f + /p — see your CA's docs.

param(
    [string]$Binary = "$PSScriptRoot\..\target\release\texelbox.exe",
    [string]$TimestampServer = "http://timestamp.digicert.com"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Binary)) {
    throw "Binary not found: $Binary  (run `cargo build --release` first)"
}
if (-not $env:SIGN_CERT) { throw "Set `$env:SIGN_CERT to the .pfx path" }

& signtool sign /fd SHA256 /tr $TimestampServer /td SHA256 /f $env:SIGN_CERT /p $env:SIGN_CERT_PASSWORD $Binary
if ($LASTEXITCODE -ne 0) { throw "signtool sign failed ($LASTEXITCODE)" }

& signtool verify /pa /v $Binary
if ($LASTEXITCODE -ne 0) { throw "signtool verify failed ($LASTEXITCODE)" }

Write-Host "Signed OK: $Binary"
