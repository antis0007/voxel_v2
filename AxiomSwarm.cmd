@echo off
rem Starts the parity swarm and its supervisor UI; restarts it after a crash (results resume).
rem Pressing "Stop all" in the UI exits cleanly and ends this loop.  Usage: AxiomSwarm.cmd [--workers 5] [--port 4377]
cd /d "%~dp0"
start "" http://127.0.0.1:4377
:loop
node tools\axiom-dev\swarm.mjs %*
if errorlevel 1 (
  echo swarm exited with an error; restarting in 5 s
  timeout /t 5 /nobreak >nul
  goto loop
)
