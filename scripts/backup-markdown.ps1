<#
.SYNOPSIS
    Backup Markdown files that are not already safely tracked by Git.

.DESCRIPTION
    Scans a source directory and copies review-worthy Markdown files into a timestamped backup directory
    while preserving the source-relative directory structure.

    Default behavior:
    - If a scanned directory is a Git repository, tracked Markdown files are excluded.
    - Untracked, ignored, or otherwise unclassified Markdown files are copied.
    - Non-Git directories are copied after excluding common build/output/dependency directories.
    - Common packaging/build/temp/dependency directories are excluded globally.
    - A Chinese Markdown report named 数据梳理.md is generated in the backup directory.

.EXAMPLE
    .\scripts\backup-markdown.ps1 `
        -SourceRoot 'C:\Users\kukisama\Downloads\Scripts' `
        -BackupRoot 'C:\Users\kukisama\Downloads\mdbackup'

.EXAMPLE
    .\scripts\backup-markdown.ps1 -SourceRoot 'C:\aaa' -BackupRoot 'C:\bbb' -DryRun

.NOTES
    For Windows + Git paths containing Chinese characters, this script forces UTF-8 console output
    and uses `git -c core.quotepath=false` to avoid mojibake path matching issues.
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)]
    [ValidateScript({ Test-Path -LiteralPath $_ -PathType Container })]
    [string]$SourceRoot = (Get-Location).Path,

    [Parameter(Mandatory = $false)]
    [string]$BackupRoot = (Join-Path (Get-Location).Path 'mdbackup'),

    [Parameter(Mandatory = $false)]
    [string]$TimestampFormat = 'yyyyMMdd-HHmmss',

    [Parameter(Mandatory = $false)]
    [string[]]$ExcludedSegments = @(
        '.git', '.svn', '.hg',
        'node_modules', '.venv', 'venv', 'env', 'site-packages', '__pycache__',
        'target', 'bin', 'obj', 'dist', 'build', 'release', 'debug',
        'output', 'outputs', 'out', 'artifacts', 'coverage',
        'tmp', 'temp', '.tmp', '.temp', '.cache', '.next', '.nuxt', '.turbo',
        'publish', 'published', 'packages', '.parcel-cache'
    ),

    [Parameter(Mandatory = $false)]
    [switch]$IncludeTracked,

    [Parameter(Mandatory = $false)]
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8NoBom
$OutputEncoding = $utf8NoBom

function Test-ExcludedPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $normalized = $Path -replace '/', '\'
    foreach ($segment in $ExcludedSegments) {
        if ($normalized -match "(^|\\)$([regex]::Escape($segment))(\\|$)") {
            return $true
        }
    }

    return $false
}

