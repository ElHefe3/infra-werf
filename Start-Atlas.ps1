param([int]$Port = 8770, [string]$InventoryDirectory = '')
$ErrorActionPreference = 'Stop'
$atlasRoot = $PSScriptRoot
if (-not $InventoryDirectory) { $InventoryDirectory = Join-Path (Split-Path $atlasRoot) 'server map\dashboard\data' }
$atlasPython = Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'
if (-not (Test-Path -LiteralPath $atlasPython)) { $atlasPython = (Get-Command python -ErrorAction Stop).Source }
& $atlasPython (Join-Path $atlasRoot 'atlas.py') --port $Port --inventory-dir $InventoryDirectory
