# Makes the launcher banner's looping video (oteria-launcher/assets/menu-loop.h264) from a recording.
# Raw H.264 (Annex B), Constrained Baseline with no B-frames, as OpenH264 decodes it in the launcher.
# The recording's last 80 rows are a black strip (badly cropped capture): cut off here.
param(
    [string]$Source = 'E:\rec-mes\2026-11-18_19-47-31.mp4',
    [double]$Start = 2,
    [double]$Duration = 18
)
$out = Join-Path $PSScriptRoot '..\..\oteria-launcher\assets\menu-loop.h264'
ffmpeg -v error -y -ss $Start -t $Duration -i $Source -an `
    -vf 'crop=1920:1000:0:0,scale=1280:666:flags=lanczos,fps=30' `
    -c:v libx264 -profile:v baseline -bf 0 -g 60 -crf 24 -preset slow -pix_fmt yuv420p `
    -f h264 $out
Get-Item $out | Select-Object Name, Length
