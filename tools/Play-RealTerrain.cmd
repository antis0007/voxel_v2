@echo off
powershell.exe -NoProfile -File "%~dp0Play-RealTerrain.ps1" %*
exit /b %errorlevel%
