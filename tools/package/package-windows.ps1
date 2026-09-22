<#
.SYNOPSIS
    Build the unsigned per-machine MSI with WiX v3.

.DESCRIPTION
    Run after building the release binaries:

        cargo build --release --locked -p tore-app -p tore-extract
        pwsh tools/package/package-windows.ps1 -Version 0.2.0

    The MSI lands in dist\ and the staged directory stays in
    dist\stage\windows\ so the asset guard can scan real files rather than a
    compressed blob. Nothing is signed: the first launch shows SmartScreen,
    where the player picks More info, then Run anyway.

    WiX v3 (candle.exe and light.exe) ships on the windows-2022 runner image.
    Locally, install the WiX Toolset v3.14 or later, or pass -WixBin. Both
    WixUIExtension and WixUtilExtension are part of that install; the exit
    dialog's launch checkbox needs the second one.

.PARAMETER Version
    Package version. Defaults to the tag the release workflow is running for,
    then to git describe, then to 0.0.0-dev. A leading "v" is stripped.

.PARAMETER WixBin
    Directory holding candle.exe and light.exe, when they are not on PATH and
    not in the usual install locations.
#>
[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$WixBin = ""
)

$ErrorActionPreference = "Stop"

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Split-Path -Parent (Split-Path -Parent $here)

function Resolve-ToreVersion {
    param([string]$Explicit)
    $value = $Explicit
    if (-not $value -and $env:GITHUB_REF_TYPE -eq "tag") { $value = $env:GITHUB_REF_NAME }
    if (-not $value) {
        $described = & git -C $root describe --tags --always --dirty 2>$null
        if ($LASTEXITCODE -eq 0) { $value = $described }
    }
    if (-not $value) { $value = "0.0.0-dev" }
    return ($value -replace '^v', '').Trim()
}

# Windows Installer requires a strictly numeric three-part version. A tag such
# as 0.2.0-rc1 becomes 0.2.0; anything without a numeric lead becomes 0.0.0,
# which is correct for a local development build.
function Get-MsiVersion {
    param([string]$Version)
    if ($Version -match '^(\d+)\.(\d+)\.(\d+)') { return "$($Matches[1]).$($Matches[2]).$($Matches[3])" }
    if ($Version -match '^(\d+)\.(\d+)') { return "$($Matches[1]).$($Matches[2]).0" }
    return "0.0.0"
}

function Find-WixTool {
    param([string]$Name, [string]$Hint)
    if ($Hint) {
        $candidate = Join-Path $Hint $Name
        if (Test-Path $candidate) { return $candidate }
    }
    $onPath = Get-Command $Name -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    if ($env:WIX) {
        $candidate = Join-Path (Join-Path $env:WIX "bin") $Name
        if (Test-Path $candidate) { return $candidate }
    }
    $roots = @("${env:ProgramFiles(x86)}", "$env:ProgramFiles") | Where-Object { $_ }
    foreach ($base in $roots) {
        $found = Get-ChildItem -Path $base -Filter $Name -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match 'WiX Toolset' } | Select-Object -First 1
        if ($found) { return $found.FullName }
    }
    throw "Could not find $Name. Install WiX Toolset v3 or pass -WixBin."
}

# The license pane needs RTF, so wrap the plain-text LICENSE.
function Write-LicenseRtf {
    param([string]$Source, [string]$Destination)
    $text = (Get-Content -Raw -Path $Source) -replace '\\', '\\\\' -replace '\{', '\{' -replace '\}', '\}'
    $body = ($text -split "`r?`n") -join "\par`r`n"
    $rtf = "{\rtf1\ansi\deff0{\fonttbl{\f0\fnil\fcharset0 Segoe UI;}}\fs18`r`n$body`r`n}"
    Set-Content -Path $Destination -Value $rtf -Encoding ASCII
}

$version = Resolve-ToreVersion -Explicit $Version
$msiVersion = Get-MsiVersion -Version $version
Write-Host "Packaging version $version (MSI ProductVersion $msiVersion)"

$target = Join-Path $root "target\release"
$dist = Join-Path $root "dist"
$stage = Join-Path $dist "stage\windows"

foreach ($binary in @("tore-app.exe", "tore-extract.exe")) {
    if (-not (Test-Path (Join-Path $target $binary))) {
        throw "Missing $target\$binary. Run: cargo build --release --locked -p tore-app -p tore-extract"
    }
}

if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
New-Item -ItemType Directory -Force -Path $dist | Out-Null

Copy-Item (Join-Path $target "tore-app.exe") (Join-Path $stage "tore-app.exe")
Copy-Item (Join-Path $target "tore-extract.exe") (Join-Path $stage "tore-extract.exe")
Copy-Item (Join-Path $root "LICENSE") (Join-Path $stage "LICENSE")
Copy-Item (Join-Path $root "THIRD_PARTY_NOTICES.md") (Join-Path $stage "THIRD_PARTY_NOTICES.md")
Copy-Item (Join-Path $root "README.md") (Join-Path $stage "README.md")
# The committed icon. It is installed beside the executable, used for both
# shortcuts, and shown in Settings, Installed apps.
Copy-Item (Join-Path $root "crates\tore-app\assets\icon\tore.ico") (Join-Path $stage "tore.ico")
Write-LicenseRtf -Source (Join-Path $root "LICENSE") -Destination (Join-Path $stage "LICENSE.rtf")

Write-Host "Checking the staged directory for retail data"
& python (Join-Path $root "tools\check_assets.py") $stage
if ($LASTEXITCODE -ne 0) { throw "Asset check failed on the staged directory." }

$candle = Find-WixTool -Name "candle.exe" -Hint $WixBin
$light = Find-WixTool -Name "light.exe" -Hint $WixBin
Write-Host "Using $candle"

$objDir = Join-Path $dist "stage\windows-wix"
if (Test-Path $objDir) { Remove-Item -Recurse -Force $objDir }
New-Item -ItemType Directory -Force -Path $objDir | Out-Null

$wixobj = Join-Path $objDir "tore.wixobj"
& $candle -nologo -arch x64 "-dVersion=$msiVersion" "-dStageDir=$stage" `
    -ext WixUIExtension -ext WixUtilExtension -out $wixobj (Join-Path $here "tore.wxs")
if ($LASTEXITCODE -ne 0) { throw "candle failed." }

$msi = Join-Path $dist "T.O.R.E-Fighters-$version-windows-x86_64.msi"
if (Test-Path $msi) { Remove-Item -Force $msi }
# ICE61 is the only suppression: it rejects AllowSameVersionUpgrades, which
# we want so that rebuilding the same version replaces the install instead of
# stacking a second copy. Every other validation check stays on.
& $light -nologo -ext WixUIExtension -ext WixUtilExtension -sice:ICE61 -out $msi $wixobj
if ($LASTEXITCODE -ne 0) { throw "light failed." }

Write-Host "Wrote $msi"
& python (Join-Path $root "tools\check_assets.py") $msi
if ($LASTEXITCODE -ne 0) { throw "Asset check failed on the MSI." }

Write-Host "Windows packaging complete for version $version"
