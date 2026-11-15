@echo off
rem Axiom Dev Panel: starts the local dev-tools server and opens it in the browser.
rem Keep this window open while you edit or playtest. Pass -NoBrowser to only print the URL.
title Axiom Dev Panel
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\Open-AxiomDev.ps1" %*
if errorlevel 1 pause
