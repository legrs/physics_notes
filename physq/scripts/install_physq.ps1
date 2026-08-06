# Install physq (Physics Notes terminal search) on Windows from the latest
# stable release — PowerShell-native counterpart to install_physq.sh (which
# works too, via Git Bash). Detects x86_64 vs arm64, downloads the matching
# unarchived binary, verifies its SHA-256 against the release's
# checksums.txt, and installs it into DEST.
#
# Usage:
#   .\install_physq.ps1                      # global (default): into
#                                            # $env:USERPROFILE\bin + user PATH
#   .\install_physq.ps1 -Local               # same dir, no PATH change
#   .\install_physq.ps1 -Dest C:\tools       # custom dir, no PATH change
#
# Note: the binary is unsigned, so Windows SmartScreen may warn on first run
# ("More info -> Run anyway").

param(
    [string]$Dest = (Join-Path $env:USERPROFILE "bin"),
    [switch]$Local
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"   # speed up Invoke-WebRequest

$BaseUrl = "https://github.com/legrs/physics_notes/releases/latest/download"

# --- detect arch and pick the asset name -----------------------------------
switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { $Arch = "x86_64" }
    "ARM64" { $Arch = "aarch64" }
    default { Write-Error "unsupported architecture: $env:PROCESSOR_ARCHITECTURE"; exit 1 }
}
$Bin = "physq-bin-$Arch-pc-windows-msvc.exe"

$Tmp = Join-Path $env:TEMP ("physq-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    Write-Host "Downloading $Bin from the latest release..."
    Invoke-WebRequest -Uri "$BaseUrl/$Bin" -OutFile (Join-Path $Tmp $Bin)
    Invoke-WebRequest -Uri "$BaseUrl/checksums.txt" -OutFile (Join-Path $Tmp "checksums.txt")

    Write-Host "Verifying SHA-256 against checksums.txt..."
    $Expected = (Select-String -Path (Join-Path $Tmp "checksums.txt") -Pattern " $([regex]::Escape($Bin))$" | ForEach-Object { ($_.Line -split " +")[0] })
    if (-not $Expected) { throw "no checksum for $Bin in checksums.txt" }
    $Actual = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $Tmp $Bin)).Hash.ToLower()
    if ($Actual -ne $Expected.ToLower()) { throw "checksum mismatch for $Bin" }
    Write-Host "$Bin : OK"

    # --- install -------------------------------------------------------------
    New-Item -ItemType Directory -Path $Dest -Force | Out-Null
    Move-Item -Path (Join-Path $Tmp $Bin) -Destination (Join-Path $Dest "physq.exe") -Force
    Unblock-File -Path (Join-Path $Dest "physq.exe")   # strip Zone.Identifier (SmartScreen)

    Write-Host ""
    Write-Host "Installed:"
    Write-Host "  $Dest\physq.exe"
    Write-Host ""
    & (Join-Path $Dest "physq.exe") --version
    Write-Host "To update later: $Dest\physq.exe update"

    # --- global install (default): add DEST to the user PATH ---------------
    if (-not $Local) {
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if ($userPath -and $userPath.Split(";") -contains $Dest) {
            Write-Host "$Dest is already on your PATH."
        }
        else {
            [Environment]::SetEnvironmentVariable("Path", $(if ($userPath) { "$userPath;$Dest" } else { $Dest }), "User")
            Write-Host "Added $Dest to your user PATH (takes effect in new terminals)."
            Write-Host "Then run: physq --help"
        }
    }
}
finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}
