param(
    [Parameter(Mandatory)][string]$LinuxDir,
    [Parameter(Mandatory)][string]$WindowsDir,
    [Parameter(Mandatory)][string]$OutDir,
    [int]$Quality = 80
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force $OutDir | Out-Null
$codec = [Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object MimeType -eq 'image/jpeg'
$params = New-Object Drawing.Imaging.EncoderParameters 1
$params.Param[0] = New-Object Drawing.Imaging.EncoderParameter ([Drawing.Imaging.Encoder]::Quality), ([long]$Quality)

Get-ChildItem $LinuxDir -Filter *.png | ForEach-Object {
    $other = Join-Path $WindowsDir $_.Name
    if (-not (Test-Path $other)) { Write-Warning "missing Windows capture: $($_.Name)"; return }
    $a = [Drawing.Image]::FromFile($_.FullName)
    $b = [Drawing.Image]::FromFile($other)
    $canvas = New-Object Drawing.Bitmap ($a.Width + $b.Width + 16), ([Math]::Max($a.Height, $b.Height) + 28)
    $g = [Drawing.Graphics]::FromImage($canvas)
    $g.Clear([Drawing.Color]::FromArgb(64, 64, 64))
    $font = New-Object Drawing.Font 'Segoe UI', 12
    $g.DrawString('Linux QML (reference)', $font, [Drawing.Brushes]::White, 6, 4)
    $g.DrawString('Windows Svelte / WebView2', $font, [Drawing.Brushes]::White, $a.Width + 22, 4)
    $g.DrawImage($a, 0, 28, $a.Width, $a.Height)
    $g.DrawImage($b, $a.Width + 16, 28, $b.Width, $b.Height)
    $canvas.Save((Join-Path $OutDir ($_.BaseName + '.jpg')), $codec, $params)
    $g.Dispose(); $canvas.Dispose(); $a.Dispose(); $b.Dispose()
}
