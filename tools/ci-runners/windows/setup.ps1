# Turns the CI Windows VM into two GitHub Actions runners for the
# windows-2022 jobs (64-bit and 32-bit), with the tools the hosted
# windows-2022 image gives those jobs: Git for Windows (its bash), PowerShell
# 7, Visual Studio 2022 Build Tools with the C++ workload and Windows SDK,
# WiX 3.14, and rustup with the pinned toolchain. Run over SSH as the VM's
# administrator (docs/CI-RUNNERS.md):
#
#   setup.ps1 -Token <registration token> -Password <the runner account's password>
#
# Safe to run again: installed tools are skipped, configured runners kept.
param(
    [Parameter(Mandatory)] [string] $Token,
    [Parameter(Mandatory)] [string] $Password,
    [string] $Url = 'https://github.com/john-overton/T.O.R.E-Fighters',
    [string] $RunnerVersion = '2.338.1',
    [string] $Toolchain = '1.91.1'
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$ci = 'C:\ci'
New-Item -ItemType Directory -Force $ci | Out-Null

# Defender scanning every object file makes cargo several times slower.
Add-MpPreference -ExclusionPath $ci
Add-MpPreference -ExclusionProcess 'cargo.exe', 'rustc.exe', 'link.exe', 'cl.exe'

# Paths over 260 characters occur in deep cargo target folders.
New-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -Value 1 -PropertyType DWord -Force | Out-Null

if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
    Set-ExecutionPolicy Bypass -Scope Process -Force
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor 3072
    Invoke-Expression ((New-Object Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
}
$choco = 'C:\ProgramData\chocolatey\bin\choco.exe'
& $choco install -y --no-progress git powershell-core wixtoolset
& $choco install -y --no-progress visualstudio2022buildtools --package-parameters '--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive --locale en-US'
& 'C:\Program Files\Git\cmd\git.exe' config --system core.longpaths true

# rustup in C:\ci for every account, the runner services included.
[Environment]::SetEnvironmentVariable('CARGO_HOME', "$ci\cargo", 'Machine')
[Environment]::SetEnvironmentVariable('RUSTUP_HOME', "$ci\rustup", 'Machine')
$env:CARGO_HOME = "$ci\cargo"
$env:RUSTUP_HOME = "$ci\rustup"
if (-not (Test-Path "$ci\cargo\bin\rustup.exe")) {
    Invoke-WebRequest https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile "$ci\rustup-init.exe"
    & "$ci\rustup-init.exe" -y --profile minimal --default-toolchain none --no-modify-path
}
# Installed here, before either runner starts, so two first jobs never
# install the same toolchain at once.
& "$ci\cargo\bin\rustup.exe" toolchain install $Toolchain --profile minimal --component rustfmt --component clippy --target i686-pc-windows-msvc

# Git's bash first, as on the hosted image, then cargo.
$path = [Environment]::GetEnvironmentVariable('Path', 'Machine')
foreach ($dir in @("$ci\cargo\bin", 'C:\Program Files\Git\bin')) {
    if (($path -split ';') -notcontains $dir) { $path = "$dir;$path" }
}
[Environment]::SetEnvironmentVariable('Path', $path, 'Machine')
$env:Path = "$path;$env:Path"

$zip = "$ci\actions-runner-$RunnerVersion.zip"
if (-not (Test-Path $zip)) {
    Invoke-WebRequest "https://github.com/actions/runner/releases/download/v$RunnerVersion/actions-runner-win-x64-$RunnerVersion.zip" -OutFile $zip
}
foreach ($n in 1, 2) {
    $dir = "$ci\runner-$n"
    if (Test-Path "$dir\.runner") { continue }
    New-Item -ItemType Directory -Force $dir | Out-Null
    Expand-Archive -Force $zip $dir
    Push-Location $dir
    & .\config.cmd --unattended --replace --url $Url --token $Token `
        --name "tore-windows-$n" --labels 'tore-windows-2022' --work "$ci\work-$n" `
        --runasservice --windowslogonaccount '.\runner' --windowslogonpassword $Password
    Pop-Location
}
Get-Service actions.runner.* | Format-Table Name, Status
