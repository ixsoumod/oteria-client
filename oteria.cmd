@echo off
rem Oteria Client: starts the game with one fixed game folder, so video settings, controls
rem (options.txt, horizon-graphics.txt), worlds and caches survive every rebuild, `git clean` and
rem a fresh clone. Logs and crash reports go there too, apart from Horizon's.
rem It starts the quick-to-rebuild `fast` build (oteria-build.cmd) when there is one, else the
rem release build.
setlocal
set "ROOT=%~dp0"
set "OTERIA_DIR=%APPDATA%\.oteria"
set "EXE=%ROOT%target\fast\horizon-client.exe"
if not exist "%EXE%" set "EXE=%ROOT%target\release\horizon-client.exe"

if not exist "%EXE%" (
    echo Oteria Client n'est pas compile. Lance d'abord : oteria-build.cmd
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
rem Player name on offline-mode servers (a --name given on the command line wins).
set "OTERIA_NAME=Ixsou_mod"
start "" /D "%ROOT%" "%EXE%" --game-dir "%OTERIA_DIR%" --name "%OTERIA_NAME%" %*
