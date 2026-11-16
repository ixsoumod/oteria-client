<#
.SYNOPSIS
Publishes Oteria's beta on GitHub Releases, where oteria-launcher-setup.exe and the launcher download it.

.DESCRIPTION
  powershell -ExecutionPolicy Bypass -File tools\oteria\publish_beta.ps1 -Launcher 1.0.0 -Client 0.1.0 -DryRun   # build and pack; publish nothing
  powershell -ExecutionPolicy Bypass -File tools\oteria\publish_beta.ps1 -Launcher 1.0.0 -Client 0.1.0           # publish both
  powershell -ExecutionPolicy Bypass -File tools\oteria\publish_beta.ps1 -Client 0.1.1                          # a new client only
  powershell -ExecutionPolicy Bypass -File tools\oteria\publish_beta.ps1 -Launcher 1.0.1                        # a new launcher only

What goes where, on ixsoumod/oteria-client:
  launcher-v<L>   OteriaLauncher.exe, updater.exe, oteria-launcher-setup.exe
  client-v<C>     oteria-client-<C>.zip (tools\oteria\package_client.ps1)
  beta            manifest.json (what every launcher reads on start, with absolute links to the two above)
                  and oteria-launcher-setup.exe: the link to give friends is always
                  https://github.com/ixsoumod/oteria-client/releases/download/beta/oteria-launcher-setup.exe

