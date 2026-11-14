@echo off
powershell.exe -NoProfile -File "%~dp0Play-Axiom.ps1" %*
exit /b %errorlevel%
