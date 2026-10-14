# 复制内置离线日历的来源与许可证；数据表已经编译进程序，不复制开发源码。
param([Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$source = Join-Path $projectRoot 'resources/calendar'
$names = @('README.md', 'manifest.json', 'LUNAR_PYTHON_LICENSE')
foreach ($name in $names) {
    if (-not (Test-Path -LiteralPath (Join-Path $source $name) -PathType Leaf)) { throw "缺少日历声明：$name" }
}
$target = Join-Path ([IO.Path]::GetFullPath($Destination)) 'resources/calendar'
New-Item -ItemType Directory -Path $target -Force | Out-Null
foreach ($name in $names) {
    Copy-Item -LiteralPath (Join-Path $source $name) -Destination $target -Force
}
Write-Output "日历来源与许可证已复制至 $target"