The manifest keeps the part not published this time from the beta release's current manifest. News and partner
servers come from tools\oteria\beta-news.json. The client is built with the `fast` profile unless -Profile release.
A dry run writes everything to dist\oteria-beta, with a manifest.local.json that dist_try.cmd-style tests can use.
#>
param(
    [string]$Launcher = '',
    [string]$Client = '',
    [ValidateSet('fast', 'release')][string]$Profile = 'fast',
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
if (-not $Launcher -and -not $Client) { throw 'Donne -Launcher X.Y.Z, -Client X.Y.Z, ou les deux.' }
$Repo = 'ixsoumod/oteria-client'
$Base = "https://github.com/$Repo/releases/download"
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Out = Join-Path $Root 'dist\oteria-beta'
$L = Join-Path $Root 'oteria-launcher'
$env:Path += ";$env:USERPROFILE\.cargo\bin;$env:ProgramFiles\GitHub CLI"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = 'rust-lld.exe'
$env:GH_REPO = $Repo

function Run([string]$exe, [string[]]$argv) {
    & $exe @argv
    if ($LASTEXITCODE -ne 0) { throw "$exe $($argv -join ' ') a échoué (code $LASTEXITCODE)" }
}
function Sha([string]$f) { (Get-FileHash -Algorithm SHA256 $f).Hash.ToLower() }
function Entry([string]$version, [string]$file, [string]$url) {
    [ordered]@{ version = $version; file = $url; sha256 = (Sha $file); size = (Get-Item $file).Length }
}

if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
New-Item -ItemType Directory "$Out\launcher", "$Out\client" -Force | Out-Null

# The current manifest, for the part not published now.
$current = $null
try {
    $current = Invoke-RestMethod -UseBasicParsing "$Base/beta/manifest.json"
} catch { Write-Host '   (pas encore de manifeste publié)' }

$manifest = [ordered]@{}
$local = [ordered]@{}

if ($Launcher) {
    Write-Host "== Le launcher $Launcher"
    Push-Location $L
    try {
        $env:OTERIA_LAUNCHER_VERSION = $Launcher
        Run cargo @('build', '--release')
    } finally { Remove-Item Env:OTERIA_LAUNCHER_VERSION -ErrorAction SilentlyContinue; Pop-Location }
    foreach ($f in 'OteriaLauncher.exe', 'updater.exe', 'oteria-launcher-setup.exe') { Copy-Item "$L\target\release\$f" "$Out\launcher\" }
    $tag = "launcher-v$Launcher"
    $manifest.launcher = Entry $Launcher "$Out\launcher\OteriaLauncher.exe" "$Base/$tag/OteriaLauncher.exe"
    $manifest.updater = Entry $Launcher "$Out\launcher\updater.exe" "$Base/$tag/updater.exe"
    $local.launcher = Entry $Launcher "$Out\launcher\OteriaLauncher.exe" 'launcher/OteriaLauncher.exe'
    $local.updater = Entry $Launcher "$Out\launcher\updater.exe" 'launcher/updater.exe'
} elseif ($current) {
    $manifest.launcher = $current.launcher; $manifest.updater = $current.updater
} else { throw "Aucun launcher publié : ajoute -Launcher" }

if ($Client) {
    Write-Host "== Le client $Client (profil $Profile)"
    $game = Join-Path $Root "target\$Profile"
    # A running game locks its exe: it is renamed, never closed (the build then writes a new one).
    foreach ($exe in 'horizon-client.exe', 'horizon-prepare.exe') {
        $p = Join-Path $game $exe
        if ((Test-Path $p) -and (Get-Process | Where-Object { $_.Path -eq $p })) {
            Move-Item -Force $p ($p -replace '\.exe$', '.old.exe')
        }
    }
    Run cargo @('build', '--profile', $Profile, '-p', 'horizon-client', '--bin', 'horizon-client', '-p', 'horizon-prepare', '--bin', 'horizon-prepare')
    & "$PSScriptRoot\package_client.ps1" -Game $game -Version $Client -Out "$Out\client"
    $zip = "$Out\client\oteria-client-$Client.zip"
    $manifest.client = Entry $Client $zip "$Base/client-v$Client/oteria-client-$Client.zip"
    $local.client = Entry $Client $zip "client/oteria-client-$Client.zip"
} elseif ($current) {
    $manifest.client = $current.client
} else { throw "Aucun client publié : ajoute -Client" }

$extra = Get-Content -Raw -Encoding UTF8 "$PSScriptRoot\beta-news.json" | ConvertFrom-Json
foreach ($m in $manifest, $local) { $m.news = @($extra.news); $m.servers = @($extra.servers) }
$utf8 = New-Object Text.UTF8Encoding($false)
[IO.File]::WriteAllText("$Out\manifest.json", ($manifest | ConvertTo-Json -Depth 6), $utf8)
if ($Launcher -and $Client) {
    [IO.File]::WriteAllText("$Out\manifest.local.json", ($local | ConvertTo-Json -Depth 6), $utf8)
}
Write-Host "== Manifeste : $Out\manifest.json"

if ($DryRun) {
    Write-Host 'Essai à blanc : rien n''est publié.'
    exit 0
}

Write-Host '== Publication sur GitHub'
# `gh release view` answers on stderr when a release is missing: PowerShell 5 must not take that for an error.
function Published([string]$tag) {
    cmd /c "gh release view $tag >nul 2>nul"
    return ($LASTEXITCODE -eq 0)
}
foreach ($tag in @($(if ($Launcher) { "launcher-v$Launcher" }), $(if ($Client) { "client-v$Client" })) | Where-Object { $_ }) {
    if (Published $tag) { throw "$tag est déjà publiée : change de numéro de version." }
}
if ($Launcher) {
    Run gh @('release', 'create', "launcher-v$Launcher", '--title', "Oteria Launcher $Launcher", '--notes', "Launcher Oteria $Launcher (bêta).",
        "$Out\launcher\OteriaLauncher.exe", "$Out\launcher\updater.exe", "$Out\launcher\oteria-launcher-setup.exe")
}
if ($Client) {
    Run gh @('release', 'create', "client-v$Client", '--prerelease', '--title', "Oteria Client $Client", '--notes', "Oteria Client $Client (bêta). Installé par le launcher Oteria.",
        "$Out\client\oteria-client-$Client.zip")
}
if (-not (Published 'beta')) {
    $notes = "Bêta d'Oteria Client.`n`nTélécharge **oteria-launcher-setup.exe** et lance-le : il installe le launcher, qui télécharge le jeu et se met à jour tout seul.`n`nIl faut posséder Minecraft: Java Edition : le launcher télécharge le jeu et Java depuis les serveurs de Mojang.`n`nOteria Client est un fork d'Horizon (MIT). Pas un produit officiel Minecraft, ni approuvé par Mojang ou Microsoft."
    Run gh @('release', 'create', 'beta', '--prerelease', '--title', 'Oteria Client (bêta)', '--notes', $notes)
}
$assets = @("$Out\manifest.json")
if ($Launcher) { $assets += "$Out\launcher\oteria-launcher-setup.exe" }
Run gh (@('release', 'upload', 'beta', '--clobber') + $assets)
Write-Host "Publié. Lien pour tes potes : $Base/beta/oteria-launcher-setup.exe"
