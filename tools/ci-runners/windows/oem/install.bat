@echo off
rem dockurr/windows runs this once, as an administrator, at the first logon.
rem It only opens SSH; tools/ci-runners/windows/setup.ps1 does the rest.
powershell -NoProfile -ExecutionPolicy Bypass -File C:\OEM\setup-ssh.ps1 > C:\OEM\setup-ssh.log 2>&1
