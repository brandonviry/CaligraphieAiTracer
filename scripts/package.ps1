# ─────────────────────────────────────────────────────────────────────────────
# package.ps1 - Script de packaging reproductible pour CaligraphieAiTracer
#
# Usage :
#   .\scripts\package.ps1
#   .\scripts\package.ps1 -Version "1.2.0"
#   .\scripts\package.ps1 -SkipTests
#
# Produit : dist\CaligraphieAiTracer-<version>\   (dossier pret a distribuer)
#           dist\CaligraphieAiTracer-<version>.zip (archive compressee)
# ─────────────────────────────────────────────────────────────────────────────

param(
    [string]$Version = "",
    [switch]$SkipTests
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Continue"

function Info  { param($msg) Write-Host "  $msg" -ForegroundColor Cyan }
function Ok    { param($msg) Write-Host "  OK $msg" -ForegroundColor Green }
function Fail  { param($msg) Write-Host "  FAIL $msg" -ForegroundColor Red; exit 1 }
function Run   { param($cmd) Invoke-Expression $cmd; return $LASTEXITCODE }
function Title { param($msg) Write-Host "`n=== $msg ===" -ForegroundColor Yellow }

# ── Detecter la version depuis Cargo.toml ────────────────────────────────────

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

if ($Version -eq "") {
    $match = Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"(.+)"'
    $Version = $match.Matches[0].Groups[1].Value
}

$appName  = "CaligraphieAiTracer"
$distName = "$appName-$Version"
$distDir  = Join-Path $root "dist\$distName"
$zipPath  = Join-Path $root "dist\$distName.zip"

Title "Packaging $appName v$Version"
Info "Racine  : $root"
Info "Dossier : $distDir"
Info "Archive : $zipPath"

# ── 1. Tests ─────────────────────────────────────────────────────────────────

if (-not $SkipTests) {
    Title "Etape 1 - Tests"
    & cargo test --all
    if ($LASTEXITCODE -ne 0) { Fail "Les tests ont echoue. Packaging annule." }
    Ok "Tous les tests passent."
} else {
    Info "Tests ignores (-SkipTests)."
}

# ── 2. Build release propre ──────────────────────────────────────────────────

Title "Etape 2 - Build release"
Info "cargo build --release ..."
& cargo build --release
if ($LASTEXITCODE -ne 0) { Fail "Le build a echoue." }

$exeSrc = Join-Path $root "target\release\calligraphie_ai_tracer.exe"
if (-not (Test-Path $exeSrc)) { Fail "Binaire introuvable : $exeSrc" }
$exeSize = [math]::Round((Get-Item $exeSrc).Length / 1MB, 1)
Ok "Build termine - calligraphie_ai_tracer.exe ($exeSize Mo)"

# ── 3. Preparer le dossier de distribution ───────────────────────────────────

Title "Etape 3 - Preparation du dossier dist"

if (Test-Path $distDir) {
    Remove-Item $distDir -Recurse -Force
    Info "Ancien dossier supprime."
}
New-Item -ItemType Directory -Path $distDir              | Out-Null
New-Item -ItemType Directory -Path "$distDir\assets\fonts"   | Out-Null
New-Item -ItemType Directory -Path "$distDir\assets\brushes" | Out-Null
New-Item -ItemType Directory -Path "$distDir\scripts"        | Out-Null

# Executable (renomme proprement)
Copy-Item $exeSrc "$distDir\$appName.exe"
Ok "Executable copie -> $appName.exe"

# config.toml
Copy-Item "$root\config.toml" "$distDir\config.toml"
Ok "config.toml copie"

# README
Copy-Item "$root\README.md" "$distDir\README.md"
Ok "README.md copie"

# Script Python
Copy-Item "$root\scripts\send_job.py" "$distDir\scripts\send_job.py"
Ok "send_job.py copie"

# Polices TTF deja en cache
$fontsCache = Join-Path $root "assets\fonts"
if (Test-Path $fontsCache) {
    $ttfFiles = @(Get-ChildItem $fontsCache -Filter "*.ttf")
    if ($ttfFiles.Count -gt 0) {
        Copy-Item "$fontsCache\*.ttf" "$distDir\assets\fonts\"
        Ok "$($ttfFiles.Count) police(s) TTF copiee(s)"
    } else {
        Info "Aucune police en cache - telechargement automatique au premier lancement."
    }
}

# Brosses PNG personnalisees
$brushesDir = Join-Path $root "assets\brushes"
if (Test-Path $brushesDir) {
    $pngFiles = @(Get-ChildItem $brushesDir -Filter "*.png")
    if ($pngFiles.Count -gt 0) {
        Copy-Item "$brushesDir\*.png" "$distDir\assets\brushes\"
        Ok "$($pngFiles.Count) brosse(s) PNG copiee(s)"
    }
}

# ── 4. Creer l'archive ZIP ───────────────────────────────────────────────────

Title "Etape 4 - Creation de l'archive ZIP"

if (Test-Path $zipPath) {
    Remove-Item $zipPath -Force
}

Compress-Archive -Path "$distDir\*" -DestinationPath $zipPath -CompressionLevel Optimal

if (-not (Test-Path $zipPath)) { Fail "Compression echouee." }
$zipSize = [math]::Round((Get-Item $zipPath).Length / 1MB, 1)
Ok "Archive creee : $distName.zip ($zipSize Mo)"

# ── 5. Resume ────────────────────────────────────────────────────────────────

Title "Resultat"
Write-Host ""
Write-Host "  Dossier : $distDir" -ForegroundColor White
Write-Host "  Archive : $zipPath" -ForegroundColor White
Write-Host ""
Write-Host "  Contenu du dossier :" -ForegroundColor Gray

$items = Get-ChildItem $distDir -Recurse -File
foreach ($item in $items) {
    $rel = $item.FullName.Substring($distDir.Length + 1)
    $sz  = [math]::Round($item.Length / 1KB, 0)
    Write-Host ("    {0,-52} {1,5} Ko" -f $rel, $sz) -ForegroundColor Gray
}

Write-Host ""
Ok "Packaging v$Version termine avec succes."
Write-Host ""
