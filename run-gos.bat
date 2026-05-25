@echo off
:: run-gos.bat — double-click launcher for the GOS kernel under QEMU.
:: Forwards to the PowerShell launcher (run-gos.ps1) with the default
:: bypass-execution-policy flag so it works on a stock Windows install
:: without prior `Set-ExecutionPolicy` changes.

cd /d "%~dp0"
powershell.exe -ExecutionPolicy Bypass -NoProfile -File ".\run-gos.ps1" %*

pause
