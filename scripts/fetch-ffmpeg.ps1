# Lädt den LGPL-Shared-Build von FFmpeg 9.0 (BtbN) nach src-tauri/ffmpeg.
# Passend zu ffmpeg-next 9.x. Nur LGPL-Builds verwenden (keine GPL-Encoder wie libx264).
$ErrorActionPreference = 'Stop'
$dst = Join-Path $PSScriptRoot '..\src-tauri\ffmpeg'
$url = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-n9.0-latest-win64-lgpl-shared-9.0.zip'
New-Item -ItemType Directory -Force $dst | Out-Null
$zip = Join-Path $dst 'ffmpeg.zip'
$ProgressPreference = 'SilentlyContinue'
Invoke-WebRequest $url -OutFile $zip
Expand-Archive $zip -DestinationPath "$dst\_x" -Force
Get-ChildItem (Get-ChildItem "$dst\_x" | Select-Object -First 1).FullName | Move-Item -Destination $dst -Force
Remove-Item -Recurse -Force "$dst\_x", $zip
Write-Host "FFmpeg liegt in $dst"
