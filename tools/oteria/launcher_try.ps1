<#
.SYNOPSIS
Opens the launcher you just built, to try the Microsoft sign-in, on a test install (never %LOCALAPPDATA%\Oteria).

.DESCRIPTION
  powershell -ExecutionPolicy Bypass -File tools\oteria\launcher_try.ps1

It builds oteria-launcher, then starts target\release\OteriaLauncher.exe on the test install of
.local\oteria-public-test (made by installing the beta from GitHub into a scratch folder), with the Azure app's
client ID read from the repository variable HORIZON_MSA_CLIENT_ID. Use Accounts > Microsoft account > Se connecter.
The refresh token goes to Windows' Credential Manager (entry oteria-launcher); remove the account in the launcher
to delete it. -InstallDir gives another test install folder (it holds home\ and game\).
#>
param([string]$InstallDir = '')
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$env:Path += ";$env:USERPROFILE\.cargo\bin;$env:ProgramFiles\GitHub CLI"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = 'rust-lld.exe'
if (-not $InstallDir) { $InstallDir = Join-Path $Root '.local\oteria-public-test' }
if (-not (Test-Path "$InstallDir\home\state.json")) {
    throw "Pas d'installation de test dans $InstallDir : installe la bêta dans ce dossier (OTERIA_HOME=$InstallDir\home) avec oteria-launcher-setup.exe."
}
Push-Location "$Root\oteria-launcher"
try { cmd /c 'cargo build --release >nul 2>nul' } finally { Pop-Location }
$exe = "$Root\oteria-launcher\target\release\OteriaLauncher.exe"
if (-not (Test-Path $exe)) { throw 'Le launcher ne compile pas.' }
$env:OTERIA_HOME = "$InstallDir\home"
$env:OTERIA_GAME_DIR = "$InstallDir\game"
$env:OTERIA_MSA_CLIENT_ID = (gh variable get HORIZON_MSA_CLIENT_ID -R ixsoumod/oteria-client).Trim()
# This copy is not the installed one: it never replaces itself, and the updater beside it is the new one.
Start-Process $exe
Write-Host "Launcher ouvert sur $InstallDir (identifiant Azure lu depuis GitHub)."
