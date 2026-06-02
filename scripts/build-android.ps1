# 一键命令行构建 Android APK。
# 用法：
#   pwsh scripts/build-android.ps1                # debug 构建
#   pwsh scripts/build-android.ps1 -Release       # release 构建（未签名）
#
# 自动检查 / 提示：JDK 17、ANDROID_HOME、cmdline-tools、platform-34、build-tools 34、gradle wrapper
# 不会改你的系统环境变量；缺什么就清晰报错说装哪个。
#
# 产物会复制到仓库根目录 dist-android/。

[CmdletBinding()]
param(
    [switch]$Release
)

$ErrorActionPreference = 'Stop'
$RepoRoot   = Split-Path -Parent $PSScriptRoot
$AndroidDir = Join-Path $RepoRoot 'android'
$OutDir     = Join-Path $RepoRoot 'dist-android'

function Fail($msg) { Write-Host "ERR  $msg" -ForegroundColor Red; exit 1 }
function Info($msg) { Write-Host "==>  $msg" -ForegroundColor Cyan }
function Ok($msg)   { Write-Host "OK   $msg" -ForegroundColor Green }

# ---------- 1) JDK 17 ----------
Info '检查 JDK 17'
$javaCmd = Get-Command java -ErrorAction SilentlyContinue
if (-not $javaCmd) {
    Fail @"
未找到 java。请安装 JDK 17，例如：
  winget install Microsoft.OpenJDK.17
装完新开一个 PowerShell 再跑本脚本。
"@
}
$verLine = (& java -version 2>&1) -join "`n"
# "openjdk version \"17.0.x\"" 或 "java version \"17.0.x\""
if ($verLine -notmatch 'version "(\d+)(\.|")') {
    Fail "无法解析 java -version 输出：$verLine"
}
$major = [int]$Matches[1]
if ($major -lt 17) {
    Fail "需要 JDK 17+，当前是 $major。装 OpenJDK 17：winget install Microsoft.OpenJDK.17"
}
Ok  "java major=$major"

# ---------- 2) ANDROID_HOME ----------
Info '检查 ANDROID_HOME'
$androidHome = $env:ANDROID_HOME
if (-not $androidHome) { $androidHome = $env:ANDROID_SDK_ROOT }
if (-not $androidHome -or -not (Test-Path $androidHome)) {
    Fail @"
未找到 ANDROID_HOME。请按以下步骤一次性配置：
  1. 下载 command-line tools (Windows zip)：
     https://developer.android.com/studio#command-line-tools-only
  2. 解压到 C:\Android\cmdline-tools\latest\
     注意末级目录必须叫 latest（里面有 bin/sdkmanager.bat）。
  3. 在 PowerShell 跑（永久写入用户环境变量）：
       setx ANDROID_HOME "C:\Android"
       setx PATH "`$env:PATH;C:\Android\cmdline-tools\latest\bin;C:\Android\platform-tools"
  4. 关掉所有 PowerShell 窗口重开，再跑本脚本。
"@
}
Ok "ANDROID_HOME=$androidHome"

# ---------- 3) sdkmanager + 必要组件 ----------
$sdkmanager = Join-Path $androidHome 'cmdline-tools\latest\bin\sdkmanager.bat'
if (-not (Test-Path $sdkmanager)) {
    Fail "未找到 $sdkmanager。说明 cmdline-tools 没解到 cmdline-tools\latest\ 下面，请按上面提示重新放置。"
}

$needPkgs = @(
    'platform-tools',
    'platforms;android-34',
    'build-tools;34.0.0'
)
Info "检查 / 安装 SDK 组件：$($needPkgs -join ', ')"
# sdkmanager --install 是幂等的；已装会跳过。第一次会拉 license。
& $sdkmanager --licenses 2>&1 | Out-Null
& $sdkmanager $needPkgs
if ($LASTEXITCODE -ne 0) { Fail "sdkmanager 安装失败 (exit=$LASTEXITCODE)" }
Ok 'SDK 组件就绪'

# ---------- 4) Gradle wrapper ----------
Push-Location $AndroidDir
try {
    if (-not (Test-Path 'gradlew.bat')) {
        Info '首次：生成 Gradle Wrapper 8.9'
        $gradleCmd = Get-Command gradle -ErrorAction SilentlyContinue
        $gradleExe = $gradleCmd?.Path
        if (-not $gradleExe) {
            # 自动下载临时 gradle dist 跑 wrapper init，之后只用 gradlew.bat
            $cacheRoot = Join-Path $RepoRoot '.gradle-dist'
            $gradleVer = '8.9'
            $gradleHome = Join-Path $cacheRoot "gradle-$gradleVer"
            $gradleExe  = Join-Path $gradleHome 'bin\gradle.bat'
            if (-not (Test-Path $gradleExe)) {
                # 默认走腾讯云镜像，国内速度快且不被截断；想用官方源把下面 $zipUrl 改回 services.gradle.org 即可
                $zipUrl  = "https://mirrors.cloud.tencent.com/gradle/gradle-$gradleVer-bin.zip"
                $zipPath = Join-Path $cacheRoot "gradle-$gradleVer-bin.zip"
                New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null
                Info "下载 $zipUrl"
                # PowerShell 5.1 默认 TLS 1.0；强制 1.2 兼容 services.gradle.org
                [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
                $ProgressPreference = 'SilentlyContinue'  # 否则 Invoke-WebRequest 进度条会大幅拖慢下载
                Invoke-WebRequest -Uri $zipUrl -OutFile $zipPath -UseBasicParsing
                Info "解压到 $cacheRoot"
                # Expand-Archive 在 pwsh 7 里偶发卡 / 失败，用 tar 更稳
                tar -xf $zipPath -C $cacheRoot
                Remove-Item $zipPath -Force
            }
            if (-not (Test-Path $gradleExe)) { Fail "下载的 gradle 解压后没找到 $gradleExe" }
            Ok "临时 gradle：$gradleExe"
        }
        # wrapper 自己也走镜像，免得后续 gradlew 第一次跑时还要去官方源拉
        & $gradleExe wrapper --gradle-version 8.9 --gradle-distribution-url "https://mirrors.cloud.tencent.com/gradle/gradle-8.9-bin.zip"
        if ($LASTEXITCODE -ne 0) { Fail "生成 wrapper 失败 (exit=$LASTEXITCODE)" }
        Ok '已生成 gradlew.bat'
    }

    # ---------- 5) 编译 ----------
    $task = if ($Release) { 'assembleRelease' } else { 'assembleDebug' }
    Info "执行 .\gradlew.bat $task"
    & .\gradlew.bat $task --console=plain
    if ($LASTEXITCODE -ne 0) { Fail "构建失败 (exit=$LASTEXITCODE)" }

    # ---------- 6) 收集产物 ----------
    $apkPattern = if ($Release) {
        'app\build\outputs\apk\release\*.apk'
    } else {
        'app\build\outputs\apk\debug\*.apk'
    }
    $apks = Get-ChildItem $apkPattern -ErrorAction SilentlyContinue
    if (-not $apks) { Fail "没找到 APK：$apkPattern" }

    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    foreach ($apk in $apks) {
        Copy-Item $apk.FullName (Join-Path $OutDir $apk.Name) -Force
        Ok "→ $($apk.Name) (`"$OutDir`")"
    }

    Write-Host ''
    Write-Host '安装到已连接手机：' -ForegroundColor Yellow
    Write-Host "  adb install -r `"$(Join-Path $OutDir $apks[0].Name)`""
}
finally {
    Pop-Location
}
