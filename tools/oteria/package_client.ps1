<#
.SYNOPSIS
Packs Oteria Client for the launcher: oteria-client-<version>.zip, holding one folder oteria-client\ with
  Oteria.exe              the game (Horizon's horizon-client.exe, renamed)
  oteria-prepare.exe      prepares the game's resources from the player's own Minecraft: Java Edition 26.3
  exporters\, research-lock.json, mods\horizon\src\test\java, crates\horizon-prepare\src\exporters.rs
                          what the preparer checks its exporters against (their paths are part of its check)
  LICENSE, LISEZMOI.txt

.DESCRIPTION
  powershell -ExecutionPolicy Bypass -File tools\oteria\package_client.ps1 -Game target\fast -Version 0.1.0 -Out dist\oteria-beta
#>
param(
    [Parameter(Mandatory)][string]$Game,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Out
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Game = (Resolve-Path $Game).Path
New-Item -ItemType Directory $Out -Force | Out-Null
$Out = (Resolve-Path $Out).Path
$Stage = Join-Path $Out 'stage'
$Dir = Join-Path $Stage 'oteria-client'
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory $Dir | Out-Null

foreach ($f in 'horizon-client.exe', 'horizon-prepare.exe') {
    if (-not (Test-Path "$Game\$f")) { throw "$Game\$f est introuvable : compile le jeu d'abord" }
}
if (-not (Test-Path "$Root\exporters\manifest.json")) {
    throw "exporters\manifest.json est introuvable : lance d'abord horizon-prepare compile-exporters (voir tools\oteria\publish.ps1)"
}
Copy-Item "$Game\horizon-client.exe" "$Dir\Oteria.exe"
Copy-Item "$Game\horizon-prepare.exe" "$Dir\oteria-prepare.exe"
Copy-Item -Recurse "$Root\exporters" "$Dir\exporters"
# The preparer hashes these sources (crates/horizon-prepare/src/exporters.rs, exporters::key): they go as git has them.
$tar = "$env:SystemRoot\System32\tar.exe"
git -C $Root archive --format=tar -o "$Stage\sources.tar" HEAD research-lock.json LICENSE mods/horizon/src/test/java crates/horizon-prepare/src/exporters.rs
if ($LASTEXITCODE -ne 0) { throw 'git archive a échoué' }
& $tar -x -f "$Stage\sources.tar" -C $Dir
if ($LASTEXITCODE -ne 0) { throw 'tar -x a échoué' }
Remove-Item "$Stage\sources.tar"
$readme = @"
Oteria Client $Version
======================

Ne lance pas ce dossier à la main : le launcher Oteria l'installe, le met à jour et démarre le jeu.

Au premier lancement, le launcher prépare les ressources du jeu (textures, sons, langues) à partir de ta
propre copie de Minecraft: Java Edition 26.3 : il faut l'avoir installée et lancée une fois depuis le
launcher officiel de Minecraft. Oteria ne contient aucun fichier de Mojang.

Oteria Client est un fork d'Horizon (licence MIT, voir LICENSE). Ce n'est pas un produit officiel
Minecraft, ni approuvé par Mojang ou Microsoft.
"@
[IO.File]::WriteAllText("$Dir\LISEZMOI.txt", $readme, (New-Object Text.UTF8Encoding($true)))

$Zip = Join-Path $Out "oteria-client-$Version.zip"
if (Test-Path $Zip) { Remove-Item $Zip }
# Windows' tar (bsdtar) writes ZIPs with forward slashes; Git's tar, first on a Git Bash PATH, writes none.
& $tar -a -c -f $Zip -C $Stage 'oteria-client'
if ($LASTEXITCODE -ne 0) { throw 'La création du ZIP a échoué' }
Remove-Item -Recurse -Force $Stage
Write-Host "   $Zip ($([math]::Round((Get-Item $Zip).Length / 1MB)) Mo)"
