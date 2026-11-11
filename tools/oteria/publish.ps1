<#
.SYNOPSIS
Builds, signs and publishes an Oteria release from this Windows PC, as Horizon's CI does on its own runners
(docs/release-flow.md): the game as the `nightly` pre-release, the launcher as `launcher@X.Y.Z`.

.DESCRIPTION
  powershell -ExecutionPolicy Bypass -File tools\oteria\publish.ps1 -What all -DryRun   # build, sign, check; publish nothing
  powershell -ExecutionPolicy Bypass -File tools\oteria\publish.ps1 -What game          # the game, as `nightly`
  powershell -ExecutionPolicy Bypass -File tools\oteria\publish.ps1 -What launcher      # the launcher, as launcher@X.Y.Z

Releases are built from HEAD, which must be committed and pushed to origin/main (a release names its commit).
They are signed with the private key in %USERPROFILE%\.oteria-release\release-signing.key (base64 PKCS#8, made by
`horizon-release-sign keygen`; its public key is in crates/horizon-release-sign/release-keys.txt). The key goes to
tools/ci/sign_release.sh through the environment only, for that one call, and is never printed.

The game is the plain horizon-client, not horizon-official: Horizon's in-game services (its ping, official server,
Discord presence and Play with Friends relay) are not in Oteria's builds. Windows x64 only for now.
#>
param(
    [ValidateSet('game', 'launcher', 'all')][string]$What = 'all',
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
$Repo = 'ixsoumod/oteria-client'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$KeyFile = Join-Path $env:USERPROFILE '.oteria-release\release-signing.key'
$GitBash = Join-Path $env:ProgramFiles 'Git\bin\bash.exe'
$Dot = [char]0x00B7  # the middle dot of a Nightly's title, which launchers parse strictly
$env:Path += ";$env:USERPROFILE\.cargo\bin;$env:ProgramFiles\GitHub CLI"
# Every gh call (ours and tools/ci/nightly.py's) goes to Oteria's repository, never to the `upstream` remote.
$env:GH_REPO = $Repo
$env:GITHUB_REPOSITORY = $Repo
Set-Location $Root

function Run([string]$exe, [string[]]$argv) {
    & $exe @argv
    if ($LASTEXITCODE -ne 0) { throw "$exe $($argv -join ' ') failed (exit $LASTEXITCODE)" }
}

function Sign([string]$tag, [string]$also, [string[]]$files) {
    if (-not (Test-Path $KeyFile)) { throw "No signing key at $KeyFile" }
    try {
        $env:RELEASE_SIGNING_KEY = (Get-Content $KeyFile -Raw).Trim()
        if ($also) { $env:ALSO_FOR = $also } else { Remove-Item Env:ALSO_FOR -ErrorAction SilentlyContinue }
        Run $GitBash (@('tools/ci/sign_release.sh', $tag) + $files)
    } finally {
        Remove-Item Env:RELEASE_SIGNING_KEY, Env:ALSO_FOR -ErrorAction SilentlyContinue
    }
}

function Verify([string]$tag, [string]$folder, [string]$signature) {
    $argv = @('run', '--locked', '--quiet', '-p', 'horizon-release-sign', '--', 'verify', '--tag', $tag, '--publishing')
    if ($signature) { $argv += @('--signature', $signature) }
    Run cargo ($argv + @($folder))
}

function Zip([string]$from, [string]$entry, [string]$to) {
    # Windows' tar (bsdtar) writes forward slashes in the archive, as 7-Zip does; PowerShell 5's
    # Compress-Archive would write backslashes, which unzip tools read as part of the file name.
    Run tar @('-a', '-c', '-f', $to, '-C', $from, $entry)
}

# ----- Preconditions -------------------------------------------------------------------------------------------
if (-not (Test-Path $GitBash)) { throw "Git Bash not found at $GitBash (sign_release.sh needs its sha256sum and openssl)" }
if (git status --porcelain) { throw 'The working tree has changes: commit them first (a release names its commit).' }
Run git @('fetch', '--quiet', 'origin', 'main')
$Sha = (git rev-parse HEAD).Trim()
git merge-base --is-ancestor $Sha origin/main
if ($LASTEXITCODE -ne 0) {
    if (-not $DryRun) { throw "HEAD ($($Sha.Substring(0,7))) is not on origin/main: push it first." }
    Write-Host "HEAD is not on origin/main yet: fine for a dry run, not for a release."
}
$Short = $Sha.Substring(0, 7)
$Date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd')
Write-Host "Oteria release from $Short ($Date)$(if ($DryRun) { ', dry run: nothing is published' })"

# ----- The game: the `nightly` pre-release ---------------------------------------------------------------------
if ($What -in 'game', 'all') {
    $Version = "nightly-$Date-$Short"
    $Dist = Join-Path $Root 'dist'
    if (Test-Path $Dist) { Remove-Item -Recurse -Force $Dist }
    New-Item -ItemType Directory $Dist | Out-Null

    Write-Host '== The Java exporters horizon-prepare ships (compiled here against the pinned jars)'
    Run cargo @('build', '--release', '--locked', '-p', 'horizon-prepare')
    $bundle = (& "$Root\target\release\horizon-prepare.exe" compile-exporters --root $Root | Select-Object -Last 1)
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path "$bundle\manifest.json")) { throw "compile-exporters gave no bundle ($bundle)" }
    if (Test-Path "$Root\exporters") { Remove-Item -Recurse -Force "$Root\exporters" }
    New-Item -ItemType Directory "$Root\exporters" | Out-Null
    Copy-Item -Recurse "$bundle\server", "$bundle\client", "$bundle\manifest.json" "$Root\exporters\"

    Write-Host "== The game, built as $Version"
    try {
        $env:HORIZON_NIGHTLY = "$Date $Short"
        Run cargo @('build', '--release', '--locked',
            '-p', 'horizon-client', '--bin', 'horizon-client',
            '-p', 'horizon-client-lib', '--bin', 'horizon-ctl',
            '-p', 'horizon-prepare', '--bin', 'horizon-prepare',
            '-p', 'horizon-server', '--bin', 'horizon-server')
    } finally { Remove-Item Env:HORIZON_NIGHTLY -ErrorAction SilentlyContinue }
    Run cargo @('run', '--locked', '--quiet', '-p', 'xtask', '--', 'package', 'target\release\horizon-client.exe', 'dist\horizon-windows-x64')
    Zip $Dist 'horizon-windows-x64' "$Dist\horizon-windows-x64.zip"
    Run git @('archive', '--format=tar.gz', "--prefix=oteria-$Short/", '-o', 'dist/horizon-source.tar.gz', $Sha)

    Write-Host '== Signed checksums, checked as launchers check them'
    Sign 'nightly' $Version @('dist/horizon-windows-x64.zip', 'dist/horizon-source.tar.gz')
    Verify 'nightly' 'dist' ''
    Verify $Version 'dist' 'SHA256SUMS.v2.sig'

    if (-not $DryRun) {
        Write-Host '== Publishing the `nightly` pre-release'
        # The current `nightly` becomes nightly-<its date>, kept a week; tonight's takes the tag.
        Run python @('tools/ci/nightly.py', 'rotate')
        $notes = Join-Path $Dist 'nightly-notes.md'
        @(
            "Oteria Client Nightly, commit $Short.",
            '',
            'Install it with the Oteria launcher (the launcher@ release): it downloads these game files and prepares',
            "them from your own copy of Minecraft: Java Edition. horizon-source.tar.gz is this commit's source.",
            '',
            'Oteria Client is a fork of Horizon (MIT). Not an official Minecraft product; not approved by or associated with Mojang or Microsoft.'
        ) | Set-Content -Encoding UTF8 $notes
        Run gh @('release', 'create', 'nightly', '--prerelease', '--latest=false', '--target', $Sha,
            '--title', "Nightly $Date $Dot $Short", '--notes-file', $notes,
            'dist/horizon-windows-x64.zip#Game files the launcher installs (Nightly): Windows (horizon-windows-x64.zip)',
            "dist/horizon-source.tar.gz#Source of this Nightly, commit $Short (horizon-source.tar.gz)",
            'dist/SHA256SUMS', 'dist/SHA256SUMS.sig', 'dist/SHA256SUMS.v2.sig')
        # Native mods build against a Nightly's exact commit: its source tag is kept for good.
        Run python @('tools/ci/nightly.py', 'keep-source', $Sha, $Date)
        Run python @('tools/ci/nightly.py', 'prune', '--days', '7')
    }
}

