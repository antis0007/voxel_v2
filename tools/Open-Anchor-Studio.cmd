@echo off
setlocal
pushd "%~dp0.."
cargo build --offline -p engine_worldgen --bin anchor_studio --profile release-fast --target-dir target/anchor-studio/compiler
if errorlevel 1 (
    popd
    pause
    exit /b 1
)
node tools/anchor-studio/server.mjs --open
if errorlevel 1 pause
popd
