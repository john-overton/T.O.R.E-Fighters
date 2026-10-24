# First-logon step: OpenSSH server with the CI key, PowerShell as its shell,
# and no sleep. Everything else is installed over SSH by setup.ps1.
$ErrorActionPreference = 'Stop'
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
Set-Service -Name sshd -StartupType Automatic
Start-Service sshd
New-NetFirewallRule -Name 'tore-ci-sshd' -DisplayName 'OpenSSH (tore CI)' -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22 -ErrorAction SilentlyContinue
$keys = 'C:\ProgramData\ssh\administrators_authorized_keys'
Copy-Item C:\OEM\authorized_keys $keys -Force
icacls.exe $keys /inheritance:r /grant 'Administrators:F' /grant 'SYSTEM:F'
New-ItemProperty -Path 'HKLM:\SOFTWARE\OpenSSH' -Name DefaultShell -Value 'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe' -PropertyType String -Force
powercfg /change standby-timeout-ac 0
powercfg /change hibernate-timeout-ac 0
powercfg /change monitor-timeout-ac 0