function Get-RelativePathSafe {
    param(
        [Parameter(Mandatory = $true)][string]$Base,
        [Parameter(Mandatory = $true)][string]$Full
    )

    $baseUri = [System.Uri]::new(($Base.TrimEnd('\') + '\'))
    $fullUri = [System.Uri]::new($Full)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($fullUri).ToString()).Replace('/', '\')
}

function Get-NormalizedFullPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    return ([System.IO.Path]::GetFullPath($Path)).TrimEnd('\').ToLowerInvariant()
}

function Escape-MarkdownCell {
    param([AllowNull()][object]$Value)

    if ($null -eq $Value) {
        return ''
    }

    return (($Value.ToString() -replace '\r?\n', ' ') -replace '\|', '／').Trim()
}

function Get-MarkdownTitle {
    param([Parameter(Mandatory = $true)][string]$File)

    try {
        $lines = @(Get-Content -LiteralPath $File -TotalCount 80 -ErrorAction Stop)
        $title = ($lines | Where-Object { $_ -match '^\s*#+' } | Select-Object -First 1)
        if (-not $title) {
            $title = ($lines | Where-Object { $_.Trim().Length -gt 0 } | Select-Object -First 1)
        }
        return (Escape-MarkdownCell $title)
    }
    catch {
        return ''
    }
}

function Get-ScanRoots {
    param([Parameter(Mandatory = $true)][string]$Root)

    if (Test-Path -LiteralPath (Join-Path $Root '.git') -PathType Container) {
        return @(Get-Item -LiteralPath $Root)
    }

    return @(Get-ChildItem -LiteralPath $Root -Directory -Force | Sort-Object Name)
}

$SourceRoot = [System.IO.Path]::GetFullPath($SourceRoot)
$BackupRoot = [System.IO.Path]::GetFullPath($BackupRoot)
$timestamp = Get-Date -Format $TimestampFormat
$destinationRoot = Join-Path $BackupRoot $timestamp
$reportPath = Join-Path $destinationRoot '数据梳理.md'

if (-not $DryRun) {
    New-Item -ItemType Directory -Path $destinationRoot -Force | Out-Null
}

$items = New-Object System.Collections.Generic.List[object]
$summaries = New-Object System.Collections.Generic.List[object]
$scanRoots = Get-ScanRoots -Root $SourceRoot

foreach ($scanRoot in $scanRoots) {
    if (Test-ExcludedPath -Path $scanRoot.FullName) {
        continue
    }

    $isGit = Test-Path -LiteralPath (Join-Path $scanRoot.FullName '.git') -PathType Container
    $trackedSet = @{}
    $untrackedSet = @{}
    $ignoredSet = @{}
    $gitStatusCount = 0

    if ($isGit) {
        Push-Location -LiteralPath $scanRoot.FullName
        try {
            $gitStatusCount = @(git -c core.quotepath=false status --porcelain=v1 2>$null).Count
            foreach ($path in @(git -c core.quotepath=false ls-files -- '*.md' 2>$null)) {
                $trackedSet[(Get-NormalizedFullPath (Join-Path $scanRoot.FullName $path))] = $true
            }
            foreach ($path in @(git -c core.quotepath=false ls-files --others --exclude-standard -- '*.md' 2>$null)) {
                $untrackedSet[(Get-NormalizedFullPath (Join-Path $scanRoot.FullName $path))] = $true
            }
            foreach ($path in @(git -c core.quotepath=false ls-files --others --ignored --exclude-standard -- '*.md' 2>$null)) {
                $ignoredSet[(Get-NormalizedFullPath (Join-Path $scanRoot.FullName $path))] = $true
            }
        }
        finally {
            Pop-Location
        }
    }

    $markdownFiles = @(
        Get-ChildItem -LiteralPath $scanRoot.FullName -Recurse -Force -File -Filter '*.md' -ErrorAction SilentlyContinue |
            Where-Object { -not (Test-ExcludedPath -Path $_.FullName) } |
            Sort-Object FullName
    )

    $copiedCount = 0
    $skippedTrackedCount = 0

    foreach ($file in $markdownFiles) {
        $fullKey = Get-NormalizedFullPath $file.FullName
        $scanRootRelativeFile = Get-RelativePathSafe -Base $scanRoot.FullName -Full $file.FullName
        $sourceRootRelativeFile = Get-RelativePathSafe -Base $SourceRoot -Full $file.FullName

        $gitClass = if (-not $isGit) {
            '非Git目录'
        }
        elseif ($trackedSet.ContainsKey($fullKey)) {
            'tracked'
        }
        elseif ($untrackedSet.ContainsKey($fullKey)) {
            'untracked'
        }
        elseif ($ignoredSet.ContainsKey($fullKey)) {
            'ignored'
        }
        else {
            '未被git ls-files识别'
        }

        if (-not $IncludeTracked -and $gitClass -eq 'tracked') {
            $skippedTrackedCount++
            continue
        }

        $destination = Join-Path $destinationRoot $sourceRootRelativeFile
        if (-not $DryRun) {
            New-Item -ItemType Directory -Path (Split-Path $destination -Parent) -Force | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
        }
        $copiedCount++

        $items.Add([pscustomobject]@{
            Root = $scanRoot.Name
            IsGit = $isGit
            File = $scanRootRelativeFile
            RootRelativeFile = $sourceRootRelativeFile
            Dir = (Split-Path $scanRootRelativeFile -Parent)
            GitClass = $gitClass
            KB = [math]::Round($file.Length / 1KB, 1)
            Title = Get-MarkdownTitle -File $file.FullName
            Source = $file.FullName
            Destination = $destination
        })
    }

    $summaries.Add([pscustomobject]@{
        Root = $scanRoot.Name
        IsGit = $isGit
        CopiedMdCount = $copiedCount
        SkippedTrackedMdCount = $skippedTrackedCount
        GitStatusCount = $gitStatusCount
        Path = $scanRoot.FullName
    })
}

$byDirectory = @(
    $items |
        Group-Object Root, Dir |
        Sort-Object Count -Descending |
        ForEach-Object {
            $first = $_.Group[0]
            [pscustomobject]@{
                Root = $first.Root
                Dir = if ([string]::IsNullOrWhiteSpace($first.Dir)) { '.' } else { $first.Dir }
                Count = $_.Count
                GitClasses = (($_.Group.GitClass | Sort-Object -Unique) -join ', ')
                Example = $_.Group[0].File
                Purpose = $_.Group[0].Title
            }
        }
)

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add('# Markdown 备份数据梳理')
$lines.Add('')
$lines.Add('- 扫描时间：' + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'))
$lines.Add('- 源目录：`' + $SourceRoot + '`')
$lines.Add('- 备份目录：`' + $destinationRoot + '`')
$lines.Add('- DryRun：`' + $DryRun.ToString() + '`')
$lines.Add('- IncludeTracked：`' + $IncludeTracked.ToString() + '`')
$lines.Add('- 备份规则：默认排除 Git tracked Markdown，仅复制 untracked / ignored / 未被 git ls-files 识别的 Markdown；非 Git 目录复制排除产物目录后的有效 Markdown。')
$lines.Add('- 本次匹配 Markdown：' + $items.Count + ' 个')
$lines.Add('')
$lines.Add('## 排除规则')
$lines.Add('')
$lines.Add('以下目录被视为产物、临时目录、依赖目录、构建/打包目录，已整体排除：')
$lines.Add('')
$lines.Add('`' + (($ExcludedSegments | Sort-Object) -join '`, `') + '`')
$lines.Add('')
$lines.Add('## 目录总览')
$lines.Add('')
$lines.Add('| 目录 | Git目录 | 匹配MD | 已排除tracked MD | git状态项 | 路径 |')
$lines.Add('|---|---:|---:|---:|---:|---|')
foreach ($summary in @($summaries | Where-Object { $_.CopiedMdCount -gt 0 -or $_.SkippedTrackedMdCount -gt 0 } | Sort-Object @{Expression='CopiedMdCount';Descending=$true}, Root)) {
    $lines.Add('| ' + (Escape-MarkdownCell $summary.Root) + ' | ' + $summary.IsGit + ' | ' + $summary.CopiedMdCount + ' | ' + $summary.SkippedTrackedMdCount + ' | ' + $summary.GitStatusCount + ' | `' + (Escape-MarkdownCell $summary.Path) + '` |')
}
$lines.Add('')
$lines.Add('## 按目录汇总')
$lines.Add('')
$lines.Add('| 仓库/目录 | 子目录 | 数量 | Git状态 | 示例文件 | 大概用途/标题线索 |')
$lines.Add('|---|---|---:|---|---|---|')
foreach ($group in $byDirectory) {
    $lines.Add('| ' + (Escape-MarkdownCell $group.Root) + ' | `' + (Escape-MarkdownCell $group.Dir) + '` | ' + $group.Count + ' | ' + (Escape-MarkdownCell $group.GitClasses) + ' | `' + (Escape-MarkdownCell $group.Example) + '` | ' + (Escape-MarkdownCell $group.Purpose) + ' |')
}
if ($byDirectory.Count -eq 0) {
    $lines.Add('| - | - | 0 | - | - | 没有需要备份的 Markdown |')
}
$lines.Add('')
$lines.Add('## 完整清单')
$lines.Add('')
$lines.Add('| 仓库/目录 | Git状态 | 大小KB | 备份相对路径 | 标题/内容线索 |')
$lines.Add('|---|---|---:|---|---|')
foreach ($item in @($items | Sort-Object Root, File)) {
    $lines.Add('| ' + (Escape-MarkdownCell $item.Root) + ' | ' + (Escape-MarkdownCell $item.GitClass) + ' | ' + $item.KB + ' | `' + (Escape-MarkdownCell $item.RootRelativeFile) + '` | ' + (Escape-MarkdownCell $item.Title) + ' |')
}

if (-not $DryRun) {
    $lines | Set-Content -LiteralPath $reportPath -Encoding UTF8
}

$result = [pscustomobject]@{
    SourceRoot = $SourceRoot
    BackupRoot = $BackupRoot
    DestinationRoot = $destinationRoot
    ReportPath = if ($DryRun) { $null } else { $reportPath }
    DryRun = [bool]$DryRun
    IncludeTracked = [bool]$IncludeTracked
    MatchedMarkdown = $items.Count
    ReportIncluded = -not [bool]$DryRun
    TotalMarkdownInDestination = if ($DryRun) { 0 } else { @((Get-ChildItem -LiteralPath $destinationRoot -Recurse -File -Filter '*.md')).Count }
}

$result | ConvertTo-Json -Depth 4
