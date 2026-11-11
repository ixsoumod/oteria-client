@echo off
rem Oteria Client: rebuild for playing and testing, then start it with oteria.cmd.
rem The `fast` profile (Cargo.toml) is optimized as a release is, but without LTO, in 16 parts and
rem incremental: after the first build, a change rebuilds in a minute or two instead of the release
rem profile's quarter of an hour. rust-lld, shipped with Rust, links much faster than Microsoft's
rem link.exe. Published releases keep the release profile (tools\oteria\publish.ps1).
rem Close the game first: Windows won't let the build replace a running program.
setlocal
cd /d "%~dp0"
set "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld.exe"
cargo build --profile fast -p horizon-client
if errorlevel 1 (
    echo.
    echo La compilation a echoue. Si le jeu est ouvert, ferme-le et relance oteria-build.cmd.
    exit /b 1
)
call "%~dp0oteria.cmd" %*
