# Build all Aidoku sources using aidoku-cli
param(
    [string]$SourceName = "*"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "=== Building Aidoku Sources ===" -ForegroundColor Cyan

# Collect built packages
$packages = @()

# Build each source
$srcDirs = Get-ChildItem -Path (Join-Path $ScriptDir "src\rust") -Directory -Filter $SourceName

foreach ($srcDir in $srcDirs) {
    $cargoToml = Join-Path $srcDir.FullName "Cargo.toml"
    if (Test-Path $cargoToml) {
        $srcName = $srcDir.Name
        Write-Host ""
        Write-Host "Building $srcName..." -ForegroundColor Yellow
        
        Push-Location $srcDir.FullName
        
        # Use aidoku package to build and package the source
        aidoku package
        
        if (Test-Path "package.aix") {
            $packages += (Join-Path $srcDir.FullName "package.aix")
            Write-Host "Built $srcName successfully" -ForegroundColor Green
        } else {
            Write-Host "Failed to build $srcName" -ForegroundColor Red
        }
        
        Pop-Location
    }
}

# Build source list using aidoku build
if ($packages.Count -gt 0) {
    Write-Host ""
    Write-Host "=== Building Source List ===" -ForegroundColor Cyan
    
    $publicDir = Join-Path $ScriptDir "public"
    aidoku build -o $publicDir -n "Development Source List" @packages
    
    Write-Host ""
    Write-Host "Build complete! $($packages.Count) source(s) built." -ForegroundColor Green
    Write-Host "Source list at: $publicDir" -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "No sources were built." -ForegroundColor Red
}
