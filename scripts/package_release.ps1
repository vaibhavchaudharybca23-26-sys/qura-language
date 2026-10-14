# Qura Language - Release Packaging Script for Windows x64
# Usage: powershell -ExecutionPolicy Bypass -File scripts/package_release.ps1 [-Version <version>]

[CmdletBinding()]
param(
    [string]$Version = "v0.1.0-alpha",
    [switch]$SkipBuild = $false
)

$ErrorActionPreference = "Stop"

# 1. Resolve repository root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = (Resolve-Path (Join-Path $ScriptDir "..")).Path
Set-Location -Path $RepoRoot

Write-Host "=================================================" -ForegroundColor Cyan
Write-Host " Qura Language - Packaging Windows x64 Release   " -ForegroundColor Cyan
Write-Host " Target Version: $Version                       " -ForegroundColor Cyan
Write-Host " Repo Root:      $RepoRoot                      " -ForegroundColor Cyan
Write-Host "=================================================" -ForegroundColor Cyan

# 2. Build release binary if not skipped
if (-not $SkipBuild) {
    Write-Host "[1/5] Building release compiler binary via cargo..." -ForegroundColor Yellow
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Cargo build failed with exit code $LASTEXITCODE."
        exit $LASTEXITCODE
    }
} else {
    Write-Host "[1/5] Skipping cargo build (using existing binary)..." -ForegroundColor Yellow
}

$ReleaseExe = Join-Path $RepoRoot "target\release\qura.exe"
if (-not (Test-Path $ReleaseExe)) {
    Write-Error "Release executable not found at: $ReleaseExe"
    exit 1
}

# 3. Prepare staging directories
$DistDir = Join-Path $RepoRoot "dist"
$StagingDir = Join-Path $DistDir "staging\qura-$Version-windows-x64"

Write-Host "[2/5] Setting up staging directory: $StagingDir" -ForegroundColor Yellow

if (Test-Path $StagingDir) {
    Remove-Item -Recurse -Force $StagingDir
}
New-Item -ItemType Directory -Force -Path $StagingDir | Out-Null

# 4. Copy distribution files
Write-Host "[3/5] Populating release artifacts..." -ForegroundColor Yellow

# Copy compiler binary
$BinDir = Join-Path $StagingDir "bin"
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
Copy-Item -Path $ReleaseExe -Destination (Join-Path $BinDir "qura.exe") -Force
Write-Host "  + bin/qura.exe"

# Copy documentation and licenses
Copy-Item -Path (Join-Path $RepoRoot "README.md") -Destination $StagingDir -Force
Write-Host "  + README.md"

if (Test-Path (Join-Path $RepoRoot "LICENSE")) {
    Copy-Item -Path (Join-Path $RepoRoot "LICENSE") -Destination $StagingDir -Force
    Write-Host "  + LICENSE"
}

if (Test-Path (Join-Path $RepoRoot "docs\QUICKSTART.md")) {
    Copy-Item -Path (Join-Path $RepoRoot "docs\QUICKSTART.md") -Destination $StagingDir -Force
    Write-Host "  + QUICKSTART.md"
}

# Copy examples
$ExamplesStaging = Join-Path $StagingDir "examples"
New-Item -ItemType Directory -Force -Path $ExamplesStaging | Out-Null
Get-ChildItem -Path (Join-Path $RepoRoot "examples") -Filter "*.qura" | ForEach-Object {
    Copy-Item -Path $_.FullName -Destination $ExamplesStaging -Force
    Write-Host "  + examples/$($_.Name)"
}

# Copy VS Code extension files
$VsCodeStaging = Join-Path $StagingDir "editors\vscode"
New-Item -ItemType Directory -Force -Path $VsCodeStaging | Out-Null
Copy-Item -Path (Join-Path $RepoRoot "editors\vscode\package.json") -Destination $VsCodeStaging -Force
Copy-Item -Path (Join-Path $RepoRoot "editors\vscode\language-configuration.json") -Destination $VsCodeStaging -Force
Copy-Item -Path (Join-Path $RepoRoot "editors\vscode\README.md") -Destination $VsCodeStaging -Force

$SyntaxesStaging = Join-Path $VsCodeStaging "syntaxes"
New-Item -ItemType Directory -Force -Path $SyntaxesStaging | Out-Null
Copy-Item -Path (Join-Path $RepoRoot "editors\vscode\syntaxes\qura.tmLanguage.json") -Destination $SyntaxesStaging -Force

if (Test-Path (Join-Path $RepoRoot "editors\vscode\LICENSE")) {
    Copy-Item -Path (Join-Path $RepoRoot "editors\vscode\LICENSE") -Destination $VsCodeStaging -Force
}

Get-ChildItem -Path (Join-Path $RepoRoot "editors\vscode") -Filter "*.vsix" | ForEach-Object {
    Copy-Item -Path $_.FullName -Destination $VsCodeStaging -Force
    Write-Host "  + editors/vscode/$($_.Name)"
}
Write-Host "  + editors/vscode (Syntax highlighting extension)"

# 5. Compress into ZIP archive
$ZipFileName = "qura-$Version-windows-x64.zip"
$ZipFilePath = Join-Path $DistDir $ZipFileName

Write-Host "[4/5] Creating release zip: $ZipFilePath" -ForegroundColor Yellow
if (Test-Path $ZipFilePath) {
    Remove-Item -Force $ZipFilePath
}

Compress-Archive -Path "$StagingDir\*" -DestinationPath $ZipFilePath -CompressionLevel Optimal -Force

# 6. Verify and output summary
Write-Host "[5/5] Verifying archive..." -ForegroundColor Green
if (Test-Path $ZipFilePath) {
    $ZipItem = Get-Item $ZipFilePath
    $SizeMB = [math]::Round($ZipItem.Length / 1MB, 2)
    $SizeKB = [math]::Round($ZipItem.Length / 1KB, 1)

    Write-Host "=================================================" -ForegroundColor Green
    Write-Host " Packaging Successful!                           " -ForegroundColor Green
    Write-Host " Archive: $ZipFilePath                           " -ForegroundColor Green
    Write-Host " Size:    $SizeMB MB ($SizeKB KB)                " -ForegroundColor Green
    Write-Host "=================================================" -ForegroundColor Green
} else {
    Write-Error "Failed to generate release zip archive."
    exit 1
}
