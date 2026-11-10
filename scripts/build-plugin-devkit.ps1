param(
    [string]$Version = '1.0.1',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw '开发工具包版本须为正式 SemVer' }
    if (-not $SkipBuild) {
        & cargo build --release --locked --offline --manifest-path src-tauri/Cargo.toml --bin yanmew-plugin
        if ($LASTEXITCODE -ne 0) { throw '构建插件工具失败' }
    }
    $work = Join-Path $projectRoot '.test-temp/plugin-devkit-build'
    New-Item -ItemType Directory -Force -Path $work | Out-Null
    $metadataFile = Join-Path $work 'cargo-metadata.json'
    & cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1 --locked --offline --filter-platform x86_64-pc-windows-msvc | Set-Content -LiteralPath $metadataFile -Encoding UTF8
    if ($LASTEXITCODE -ne 0) { throw '读取依赖许可失败' }
    & python plugins/build/devkit.py --version $Version --metadata $metadataFile
    if ($LASTEXITCODE -ne 0) { throw '生成开发工具包失败' }
} finally { Pop-Location }
