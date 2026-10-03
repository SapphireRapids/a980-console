param([string]$Exe, [string]$OutPng)
Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Icon]::ExtractAssociatedIcon($Exe)
$bmp = $icon.ToBitmap()
$bmp.Save($OutPng, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output ("extracted " + $bmp.Width + "x" + $bmp.Height)
