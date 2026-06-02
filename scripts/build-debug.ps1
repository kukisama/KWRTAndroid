<#
.SYNOPSIS
    KWRT Controller debug 编译 + 启动 + 全量日志落盘脚本。

.DESCRIPTION
    目标：暴露一切 debug 信息，不做任何友好处理。
      1. 杀掉旧 kwrt-controller 进程。
      2. src-tauri 下 cargo build （debug，profile=dev）。
         - cargo 输出直接 tee 到 debug-logs\build-<stamp>.log
      3. 用以下环境变量启动 debug exe：
           RUST_LOG=trace
           RUST_BACKTRACE=full
           RUST_LIB_BACKTRACE=1
           WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--enable-logging --v=1
         stdout / stderr 全部重定向到 debug-logs\run-<stamp>.log。
      4. 默认 -Follow，会 Get-Content -Wait 跟随 run 日志直到 Ctrl+C。

.PARAMETER NoBuild
    跳过 cargo build，直接拉起最近一次 debug 产物。

.PARAMETER NoRun
    只编译，不启动。

.PARAMETER NoFollow
    启动后不跟随日志（只打印路径）。

.PARAMETER Clean
    编译前 cargo clean。
#>
[CmdletBinding()]
param(
    [switch]$NoBuild,
    [switch]$NoRun,
    [switch]$NoFollow,
    [switch]$Clean
)

$ErrorActionPreference = 'Stop'

$RepoRoot  = Split-Path -Parent $PSScriptRoot
$SrcTauri  = Join-Path $RepoRoot 'src-tauri'
$CargoToml = Join-Path $SrcTauri 'Cargo.toml'
$LogDir    = Join-Path $RepoRoot 'debug-logs'
$Stamp     = Get-Date -Format 'yyyyMMdd-HHmmss'
$BuildLog  = Join-Path $LogDir "build-$Stamp.log"
$RunLog    = Join-Path $LogDir "run-$Stamp.log"
$LatestRun = Join-Path $LogDir 'run-latest.log'
$LatestBld = Join-Path $LogDir 'build-latest.log'

if (-not (Test-Path $CargoToml)) { throw "未找到 $CargoToml" }
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

Write-Host "==> 日志目录: $LogDir"
Write-Host "==> build log : $BuildLog"
Write-Host "==> run   log : $RunLog"

# 1. 杀旧实例
Get-Process kwrt-controller -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host "==> kill old PID=$($_.Id)"
    $_ | Stop-Process -Force
}

# 2. 编译
if (-not $NoBuild) {
    Push-Location $SrcTauri
    $prevEAP = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        if ($Clean) {
            Write-Host "==> cargo clean"
            & cmd /c "cargo clean 1>>`"$BuildLog`" 2>&1"
        }
        Write-Host "==> cargo build (debug)"
        $env:CARGO_TERM_COLOR = 'never'
        $env:RUST_BACKTRACE   = 'full'
        # 规避企业网/代理下 schannel CRT 在线吊销检查失败 (CRYPT_E_REVOCATION_OFFLINE)
        $env:CARGO_HTTP_CHECK_REVOKE = 'false'
        # 用 cmd 合并 stdout+stderr 写日志，避免 PowerShell 把 stderr 当 error 流
        & cmd /c "cargo build --color never 1>>`"$BuildLog`" 2>&1"
        $code = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $prevEAP
        Pop-Location
    }
    Copy-Item $BuildLog $LatestBld -Force
    if ($code -ne 0) {
        Write-Host "==> cargo build 失败 exit=$code，看 $BuildLog" -ForegroundColor Red
        exit $code
    }
}

$Exe = Join-Path $SrcTauri 'target\debug\kwrt-controller.exe'
if (-not (Test-Path $Exe)) { throw "未找到 debug 产物 $Exe" }
Write-Host ("==> exe = {0} ({1:N1} MB)" -f $Exe, ((Get-Item $Exe).Length/1MB))

if ($NoRun) { return }

# 3. 启动，全量日志重定向
$RunOut = $RunLog
$RunErr = [System.IO.Path]::ChangeExtension($RunLog, '.err.log')
'' | Set-Content -Path $RunOut -Encoding UTF8
'' | Set-Content -Path $RunErr -Encoding UTF8

# Webview2 的额外参数，让 webview 也吐 verbose 日志
$env:RUST_LOG          = 'trace'
$env:RUST_BACKTRACE    = 'full'
$env:RUST_LIB_BACKTRACE = '1'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--enable-logging --v=1'

# 头信息写到 stdout 文件
@(
    "==== launch $Stamp ====",
    "EXE : $Exe",
    "ENV : RUST_LOG=$($env:RUST_LOG)",
    "ENV : RUST_BACKTRACE=$($env:RUST_BACKTRACE)",
    "ENV : WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=$($env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS)",
    "==== begin stdout ===="
) | Add-Content -Path $RunOut -Encoding UTF8

# Start-Process 启动的子进程独立于本脚本生命周期
$proc = Start-Process -FilePath $Exe `
    -WorkingDirectory $SrcTauri `
    -RedirectStandardOutput $RunOut `
    -RedirectStandardError  $RunErr `
    -PassThru `
    -WindowStyle Hidden

Write-Host "==> launched PID=$($proc.Id)" -ForegroundColor Green
Write-Host "==> stdout -> $RunOut"
Write-Host "==> stderr -> $RunErr"

# latest 快捷
Copy-Item $RunOut $LatestRun -Force -ErrorAction SilentlyContinue

if ($NoFollow) {
    Write-Host "==> 不跟随。查看:"
    Write-Host "      Get-Content -Wait `"$RunOut`""
    Write-Host "      Get-Content -Wait `"$RunErr`""
    return
}

Write-Host "==> 同时跟随 stdout+stderr （Ctrl+C 退出跟随，进程继续运行）" -ForegroundColor Cyan
try {
    # 用 Start-Job 跟随 stderr，主线程跟 stdout
    $errJob = Start-Job -ScriptBlock {
        param($p) Get-Content -Path $p -Wait -Tail 0 | ForEach-Object { '[ERR] ' + $_ }
    } -ArgumentList $RunErr
    Get-Content -Path $RunOut -Wait -Tail 0 | ForEach-Object {
        # 顺带把 err job 的输出 drain 出来
        Receive-Job $errJob -ErrorAction SilentlyContinue
        $_
    }
}
finally {
    if ($errJob) { Stop-Job $errJob -ErrorAction SilentlyContinue; Remove-Job $errJob -Force -ErrorAction SilentlyContinue }
    Copy-Item $RunOut $LatestRun -Force -ErrorAction SilentlyContinue
}
