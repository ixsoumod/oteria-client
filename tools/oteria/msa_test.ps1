<#
.SYNOPSIS
Tests Oteria's Microsoft sign-in step by step, to know whether Mojang has approved the Azure app.

.DESCRIPTION
  powershell -ExecutionPolicy Bypass -File tools\oteria\msa_test.ps1            # the full test: you sign in in the browser
  powershell -ExecutionPolicy Bypass -File tools\oteria\msa_test.ps1 -Check     # only the Azure app, no sign-in

The Azure app's client ID is the repository variable HORIZON_MSA_CLIENT_ID (the one publish.ps1 builds the
launcher with); -ClientId gives another. The program opens microsoft.com/link: type the code it prints and
sign in with the Microsoft account that owns Minecraft. No password goes through the program, and no token is
printed or kept. It ends with a summary; « Validation de Mojang : NON » (HTTP 403) means the approval is
still pending.
#>
param(
    [string]$ClientId = '',
    [switch]$Check
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$env:Path += ";$env:USERPROFILE\.cargo\bin;$env:ProgramFiles\GitHub CLI"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = 'rust-lld.exe'
if (-not $ClientId) {
    $ClientId = (gh variable get HORIZON_MSA_CLIENT_ID -R ixsoumod/oteria-client).Trim()
}
if (-not $ClientId) { throw "Pas d'identifiant d'application : donne -ClientId ou crée la variable HORIZON_MSA_CLIENT_ID." }
Push-Location "$Root\oteria-launcher"
# cmd keeps cargo's progress messages, which Windows PowerShell 5 would take for errors, out of the way.
try { cmd /c 'cargo build --release --bin msa-test >nul 2>nul' } finally { Pop-Location }
$exe = "$Root\oteria-launcher\target\release\msa-test.exe"
if (-not (Test-Path $exe)) { throw 'msa-test.exe ne compile pas : lance cargo build --release --bin msa-test dans oteria-launcher.' }
$argv = @('--client-id', $ClientId)
if ($Check) { $argv += '--check' }
& $exe @argv
exit $LASTEXITCODE
