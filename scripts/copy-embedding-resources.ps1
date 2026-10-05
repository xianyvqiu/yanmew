# 发布时复制已校验资源；本脚本不复制开发数据库、模型导出工具或凭据。
param([Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$resourceSource = Join-Path $projectRoot 'resources/models/bge-small-zh-v1.5'
$resourceManifest = Get-Content -LiteralPath (Join-Path $resourceSource 'manifest.json') -Raw | ConvertFrom-Json
foreach ($entry in $resourceManifest.files.PSObject.Properties) {
    if ([IO.Path]::GetFileName($entry.Name) -ne $entry.Name) { throw '资源清单不能包含目录跳转' }
    $resourceFile = Join-Path $resourceSource $entry.Name
    if ((Get-FileHash -LiteralPath $resourceFile -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.Value) {
        throw "资源校验失败：$($entry.Name)"
    }
}
$resourceTarget = Join-Path ([IO.Path]::GetFullPath($Destination)) 'resources/models/bge-small-zh-v1.5'
New-Item -ItemType Directory -Path $resourceTarget -Force | Out-Null
foreach ($entry in $resourceManifest.files.PSObject.Properties) {
    Copy-Item -LiteralPath (Join-Path $resourceSource $entry.Name) -Destination $resourceTarget -Force
}
Copy-Item -LiteralPath (Join-Path $resourceSource 'manifest.json') -Destination $resourceTarget -Force
Write-Output "Embedding 资源已复制至 $resourceTarget"