# ----- The launcher: launcher@X.Y.Z --------------------------------------------------------------------------------
if ($What -in 'launcher', 'all') {
    $line = (Get-Content "$Root\.github\release-line.txt" -Raw).Trim()
    $last = @(gh release list --limit 200 --json tagName --jq '.[].tagName') |
        Where-Object { $_ -match "^launcher@$([regex]::Escape($line))\.(\d+)$" } |
        ForEach-Object { [int]$Matches[1] } | Sort-Object | Select-Object -Last 1
    $Patch = if ($null -eq $last) { 0 } else { $last + 1 }
    $LVersion = "$line.$Patch"
    $LTag = "launcher@$LVersion"
    $LDist = Join-Path $Root 'dist\launcher'
    if (Test-Path $LDist) { Remove-Item -Recurse -Force $LDist }
    New-Item -ItemType Directory "$LDist\pkg" -Force | Out-Null

    Write-Host "== The launcher, built as $LTag"
    $clientId = (gh variable get HORIZON_MSA_CLIENT_ID).Trim()
    if (-not $clientId) { throw 'The repository variable HORIZON_MSA_CLIENT_ID is not set: the launcher could not sign in.' }
    try {
        $env:HORIZON_LAUNCHER_VERSION = $LVersion
        $env:HORIZON_LAUNCHER_TAG = $LTag
        $env:HORIZON_MSA_CLIENT_ID = $clientId
        Push-Location "$Root\launcher"
        try { Run cargo @('build', '--release', '--locked') } finally { Pop-Location }
    } finally {
        Remove-Item Env:HORIZON_LAUNCHER_VERSION, Env:HORIZON_LAUNCHER_TAG, Env:HORIZON_MSA_CLIENT_ID -ErrorAction SilentlyContinue
    }
    $exe = "$Root\launcher\target\release\horizon-launcher.exe"
    $said = (& $exe --version | Out-String).Trim()
    if ($said -notmatch [regex]::Escape($LVersion)) { throw "The launcher says '$said', not $LVersion" }
    # The ZIP is what the launcher updates itself from; the same binary named -setup opens the installer.
    Copy-Item $exe "$LDist\pkg\horizon-launcher.exe"
    Zip "$LDist\pkg" 'horizon-launcher.exe' "$LDist\horizon-launcher-windows-x64.zip"
    Copy-Item $exe "$LDist\horizon-launcher-setup.exe"
    Remove-Item -Recurse -Force "$LDist\pkg"

    Write-Host '== Signed checksums, checked as launchers check them'
    Sign $LTag '' @('dist/launcher/horizon-launcher-windows-x64.zip', 'dist/launcher/horizon-launcher-setup.exe')
    Verify $LTag 'dist/launcher' ''

    if (-not $DryRun) {
        Write-Host "== Publishing $LTag"
        Run gh @('release', 'create', $LTag, '--latest=false', '--target', $Sha, '--title', "Oteria Launcher $LVersion",
            '--notes', "Oteria launcher $LVersion (commit $Short). New players: download horizon-launcher-setup.exe and run it.",
            'dist/launcher/horizon-launcher-setup.exe#Oteria launcher setup for Windows (horizon-launcher-setup.exe)',
            'dist/launcher/horizon-launcher-windows-x64.zip#Launcher update for Windows (horizon-launcher-windows-x64.zip)',
            'dist/launcher/SHA256SUMS', 'dist/launcher/SHA256SUMS.sig')
    }
}
Write-Host "Done$(if ($DryRun) { ' (dry run): everything built, signed and checked; nothing published' })."
