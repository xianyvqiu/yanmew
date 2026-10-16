# 构建并组装 Windows x64 便携版。仅复制程序资源，不复制运行数据。
param([switch]$SkipBuild, [switch]$PassThru)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (-not $SkipBuild) {
        & npm run tauri build -- --no-bundle
        if ($LASTEXITCODE -ne 0) { throw '正式程序构建失败，未组装便携版' }
    }
    $config = Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
    $executable = Join-Path $projectRoot 'src-tauri/target/release/yanwenzi.exe'
    if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw '缺少 release 可执行文件' }

    # 从开发工具正式可再分发目录取运行库，不复制 System32 中的系统文件。
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw '无法定位 Visual Studio Redistributable 目录' }
    $visualStudio = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or -not $visualStudio) { throw '未找到 MSVC 开发工具' }
    $redistRoot = Join-Path $visualStudio 'VC/Redist/MSVC'
    $runtimeDirectory = Get-ChildItem -LiteralPath $redistRoot -Directory |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+(\.\d+)?$' } |
        Sort-Object { [version]$_.Name } -Descending |
        ForEach-Object { Get-ChildItem -Path (Join-Path $_.FullName 'x64/Microsoft.VC*.CRT') -Directory -ErrorAction SilentlyContinue } |
        Select-Object -First 1
    if (-not $runtimeDirectory) { throw '缺少 Windows x64 C++ 运行库' }
    foreach ($required in @('msvcp140.dll', 'msvcp140_1.dll', 'vcruntime140.dll', 'vcruntime140_1.dll')) {
        if (-not (Test-Path -LiteralPath (Join-Path $runtimeDirectory.FullName $required) -PathType Leaf)) { throw "运行库缺少 $required" }
    }

    $releaseRoot = Join-Path $projectRoot ('my_doc/releases/' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
    if (Test-Path -LiteralPath $releaseRoot) { throw '发布目录已存在，请稍后重试，原产物不覆盖' }
    $portable = Join-Path $releaseRoot "YanMew-$($config.version)-windows-x64"
    New-Item -ItemType Directory -Path $portable | Out-Null
    Copy-Item -LiteralPath $executable -Destination $portable
    Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $portable
    Copy-Item -LiteralPath (Join-Path $projectRoot 'resources/portable-README.md') -Destination (Join-Path $portable '使用说明.md')
    & (Join-Path $PSScriptRoot 'copy-embedding-resources.ps1') -Destination $portable
    & (Join-Path $PSScriptRoot 'copy-plugin-resources.ps1') -Destination $portable
    & (Join-Path $PSScriptRoot 'copy-calendar-notices.ps1') -Destination $portable
    Get-ChildItem -LiteralPath $runtimeDirectory.FullName -Filter '*.dll' -File |
        ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $portable }

    # 保留本次依赖版本的许可文本；只使用本地已安装依赖，不安装新工具。
    $licenseRoot = Join-Path $portable 'licenses'
    New-Item -ItemType Directory -Path $licenseRoot | Out-Null
    $notices = [System.Collections.Generic.List[string]]::new()
    $notices.Add('# 第三方组件声明')
    $notices.Add('')
    $notices.Add('各组件保留其原许可证。模型和 ONNX Runtime 声明在 resources/models/，日历声明在 resources/calendar/。')
    $notices.Add('微软 C++ DLL 来自 Visual Studio 的 Windows x64 Redistributable CRT 目录，仅作为应用运行组件随包提供。')
    $notices.Add('')
    $notices.Add('## Rust 依赖')
    $metadataText = & cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1 --locked --offline --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw '读取依赖声明失败，未生成正式 ZIP' }
    $metadata = ($metadataText -join "`n") | ConvertFrom-Json
    $nodes = @{}
    foreach ($node in $metadata.resolve.nodes) { $nodes[$node.id] = $node }
    $included = [System.Collections.Generic.HashSet[string]]::new()
    $queue = [System.Collections.Generic.Queue[string]]::new()
    $queue.Enqueue($metadata.resolve.root)
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        if (-not $included.Add($id)) { continue }
        foreach ($dependency in $nodes[$id].deps) {
            if (@($dependency.dep_kinds | Where-Object { $null -eq $_.kind }).Count -gt 0) { $queue.Enqueue($dependency.pkg) }
        }
    }
    foreach ($package in ($metadata.packages | Sort-Object name,version)) {
        if ($package.id -eq $metadata.resolve.root -or -not $included.Contains($package.id)) { continue }
        $packageRoot = Split-Path -Parent $package.manifest_path
        $files = @(Get-ChildItem -LiteralPath $packageRoot -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)([._-]|$)' })
        if ($package.license_file) {
            $declared = Join-Path $packageRoot $package.license_file
            if (Test-Path -LiteralPath $declared -PathType Leaf) { $files += Get-Item -LiteralPath $declared }
        }
        $packageTarget = Join-Path $licenseRoot "rust/$($package.name)-$($package.version)"
        if ($files.Count -gt 0) {
            New-Item -ItemType Directory -Path $packageTarget -Force | Out-Null
            $files | Sort-Object FullName -Unique | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $packageTarget }
        }
        $notices.Add("- $($package.name) $($package.version)：$($package.license)")
    }
    $notices.Add('')
    $notices.Add('## 前端运行依赖')
    foreach ($name in @('react', 'react-dom', 'scheduler', '@tauri-apps/api')) {
        $packageRoot = Join-Path $projectRoot "node_modules/$name"
        $package = Get-Content -LiteralPath (Join-Path $packageRoot 'package.json') -Raw | ConvertFrom-Json
        $packageTarget = Join-Path $licenseRoot ('frontend/' + $name.Replace('@','').Replace('/','-'))
        New-Item -ItemType Directory -Path $packageTarget -Force | Out-Null
        Get-ChildItem -LiteralPath $packageRoot -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)([._-]|$)' } |
            ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $packageTarget }
        $notices.Add("- $name $($package.version)：$($package.license)")
    }
    $notices | Set-Content -LiteralPath (Join-Path $portable 'THIRD_PARTY_NOTICES.md') -Encoding UTF8

    if (Test-Path -LiteralPath (Join-Path $portable 'data')) { throw '发布目录不得包含用户数据' }
    $archive = Join-Path $releaseRoot '颜喵 YanMew-portable.zip'
    Compress-Archive -LiteralPath $portable -DestinationPath $archive -CompressionLevel Optimal
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath "$archive.sha256" -Encoding UTF8
    Write-Output "便携目录：$portable"
    Write-Output "便携 ZIP：$archive"
    Write-Output "SHA-256：$hash"
    if ($PassThru) { Write-Output ([pscustomobject]@{ ReleaseDirectory = $releaseRoot; PortableDirectory = $portable; Archive = $archive }) }
} finally {
    Pop-Location
}
