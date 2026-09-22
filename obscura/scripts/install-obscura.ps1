$ErrorActionPreference = 'Stop'
$version = 'v0.2.3'
$root = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $root '.tools'
New-Item -ItemType Directory -Force $destination | Out-Null
$archive = Join-Path $destination 'obscura.zip'
$url = "https://github.com/h4ckf0r0day/obscura/releases/download/$version/obscura-x86_64-windows.zip"
Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $archive
Expand-Archive -LiteralPath $archive -DestinationPath $destination -Force
& (Join-Path $destination 'obscura.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'Obscura installation failed' }
