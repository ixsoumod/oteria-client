@echo off
rem Oteria: opens the TEST install made by tools\oteria\dist_test.ps1 (scratch folders, local
rem update source), never the real one. First time: the setup; then the installed launcher.
rem   tools\oteria\dist_try.cmd [the -Out folder of dist_test.ps1]
setlocal
set "ROOT=%~dp0..\.."
set "OUT=%~1"
if "%OUT%"=="" set "OUT=%ROOT%\.local\oteria-dist-test"
set "OTERIA_HOME=%OUT%\home"
set "OTERIA_GAME_DIR=%OUT%\game"
set "OTERIA_RESOURCES=%ROOT%\.local\horizon-client\26.3"
set "OTERIA_NO_SHORTCUTS=1"
set "HORIZON_NO_AUDIO="
if not exist "%OUT%\source\manifest.json" (
    echo Pas de source de test : lance d'abord tools\oteria\dist_test.ps1 -Step source
    exit /b 1
)
if not exist "%OTERIA_HOME%\OteriaLauncher.exe" (
    start "" "%OUT%\oteria-launcher-setup.exe" --source "%OUT%\source"
) else (
    start "" "%OTERIA_HOME%\OteriaLauncher.exe"
)
