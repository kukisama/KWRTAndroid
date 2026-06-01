$ErrorActionPreference = 'Stop'
$iconDir = Join-Path $PSScriptRoot '..\src-tauri\icons'
$iconDir = (Resolve-Path -LiteralPath $iconDir -ErrorAction SilentlyContinue)
if (-not $iconDir) {
    $iconDir = Join-Path $PSScriptRoot '..\src-tauri\icons'
    New-Item -ItemType Directory -Force -Path $iconDir | Out-Null
    $iconDir = (Resolve-Path -LiteralPath $iconDir).Path
} else { $iconDir = $iconDir.Path }
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 64, 64
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$bg = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 44, 62, 80))
$g.FillRectangle($bg, 0, 0, 64, 64)
$fg = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 52, 152, 219))
$font = New-Object System.Drawing.Font 'Segoe UI', 22, ([System.Drawing.FontStyle]::Bold)
$sf = New-Object System.Drawing.StringFormat
$sf.Alignment = 'Center'
$sf.LineAlignment = 'Center'
$g.DrawString('K', $font, $fg, (New-Object System.Drawing.RectangleF 0, 0, 64, 64), $sf)
$g.Dispose()
$ms = New-Object System.IO.MemoryStream
$bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
$png = $ms.ToArray()
$ms.Dispose()
$bmp.Dispose()
$out = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter $out
$bw.Write([uint16]0)
$bw.Write([uint16]1)
$bw.Write([uint16]1)
$bw.Write([byte]64)
$bw.Write([byte]64)
$bw.Write([byte]0)
$bw.Write([byte]0)
$bw.Write([uint16]1)
$bw.Write([uint16]32)
$bw.Write([uint32]$png.Length)
$bw.Write([uint32]22)
$bw.Write($png)
$icoPath = Join-Path $iconDir 'icon.ico'
[System.IO.File]::WriteAllBytes($icoPath, $out.ToArray())
$bw.Dispose()
$out.Dispose()
Write-Host "icon.ico -> $icoPath ($((Get-Item $icoPath).Length) bytes)"
