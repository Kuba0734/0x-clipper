# Erzeugt das App-Icon (1024 px) und die Tray-Icons (normal + Aufnahme) per GDI+.
# Monochrom (Design-System): dunkle Kachel mit weißem Ring und Play-Symbol, Rot nur für Aufnahme.
# Danach: pnpm tauri icon src-tauri/icons/app-icon.png
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'
$out = Join-Path $PSScriptRoot '..\src-tauri\icons'
New-Item -ItemType Directory -Force $out | Out-Null

function New-Icon([int]$size, [string]$path, [bool]$rec) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.TextRenderingHint = 'AntiAliasGridFit'
    $g.Clear([System.Drawing.Color]::Transparent)
    $r = [single]($size * 0.22)
    $rect = New-Object System.Drawing.RectangleF 0, 0, $size, $size
    $gp = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $r * 2
    $gp.AddArc(0, 0, $d, $d, 180, 90)
    $gp.AddArc($size - $d, 0, $d, $d, 270, 90)
    $gp.AddArc($size - $d, $size - $d, $d, $d, 0, 90)
    $gp.AddArc(0, $size - $d, $d, $d, 90, 90)
    $gp.CloseFigure()
    $c1 = [System.Drawing.Color]::FromArgb(255, 0x1C, 0x1C, 0x1F)
    $c2 = [System.Drawing.Color]::FromArgb(255, 0x05, 0x05, 0x06)
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, $c1, $c2, 45
    $g.FillPath($brush, $gp)
    # Stilisierter "Replay"-Ring mit Play-Dreieck.
    $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), ([single]($size * 0.075))
    $pen.StartCap = 'Round'; $pen.EndCap = 'Round'
    $m = [single]($size * 0.24)
    $g.DrawArc($pen, $m, $m, $size - 2 * $m, $size - 2 * $m, -60, 300)
    $cx = $size / 2; $cy = $size / 2; $t = $size * 0.13
    $tri = @(
        (New-Object System.Drawing.PointF ([single]($cx - $t * 0.7)), ([single]($cy - $t))),
        (New-Object System.Drawing.PointF ([single]($cx + $t * 1.0)), ([single]$cy)),
        (New-Object System.Drawing.PointF ([single]($cx - $t * 0.7)), ([single]($cy + $t)))
    )
    $g.FillPolygon([System.Drawing.Brushes]::White, $tri)
    if ($rec) {
        $dot = [single]($size * 0.42)
        $red = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 0xEF, 0x44, 0x44))
        $g.FillEllipse([System.Drawing.Brushes]::White, $size - $dot - 0, $size - $dot - 0, $dot, $dot)
        $g.FillEllipse($red, $size - $dot + $size * 0.05, $size - $dot + $size * 0.05, $dot - $size * 0.1, $dot - $size * 0.1)
    }
    $g.Dispose()
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

New-Icon 1024 (Join-Path $out 'app-icon.png') $false
New-Icon 32 (Join-Path $out 'tray.png') $false
New-Icon 32 (Join-Path $out 'tray-rec.png') $true
Write-Host "Icons in $out"
