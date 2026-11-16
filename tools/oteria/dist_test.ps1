<#
.SYNOPSIS
Tests Oteria Client's distribution chain (oteria-launcher-setup.exe, OteriaLauncher.exe, updater.exe) with a LOCAL
update source, in scratch folders: never %LOCALAPPDATA%\Oteria nor %APPDATA%\.oteria.

.DESCRIPTION
  powershell -ExecutionPolicy Bypass -File tools\oteria\dist_test.ps1               # build, then the three cases, with screenshots
  powershell -ExecutionPolicy Bypass -File tools\oteria\dist_test.ps1 -Interactive  # build the source, then open the setup to try by hand
  powershell -ExecutionPolicy Bypass -File tools\oteria\dist_test.ps1 -Step client-update   # move the source to a newer client
  powershell -ExecutionPolicy Bypass -File tools\oteria\dist_test.ps1 -Step launcher-update # move the source to a newer launcher

The source (default .local\oteria-dist-test\source) holds manifest.json, two launchers (1.0.0 and 1.0.1, the second
built with OTERIA_LAUNCHER_VERSION) and two clients (0.1.0 and 0.2.0, zips of target\fast's game). The game's
prepared resources are the repository's own (.local\horizon-client\26.3, OTERIA_RESOURCES): nothing of Mojang's is
copied into the source.
#>
param(
    [string]$Out = '',
    [ValidateSet('all', 'source', 'client-update', 'launcher-update')][string]$Step = 'all',
    [switch]$Interactive,
    [switch]$NoBuild
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $Out) { $Out = Join-Path $Root '.local\oteria-dist-test' }
$L = Join-Path $Root 'oteria-launcher'
$Src = Join-Path $Out 'source'
$Shots = Join-Path $Out 'shots'
$env:Path += ";$env:USERPROFILE\.cargo\bin"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = 'rust-lld.exe'

function Run([string]$exe, [string[]]$argv) {
    & $exe @argv
    if ($LASTEXITCODE -ne 0) { throw "$exe $($argv -join ' ') failed (exit $LASTEXITCODE)" }
}
function Sha([string]$f) { (Get-FileHash -Algorithm SHA256 $f).Hash.ToLower() }
function Write-Utf8([string]$path, [string]$text) {
    [IO.File]::WriteAllText($path, $text, (New-Object Text.UTF8Encoding($false)))
}

# ----- The update source ----------------------------------------------------------------------------------------
function Write-Manifest([string]$launcher, [string]$client) {
    $lf = "launcher/$launcher/OteriaLauncher.exe"
    $uf = 'launcher/updater.exe'
    $cf = "client/oteria-client-$client.zip"
    $m = [ordered]@{
        launcher = [ordered]@{ version = $launcher; file = $lf; sha256 = (Sha "$Src\$lf"); size = (Get-Item "$Src\$lf").Length }
        updater  = [ordered]@{ version = '1.0.0'; file = $uf; sha256 = (Sha "$Src\$uf"); size = (Get-Item "$Src\$uf").Length }
        client   = [ordered]@{ version = $client; file = $cf; sha256 = (Sha "$Src\$cf"); size = (Get-Item "$Src\$cf").Length }
        news     = @(
            [ordered]@{ tag = 'Mise à jour'; date = '16/11/2026'; title = "Oteria $client est disponible"; body = 'Crystal et Anchor Optimizer, hitboxes en couleurs, compteurs de totems et de potions, Super Secret Settings.' },
            [ordered]@{ tag = 'PvP'; date = '15/11/2026'; title = 'Les couteaux riggés arrivent'; body = "Butterfly, karambit, push dagger : des couteaux animés en première personne, avec leur inspect sur la touche ²." },
            [ordered]@{ tag = 'Cosmétiques'; date = '14/11/2026'; title = 'Capes en tissu, ailes et auras'; body = 'La nouvelle page Cosmétiques : une cape qui bouge comme du tissu, des ailes et des auras, visibles dans le menu.' },
            [ordered]@{ tag = 'Oteria'; date = '12/11/2026'; title = 'Bienvenue sur le launcher Oteria'; body = "Le launcher se met à jour tout seul et télécharge le client pour toi : un clic sur Jouer et c'est parti." }
        )
        servers  = @(
            [ordered]@{ name = 'Oteria Test'; address = 'localhost:25565'; description = 'Le serveur Paper local de test' },
            [ordered]@{ name = 'Arène PvP'; address = 'localhost:25566'; description = 'Exemple de serveur partenaire' },
            [ordered]@{ name = 'Practice'; address = 'localhost:25567'; description = 'Exemple : duels et kits' }
        )
    }
    Write-Utf8 "$Src\manifest.json" ($m | ConvertTo-Json -Depth 5)
    Write-Host "   manifest.json : launcher $launcher, client $client"
}

function Build-Source {
    if (-not $NoBuild) {
        Write-Host '== Les trois programmes (launcher 1.0.0), puis un launcher 1.0.1 pour la mise à jour'
        Push-Location $L
        try {
            Remove-Item Env:OTERIA_LAUNCHER_VERSION -ErrorAction SilentlyContinue
            Run cargo @('build', '--release')
            $env:OTERIA_LAUNCHER_VERSION = '1.0.1'
            Run cargo @('build', '--release', '--target-dir', 'target\next', '--bin', 'OteriaLauncher')
        } finally { Remove-Item Env:OTERIA_LAUNCHER_VERSION -ErrorAction SilentlyContinue; Pop-Location }
    }
    Write-Host "== La source de test : $Src"
    if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
    New-Item -ItemType Directory "$Src\launcher\1.0.0", "$Src\launcher\1.0.1", "$Src\client", $Shots -Force | Out-Null
    Copy-Item "$L\target\release\OteriaLauncher.exe" "$Src\launcher\1.0.0\"
    Copy-Item "$L\target\next\release\OteriaLauncher.exe" "$Src\launcher\1.0.1\"
    Copy-Item "$L\target\release\updater.exe" "$Src\launcher\"
    Copy-Item "$L\target\release\oteria-launcher-setup.exe" "$Out\"
    $game = Join-Path $Root 'target\fast'
    if (-not (Test-Path "$game\horizon-client.exe")) { $game = Join-Path $Root 'target\release' }
    & "$PSScriptRoot\package_client.ps1" -Game $game -Version '0.1.0' -Out "$Src\client"
    Copy-Item "$Src\client\oteria-client-0.1.0.zip" "$Src\client\oteria-client-0.2.0.zip"
    Write-Manifest '1.0.0' '0.1.0'
}

# ----- Scratch folders for every program started here -----------------------------------------------------------
$env:OTERIA_HOME = Join-Path $Out 'home'
$env:OTERIA_GAME_DIR = Join-Path $Out 'game'
$env:OTERIA_NO_SHORTCUTS = '1'
$env:OTERIA_RESOURCES = Join-Path $Root '.local\horizon-client\26.3'
$env:HORIZON_NO_AUDIO = '1'
Remove-Item Env:OTERIA_UPDATE_SOURCE -ErrorAction SilentlyContinue

function Launcher([hashtable]$vars, [int]$timeout = 90) {
    foreach ($k in $vars.Keys) { Set-Item "Env:$k" $vars[$k] }
    try {
        $p = Start-Process -FilePath "$env:OTERIA_HOME\OteriaLauncher.exe" -PassThru
        if (-not $p.WaitForExit($timeout * 1000)) { $p.Kill(); throw 'Le launcher ne s''est pas fermé à temps' }
    } finally { foreach ($k in $vars.Keys) { Remove-Item "Env:$k" -ErrorAction SilentlyContinue } }
}
function Stop-TestGame {
    # Only the game this test started: its pid is in the scratch launcher.log.
    Get-Content -Encoding UTF8 "$env:OTERIA_HOME\launcher.log" | Select-String 'jeu lancé \(pid (\d+)\)' | ForEach-Object {
        $id = [int]$_.Matches[0].Groups[1].Value
        Stop-Process -Id $id -ErrorAction SilentlyContinue
    }
}
function Expect([bool]$ok, [string]$what) {
    if ($ok) { Write-Host "   OK  $what" -ForegroundColor Green } else { Write-Host "   ÉCHEC  $what" -ForegroundColor Red; $script:failed = $true }
}
function State { Get-Content "$env:OTERIA_HOME\state.json" -Raw | ConvertFrom-Json }

if ($Step -eq 'client-update') { Write-Manifest ((State).launcher_version) '0.2.0'; exit 0 }
if ($Step -eq 'launcher-update') { Write-Manifest '1.0.1' ((State).client_version); exit 0 }

Build-Source
if ($Step -eq 'source') { exit 0 }

if ($Interactive) {
    Write-Host ''
    Write-Host "Source prête. Installe avec la fenêtre qui s'ouvre (tout va dans $Out)."
    Write-Host 'Puis, pour tester les mises à jour, relance le launcher après :'
    Write-Host "  powershell -ExecutionPolicy Bypass -File tools\oteria\dist_test.ps1 -Out `"$Out`" -Step client-update"
    Write-Host "  powershell -ExecutionPolicy Bypass -File tools\oteria\dist_test.ps1 -Out `"$Out`" -Step launcher-update"
    Write-Host 'Les variables OTERIA_HOME / OTERIA_GAME_DIR / OTERIA_RESOURCES de ce script ne sont posées que pour lui :'
    Write-Host "relance le launcher avec tools\oteria\dist_try.cmd `"$Out`"."
    Start-Process -FilePath "$Out\oteria-launcher-setup.exe" -ArgumentList '--source', "`"$Src`""
    exit 0
}

$failed = $false
Write-Host '== 1. Première installation'
$env:OTERIA_SETUP_SHOT = "$Shots\01-setup.png"
(Start-Process "$Out\oteria-launcher-setup.exe" -ArgumentList '--source', "`"$Src`"" -PassThru).WaitForExit(30000) | Out-Null
$env:OTERIA_SETUP_SHOT = "$Shots\02-setup-done.png"; $env:OTERIA_SETUP_SHOT_AT = 'done'
(Start-Process "$Out\oteria-launcher-setup.exe" -ArgumentList '--source', "`"$Src`"", '--silent' -PassThru).WaitForExit(60000) | Out-Null
Remove-Item Env:OTERIA_SETUP_SHOT, Env:OTERIA_SETUP_SHOT_AT
Expect (Test-Path "$env:OTERIA_HOME\OteriaLauncher.exe") 'le setup a installé le launcher'
Expect (Test-Path "$env:OTERIA_HOME\updater.exe") 'le setup a installé updater.exe'
Expect ((State).launcher_version -eq '1.0.0') 'state.json : launcher 1.0.0'

Launcher @{ OTERIA_LAUNCHER_SHOT = "$Shots\03-launcher-telecharger.png" }
Launcher @{ OTERIA_LAUNCHER_SHOT = "$Shots\04-launcher-telechargement.png"; OTERIA_LAUNCHER_SHOT_AT = 'downloading'; OTERIA_LAUNCHER_AUTO = 'download'; OTERIA_TEST_SLOW_FETCH = '60' }
# The download carried on in the updater after the picture: wait for it.
for ($i = 0; $i -lt 60 -and -not (State).client_version; $i++) { Start-Sleep -Milliseconds 500 }
Expect ((State).client_version -eq '0.1.0') 'le client 0.1.0 est installé'
Launcher @{ OTERIA_LAUNCHER_SHOT = "$Shots\05-launcher-jouer.png" }
Launcher @{ OTERIA_LAUNCHER_SHOT = "$Shots\06-launcher-lancement.png"; OTERIA_LAUNCHER_SHOT_AT = 'running'; OTERIA_LAUNCHER_AUTO = 'play' }
$line = Get-Content -Encoding UTF8 "$env:OTERIA_HOME\launcher.log" | Select-String 'jeu lancé \(pid \d+\) en (\d+) ms' | Select-Object -Last 1
$ms = if ($line) { [int]$line.Matches[0].Groups[1].Value } else { -1 }
Expect ($ms -ge 0 -and $ms -lt 2000) "le jeu démarre $ms ms après le clic sur Jouer"
Start-Sleep -Seconds 4
$alive = $false
Get-Content -Encoding UTF8 "$env:OTERIA_HOME\launcher.log" | Select-String 'jeu lancé \(pid (\d+)\)' | Select-Object -Last 1 | ForEach-Object {
    $alive = [bool](Get-Process -Id ([int]$_.Matches[0].Groups[1].Value) -ErrorAction SilentlyContinue)
}
Expect $alive 'le jeu tourne encore 4 s plus tard'
Stop-TestGame

Write-Host '== 2. Mise à jour du client (0.1.0 -> 0.2.0)'
Write-Manifest '1.0.0' '0.2.0'
Launcher @{ OTERIA_LAUNCHER_SHOT = "$Shots\07-launcher-maj-client.png" }
Launcher @{ OTERIA_LAUNCHER_SHOT = "$Shots\08-launcher-maj-client-faite.png"; OTERIA_LAUNCHER_AUTO = 'download' }
Expect ((State).client_version -eq '0.2.0') 'le client 0.2.0 remplace le 0.1.0'
Expect (-not (Test-Path "$env:OTERIA_HOME\clients\0.1.0")) "l'ancien client est supprimé"

Write-Host '== 3. Mise à jour du launcher (1.0.0 -> 1.0.1)'
Write-Manifest '1.0.1' '0.2.0'
$env:OTERIA_LAUNCHER_SHOT_AFTER_UPDATE = "$Shots\10-launcher-apres-maj.png"
Launcher @{ OTERIA_LAUNCHER_SHOT = "$Shots\09-launcher-maj-launcher.png"; OTERIA_LAUNCHER_SHOT_AT = 'self-update' }
for ($i = 0; $i -lt 60 -and -not (Test-Path "$Shots\10-launcher-apres-maj.png"); $i++) { Start-Sleep -Milliseconds 500 }
Remove-Item Env:OTERIA_LAUNCHER_SHOT_AFTER_UPDATE
Start-Sleep -Seconds 1
Expect ((State).launcher_version -eq '1.0.1') 'state.json : launcher 1.0.1'
Expect ((Sha "$env:OTERIA_HOME\OteriaLauncher.exe") -eq (Sha "$Src\launcher\1.0.1\OteriaLauncher.exe")) 'OteriaLauncher.exe est le 1.0.1'
Expect (Test-Path "$Shots\10-launcher-apres-maj.png") 'le launcher a redémarré tout seul'

Write-Host '== Pages'
foreach ($page in 'mods', 'news', 'settings') {
    Launcher @{ OTERIA_LAUNCHER_SHOT = "$Shots\11-page-$page.png"; OTERIA_LAUNCHER_PAGE = $page }
}
Write-Host ''
Write-Host "Captures : $Shots"
Write-Host "Journaux : $env:OTERIA_HOME\launcher.log, $env:OTERIA_HOME\updater.log"
if ($failed) { Write-Host 'Au moins une vérification a échoué.' -ForegroundColor Red; exit 1 }
Write-Host 'Les trois cas sont bons.' -ForegroundColor Green
