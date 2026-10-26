# 校验并复制本地抠图模型及许可，不包含用户图片。
param([Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
$source = Join-Path (Split-Path -Parent $PSScriptRoot) 'resources/models/background-removal'
$manifest = Get-Content -LiteralPath (Join-Path $source 'manifest.json') -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath (Join-Path $source 'model.onnx') -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.sha256) { throw '抠图模型校验失败' }
$files = @('model.onnx', 'manifest.json', 'LICENSE-U2NET', 'LICENSE-rembg', 'NOTICE.md')
foreach ($file in $files) { if (-not (Test-Path -LiteralPath (Join-Path $source $file) -PathType Leaf)) { throw "缺少抠图资源：$file" } }
$target = Join-Path ([IO.Path]::GetFullPath($Destination)) 'resources/models/background-removal'
New-Item -ItemType Directory -Path $target -Force | Out-Null
foreach ($file in $files) { Copy-Item -LiteralPath (Join-Path $source $file) -Destination $target -Force }
Write-Output "抠图资源已复制至 $target"
