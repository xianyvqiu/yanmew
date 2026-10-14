# 将已构建的四个插件与清单复制到便携版 exe 旁，不复制用户数据。
param([Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$source = Join-Path $projectRoot 'resources/plugins'
$cataloguePath = Join-Path $source 'bundled-games.json'
$catalogue = Get-Content -LiteralPath $cataloguePath -Raw | ConvertFrom-Json
if ($catalogue.api_version -ne '1.0.0') { throw '不支持的插件清单版本' }
foreach ($name in $catalogue.packages) {
    if ($name -notmatch '^[a-z0-9-]+\.yanmew-game$') { throw '清单文件名无效' }
    if (-not (Test-Path -LiteralPath (Join-Path $source $name) -PathType Leaf)) { throw "缺少插件包：$name，请先运行 npm run plugins:build" }
}
$target = Join-Path ([IO.Path]::GetFullPath($Destination)) 'resources/plugins'
New-Item -ItemType Directory -Path $target -Force | Out-Null
foreach ($name in $catalogue.packages) {
    Copy-Item -LiteralPath (Join-Path $source $name) -Destination $target -Force
}
Copy-Item -LiteralPath $cataloguePath -Destination $target -Force
Write-Output "小游戏资源已复制至 $target"
