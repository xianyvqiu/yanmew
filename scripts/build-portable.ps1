# 构建并组装 Windows x64 便携版。仅复制程序资源，不复制运行数据。
param([switch]$SkipBuild, [switch]$PassThru)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$previousBuild = $env:YANMEW_BUILD_ID
$buildId = Get-Date -Format 'yyyyMMdd-HHmmss'
$releaseStamp = $buildId
$buildRecord = Join-Path $projectRoot 'src-tauri/target/release/yanmew-build-id.json'
if ($SkipBuild) {
    if (-not (Test-Path -LiteralPath $buildRecord)) { throw '缺少已编译程序的构建编号，不能复用；请去掉SkipBuild重新构建' }
    $record = Get-Content -LiteralPath $buildRecord -Raw | ConvertFrom-Json
    $buildId = $record.buildId
    if ($buildId -notmatch '^\d{8}-\d{6}$') { throw '已编译程序的构建编号无效' }
    $existingExe = Join-Path $projectRoot 'src-tauri/target/release/yanwenzi.exe'
    if ((Get-FileHash -LiteralPath $existingExe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $record.sha256) { throw 'EXE与构建编号记录不一致，请重新构建' }
}
$env:YANMEW_BUILD_ID = $buildId
Push-Location $projectRoot
try {
    if (-not $SkipBuild) {
        & npm run tauri build -- --no-bundle
        if ($LASTEXITCODE -ne 0) { throw '正式程序构建失败，未组装便携版' }
        [ordered]@{buildId=$buildId;sha256=(Get-FileHash -LiteralPath (Join-Path $projectRoot 'src-tauri/target/release/yanwenzi.exe') -Algorithm SHA256).Hash.ToLowerInvariant()} | ConvertTo-Json | Set-Content -LiteralPath $buildRecord -Encoding utf8NoBOM
    }
    & cargo rustc --manifest-path src-tauri/Cargo.toml --bin yanmew-update --release --locked --offline -- -C target-feature=+crt-static
    if ($LASTEXITCODE -ne 0) { throw '更新助手构建失败，未组装便携版' }
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

    $releaseRoot = Join-Path $projectRoot ('releases/' + $releaseStamp)
    if (Test-Path -LiteralPath $releaseRoot) { throw '发布目录已存在，请稍后重试，原产物不覆盖' }
    $portable = Join-Path $releaseRoot "YanMew-$($config.version)-windows-x64"
    $publishRoot = Join-Path $releaseRoot 'github-release'
    New-Item -ItemType Directory -Path $portable | Out-Null
    New-Item -ItemType Directory -Path $publishRoot | Out-Null
    Copy-Item -LiteralPath $executable -Destination $portable
    $helper = Join-Path $projectRoot 'src-tauri/target/release/yanmew-update.exe'
    Copy-Item -LiteralPath $helper -Destination $portable
    Copy-Item -LiteralPath $helper -Destination (Join-Path $publishRoot 'YanMew-Update.exe')
    Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $portable
    Copy-Item -LiteralPath (Join-Path $projectRoot 'resources/portable-README.md') -Destination (Join-Path $portable '使用说明.md')
    & (Join-Path $PSScriptRoot 'copy-embedding-resources.ps1') -Destination $portable
    & (Join-Path $PSScriptRoot 'copy-background-resources.ps1') -Destination $portable
    & (Join-Path $PSScriptRoot 'copy-plugin-resources.ps1') -Destination $portable
    # 缺图时拒绝分发；四个预置人设及全部基础状态均须随包提供。
    $personaResources = Join-Path $projectRoot 'resources/personas'
    foreach ($persona in @('gpt','claude','gemini','deepseek')) {
        foreach ($pose in @('base','standby','click','hungry','eating','sleepy','bored','thinking','chat','craving','craving_favorite','eating_favorite','peek_left','peek_right','rest_sit','rest_lie','walk_left','walk_right','emotion_happy','emotion_angry','emotion_sad','emotion_shy','emotion_afraid','emotion_speechless','emotion_surprised','lifted','error','toss_up','toss_down','landing','wait_pet')) {
            if (-not (Test-Path -LiteralPath (Join-Path $personaResources "$persona/$pose.png") -PathType Leaf)) {
                throw "预置人设图片缺失：$persona/$pose.png"
            }
        }
    }
    Copy-Item -LiteralPath $personaResources -Destination (Join-Path $portable 'resources/personas') -Recurse
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
    # 从锁文件收集全部生产依赖，包含 Markdown 等间接依赖；开发工具不进入发布清单。
    $frontendPackages = node -e "const fs=require('node:fs');const lock=JSON.parse(fs.readFileSync(process.argv[1],'utf8'));console.log(JSON.stringify(Object.entries(lock.packages).filter(([name,p])=>name.startsWith('node_modules/')&&!p.dev&&!p.link).map(([name])=>name).sort()));" (Join-Path $projectRoot 'package-lock.json')
    if ($LASTEXITCODE -ne 0) { throw '读取前端生产依赖清单失败' }
    foreach ($entry in ($frontendPackages | ConvertFrom-Json)) {
        $packageRoot = Join-Path $projectRoot $entry
        $name = $entry.Substring('node_modules/'.Length)
        $package = Get-Content -LiteralPath (Join-Path $packageRoot 'package.json') -Raw | ConvertFrom-Json
        $packageTarget = Join-Path $licenseRoot ('frontend/' + $name.Replace('@','').Replace('/','-'))
        New-Item -ItemType Directory -Path $packageTarget -Force | Out-Null
        Get-ChildItem -LiteralPath $packageRoot -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)([._-]|$)' } |
            ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $packageTarget }
        $notices.Add("- $name $($package.version)：$($package.license)")
    }
    $notices | Set-Content -LiteralPath (Join-Path $portable 'THIRD_PARTY_NOTICES.md') -Encoding UTF8

    if (Test-Path -LiteralPath (Join-Path $portable 'data')) { throw '发布目录不得包含用户数据' }
    $files = @(Get-ChildItem -LiteralPath $portable -File -Recurse | Sort-Object FullName | ForEach-Object {
        [ordered]@{path=[IO.Path]::GetRelativePath($portable,$_.FullName).Replace('\','/');size=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    [ordered]@{schemaVersion=1;product='com.yanmew.pet';version=$config.version;buildId=$buildId;files=$files} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $portable 'yanmew-package.json') -Encoding utf8NoBOM
    $archive = Join-Path $publishRoot "YanMew-$($config.version)-windows-x64.zip"
    Compress-Archive -LiteralPath $portable -DestinationPath $archive -CompressionLevel Optimal
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath "$archive.sha256" -Encoding UTF8
    [ordered]@{schemaVersion=1;version=$config.version;buildId=$buildId;assetName=[IO.Path]::GetFileName($archive);size=(Get-Item -LiteralPath $archive).Length;sha256=$hash} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $publishRoot 'yanmew-update.json') -Encoding utf8NoBOM
    "$((Get-FileHash -LiteralPath $helper -Algorithm SHA256).Hash.ToLowerInvariant())  YanMew-Update.exe" | Set-Content -LiteralPath (Join-Path $publishRoot 'YanMew-Update.exe.sha256') -Encoding utf8NoBOM
    Write-Output "便携目录：$portable"
    Write-Output "便携 ZIP：$archive"
    Write-Output "SHA-256：$hash"
    if ($PassThru) { Write-Output ([pscustomobject]@{ ReleaseDirectory = $releaseRoot; PortableDirectory = $portable; Archive = $archive }) }
} finally {
    $env:YANMEW_BUILD_ID = $previousBuild
    Pop-Location
}
