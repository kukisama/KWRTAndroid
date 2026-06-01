<#
.SYNOPSIS
    KWRT Controller 一键打包脚本（仅生成 exe，不打 MSI/NSIS）。

.DESCRIPTION
    流程：
      1. 确保旧实例不在跑（停掉名为 kwrt-controller 的进程）。
      2. 在 src-tauri 下跑 cargo build --release。
      3. 把 exe 收纳到 .\release\<version>\ 目录，文件名带时间戳。
      4. 同时维护一个 .\release\latest\kwrt-controller.exe 软拷贝，方便快速运行。
      5. 可选 -Run：编完直接 Start-Process 拉起来。

.PARAMETER Run
    编译成功后自动启动 exe。

.PARAMETER Clean
    编译前先 cargo clean。

.EXAMPLE
    .\scripts\build-release.ps1
.EXAMPLE
    .\scripts\build-release.ps1 -Run
.EXAMPLE
    .\scripts\build-release.ps1 -Clean -Run
#>
[CmdletBinding()]
param(
    [switch]$Run,
    [switch]$Clean
)

$ErrorActionPreference = 'Stop'

# 1. 解析路径
$RepoRoot   = Split-Path -Parent $PSScriptRoot
$SrcTauri   = Join-Path $RepoRoot 'src-tauri'
$CargoToml  = Join-Path $SrcTauri 'Cargo.toml'
$ReleaseDir = Join-Path $RepoRoot 'release'
$LatestDir  = Join-Path $ReleaseDir 'latest'

if (-not (Test-Path $CargoToml)) {
    throw "未找到 $CargoToml，请确认在正确的工作区运行此脚本。"
}

# 2. 读版本号（从 Cargo.toml 第一处 version = "x.y.z"）
$cargoText = Get-Content $CargoToml -Raw
$verMatch  = [regex]::Match($cargoText, '(?m)^\s*version\s*=\s*"([^"]+)"')
if (-not $verMatch.Success) { throw '无法在 Cargo.toml 中解析 version。' }
$Version = $verMatch.Groups[1].Value
$Stamp   = Get-Date -Format 'yyyyMMdd-HHmmss'
Write-Host "==> 版本 $Version  时间戳 $Stamp" -ForegroundColor Cyan

# 3. 停掉可能在跑的旧实例（兼容 .exe 已加载场景）
Get-Process kwrt-controller -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host "==> 停止旧实例 PID=$($_.Id)" -ForegroundColor Yellow
    $_ | Stop-Process -Force
}

Push-Location $SrcTauri
try {
    if ($Clean) {
        Write-Host "==> cargo clean" -ForegroundColor Cyan
        cargo clean
        if ($LASTEXITCODE -ne 0) { throw "cargo clean 失败" }
    }

    Write-Host "==> cargo build --release" -ForegroundColor Cyan
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build --release 失败 (exit=$LASTEXITCODE)" }
}
finally {
    Pop-Location
}

# 4. 找到产物
$BuiltExe = Join-Path $SrcTauri 'target\release\kwrt-controller.exe'
if (-not (Test-Path $BuiltExe)) {
    throw "未找到产物 $BuiltExe；是否被 cargo 重命名了？"
}
$Size = (Get-Item $BuiltExe).Length
Write-Host ("==> 产物 {0}  ({1:N1} MB)" -f $BuiltExe, ($Size/1MB)) -ForegroundColor Green

# 5. 收纳
$DstDir = Join-Path $ReleaseDir $Version
New-Item -ItemType Directory -Force -Path $DstDir   | Out-Null
New-Item -ItemType Directory -Force -Path $LatestDir | Out-Null

$DstName = "kwrt-controller_v${Version}_${Stamp}.exe"
$Dst     = Join-Path $DstDir $DstName
Copy-Item $BuiltExe $Dst -Force
Copy-Item $BuiltExe (Join-Path $LatestDir 'kwrt-controller.exe') -Force

Write-Host "==> 已收纳到 $Dst" -ForegroundColor Green
Write-Host "==> latest -> $(Join-Path $LatestDir 'kwrt-controller.exe')" -ForegroundColor Green

# 6. 可选启动
if ($Run) {
    Write-Host "==> Start-Process $Dst" -ForegroundColor Cyan
    Start-Process -FilePath $Dst
}

# 7. 简短总结
Write-Host ""
Write-Host "完成。" -ForegroundColor Green
Write-Host "  版本:   $Version"
Write-Host "  产物:   $Dst"
Write-Host "  快捷:   $(Join-Path $LatestDir 'kwrt-controller.exe')"
