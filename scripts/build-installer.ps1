# 同一release程序同时支持便携与安装标记，资源组装复用现有脚本。
param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $outputs = @(& (Join-Path $PSScriptRoot 'build-portable.ps1') -SkipBuild:$SkipBuild -PassThru)
    $release = $outputs | Where-Object { $_ -is [pscustomobject] -and $_.PortableDirectory } | Select-Object -Last 1
    $outputs | Where-Object { $_ -is [string] } | Write-Output
    if (-not $release) { throw '未取得本次便携资源目录，停止安装打包' }
    $resources = [ordered]@{}
    foreach ($file in Get-ChildItem -LiteralPath $release.PortableDirectory -File -Recurse) {
        $relative = $file.FullName.Substring($release.PortableDirectory.Length + 1).Replace('\', '/')
        if ($relative -in @('yanwenzi.exe', '使用说明.md')) { continue }
        $resources[$file.FullName] = $relative
    }
    $resources[(Join-Path $projectRoot 'src-tauri/windows/yanmew-installed')] = 'yanmew-installed'
    $resources[(Join-Path $projectRoot 'resources/installer-README.md')] = '使用说明.md'
    $configuration = @{ bundle = @{ resources = $resources } }
    $generated = Join-Path $release.ReleaseDirectory 'tauri.installer.resources.json'
    $configuration | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $generated -Encoding UTF8
    & npm run tauri -- bundle --config src-tauri/tauri.installer.conf.json --config $generated --bundles nsis
    if ($LASTEXITCODE -ne 0) { throw '安装器构建失败，完整便携包仍保留' }
    $package = Get-ChildItem -LiteralPath (Join-Path $projectRoot 'src-tauri/target/release/bundle/nsis') -File -Filter '*-setup.exe' |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $package) { throw '没有生成NSIS安装包' }
    $version = (Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json).version
    $destination = Join-Path $release.ReleaseDirectory "YanMew-$version-setup.exe"
    Copy-Item -LiteralPath $package.FullName -Destination $destination
    $hash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($destination))" | Set-Content -LiteralPath "$destination.sha256" -Encoding UTF8
    Write-Output "安装向导：$destination"
    Write-Output "SHA-256：$hash"
} finally {
    Pop-Location
}
