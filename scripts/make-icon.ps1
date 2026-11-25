#Requires -Version 5.1
<#
.SYNOPSIS
    Genera el icono de Windows de Axfterm a partir de la marca vectorial.

.DESCRIPTION
    Dibuja el icono (cuadrado redondeado, degradado y ^_) con GDI+ y escribe un
    .ico multi-tamaño con PNG incrustados. El resultado es reproducible y no
    necesita herramientas externas más allá de .NET Framework.

    Uso:
        pwsh -File scripts/make-icon.ps1
        pwsh -File scripts/make-icon.ps1 -Output crates/app/assets/axfterm.ico
#>
[CmdletBinding()]
param(
    [string]$Output = (Join-Path (Split-Path -Parent $PSScriptRoot) 'crates/app/assets/axfterm.ico'),
    [int[]]$Sizes = @(16, 20, 24, 32, 40, 48, 64, 128, 256)
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing

function New-IconPng {
    param([int]$Size)

    $bitmap = New-Object System.Drawing.Bitmap $Size, $Size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality

        $scale = $Size / 512.0
        $rect = New-Object System.Drawing.RectangleF 0, 0, $Size, $Size
        $top = [System.Drawing.ColorTranslator]::FromHtml('#303139')
        $bottom = [System.Drawing.ColorTranslator]::FromHtml('#08090C')

        $path = New-Object System.Drawing.Drawing2D.GraphicsPath
        $radius = 112 * $scale
        $diameter = $radius * 2
        $path.AddArc(0, 0, $diameter, $diameter, 180, 90)
        $path.AddArc($Size - $diameter, 0, $diameter, $diameter, 270, 90)
        $path.AddArc($Size - $diameter, $Size - $diameter, $diameter, $diameter, 0, 90)
        $path.AddArc(0, $Size - $diameter, $diameter, $diameter, 90, 90)
        $path.CloseFigure()

        $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, $top, $bottom, ([System.Drawing.Drawing2D.LinearGradientMode]::ForwardDiagonal)
        try {
            $graphics.FillPath($brush, $path)
        }
        finally {
            $brush.Dispose()
            $path.Dispose()
        }

        $pen = New-Object System.Drawing.Pen ([System.Drawing.ColorTranslator]::FromHtml('#F5F5F7')), (42 * $scale)
        try {
            $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
            $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
            $pen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
            [System.Drawing.PointF[]]$chevron = @(
                (New-Object System.Drawing.PointF (65 * $scale), (315 * $scale)),
                (New-Object System.Drawing.PointF (155 * $scale), (165 * $scale)),
                (New-Object System.Drawing.PointF (245 * $scale), (315 * $scale))
            )
            $graphics.DrawLines($pen, $chevron)
            $graphics.DrawLine($pen, (310 * $scale), (315 * $scale), (455 * $scale), (315 * $scale))
        }
        finally {
            $pen.Dispose()
        }

        $stream = New-Object System.IO.MemoryStream
        $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
        return ,$stream.ToArray()
    }
    finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

$images = @()
foreach ($size in $Sizes) {
    if ($size -lt 1 -or $size -gt 256) {
        throw "Tamaño de icono fuera de rango: $size"
    }
    $images += [pscustomobject]@{
        Size = $size
        Bytes = (New-IconPng -Size $size)
    }
}

$directory = Split-Path -Parent $Output
if ($directory -and -not (Test-Path $directory)) {
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
}

$file = [System.IO.File]::Open($Output, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write)
$writer = New-Object System.IO.BinaryWriter $file
try {
    # ICONDIR
    $writer.Write([uint16]0)               # reserved
    $writer.Write([uint16]1)               # type: icon
    $writer.Write([uint16]$images.Count)   # image count

    $offset = 6 + (16 * $images.Count)
    foreach ($image in $images) {
        $dimension = if ($image.Size -ge 256) { 0 } else { $image.Size }
        $writer.Write([byte]$dimension)    # width (0 = 256)
        $writer.Write([byte]$dimension)    # height
        $writer.Write([byte]0)             # palette
        $writer.Write([byte]0)             # reserved
        $writer.Write([uint16]1)           # planes
        $writer.Write([uint16]32)          # bit depth
        $writer.Write([uint32]$image.Bytes.Length)
        $writer.Write([uint32]$offset)
        $offset += $image.Bytes.Length
    }
    foreach ($image in $images) {
        $writer.Write($image.Bytes)
    }
}
finally {
    $writer.Dispose()
    $file.Dispose()
}

Write-Host "Icono escrito en $Output ($($images.Count) tamaños)." -ForegroundColor Green
