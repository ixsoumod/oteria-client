@echo off
rem Oteria Client: starts the release build with one fixed game folder, so video settings,
rem controls (options.txt, horizon-graphics.txt), worlds and caches survive every rebuild,
rem `git clean` and a fresh clone. Logs and crash reports go there too, apart from Horizon's.
setlocal
set "ROOT=%~dp0"
set "OTERIA_DIR=%APPDATA%\.oteria"
set "EXE=%ROOT%target\release\horizon-client.exe"

if not exist "%EXE%" (
    echo Oteria Client n'est pas compile. Lance d'abord : cargo build --release -p horizon-client
    exit /b 1
)

rem First start: carry over the settings and worlds of the earlier .local\play folder.
if not exist "%OTERIA_DIR%\options.txt" if exist "%ROOT%.local\play\options.txt" (
    echo Reprise des reglages et des mondes de .local\play vers %OTERIA_DIR%
    robocopy "%ROOT%.local\play" "%OTERIA_DIR%" /E /NFL /NDL /NJH /NJS /NP >nul
)
if not exist "%OTERIA_DIR%" mkdir "%OTERIA_DIR%"

rem HORIZON_HOME: logs, crash reports, telemetry consent and recordings (not in %APPDATA%\horizon).
set "HORIZON_HOME=%OTERIA_DIR%\home"
start "" /D "%ROOT%" "%EXE%" --game-dir "%OTERIA_DIR%" %*
