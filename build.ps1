# Build all Aidoku sources and aggregate into public/
param(
    [string]$SourceName = "*"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PublicDir = Join-Path $ScriptDir "public"
$SourcesDir = Join-Path $PublicDir "sources"
$IconsDir = Join-Path $PublicDir "icons"

# Clean and create public directories
if (Test-Path $SourcesDir) { Remove-Item -Recurse -Force $SourcesDir }
if (Test-Path $IconsDir) { Remove-Item -Recurse -Force $IconsDir }
New-Item -ItemType Directory -Path $SourcesDir -Force | Out-Null
New-Item -ItemType Directory -Path $IconsDir -Force | Out-Null

# Build each source
$srcDirs = Get-ChildItem -Path (Join-Path $ScriptDir "src\rust") -Directory -Filter $SourceName
$sources = @()

foreach ($srcDir in $srcDirs) {
    $cargoToml = Join-Path $srcDir.FullName "Cargo.toml"
    if (Test-Path $cargoToml) {
        $srcName = $srcDir.Name
        Write-Host "Building $srcName..." -ForegroundColor Cyan
        
        Push-Location $srcDir.FullName
        
        # Read source info with UTF-8 encoding
        $sourceJsonPath = Join-Path $srcDir.FullName "res\source.json"
        $sourceJsonContent = [System.IO.File]::ReadAllText($sourceJsonPath, [System.Text.Encoding]::UTF8)
        $sourceJson = $sourceJsonContent | ConvertFrom-Json
        $sourceId = $sourceJson.info.id
        $sourceVersion = $sourceJson.info.version
        
        # Build
        cargo build --release --target wasm32-unknown-unknown
        
        # Create package
        $releaseDir = Join-Path $srcDir.FullName "target\wasm32-unknown-unknown\release"
        $payloadDir = Join-Path $releaseDir "Payload"
        New-Item -ItemType Directory -Path $payloadDir -Force | Out-Null
        
        Copy-Item (Join-Path $srcDir.FullName "res\*") $payloadDir -Force
        Copy-Item (Join-Path $releaseDir "*.wasm") (Join-Path $payloadDir "main.wasm") -Force
        
        # Create .aix package (zip)
        $packagePath = Join-Path $srcDir.FullName "package.aix"
        if (Test-Path $packagePath) { Remove-Item $packagePath -Force }
        Compress-Archive -Path (Join-Path $payloadDir "*") -DestinationPath $packagePath -Force
        
        # Copy to public
        $aixName = "$sourceId-v$sourceVersion.aix"
        $pngName = "$sourceId-v$sourceVersion.png"
        Copy-Item $packagePath (Join-Path $SourcesDir $aixName) -Force
        Copy-Item (Join-Path $srcDir.FullName "res\Icon.png") (Join-Path $IconsDir $pngName) -Force
        
        # Build languages array
        $langList = @()
        foreach ($lang in $sourceJson.info.languages) {
            if ($lang.code) {
                $langList += $lang.code
            } else {
                $langList += $lang
            }
        }
        
        # Get content rating
        $contentRating = if ($sourceJson.info.nsfw) { $sourceJson.info.nsfw } else { 0 }
        
        # Add to sources list
        $sources += @{
            id = $sourceId
            name = $sourceJson.info.name
            version = $sourceVersion
            iconURL = $pngName
            downloadURL = $aixName
            languages = $langList
            contentRating = $contentRating
            baseURL = $sourceJson.info.url
        }
        
        Write-Host "Built $srcName -> $aixName" -ForegroundColor Green
        
        Pop-Location
    }
}

# Generate index.json
$index = @{
    name = "Development Source List"
    sources = $sources
}

$utf8NoBom = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText(
    (Join-Path $PublicDir "index.json"),
    ($index | ConvertTo-Json -Depth 5),
    $utf8NoBom
)
[System.IO.File]::WriteAllText(
    (Join-Path $PublicDir "index.min.json"),
    ($index | ConvertTo-Json -Depth 5 -Compress),
    $utf8NoBom
)

Write-Host ""
Write-Host "Build complete! Generated index.json with $($sources.Count) source(s)." -ForegroundColor Green
