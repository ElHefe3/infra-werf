param([int]$Port = 8770, [string]$InventoryDirectory = '', [switch]$Background)
$ErrorActionPreference = 'Stop'
$atlasRoot = $PSScriptRoot
if (-not $InventoryDirectory) { $InventoryDirectory = Join-Path (Split-Path $atlasRoot) 'ops\dashboard\data' }
$atlasPython = Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'
if (-not (Test-Path -LiteralPath $atlasPython)) { $atlasPython = (Get-Command python -ErrorAction Stop).Source }
if ($Background) {
    $atlasData = Join-Path $atlasRoot 'data'
    New-Item -ItemType Directory -Path $atlasData -Force | Out-Null
    $atlasArguments = @('"' + (Join-Path $atlasRoot 'atlas.py') + '"', '--port', "$Port", '--inventory-dir', '"' + $InventoryDirectory + '"')
    $atlasProcess = Start-Process -FilePath $atlasPython -ArgumentList $atlasArguments -WorkingDirectory $atlasRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $atlasData 'server.stdout.log') -RedirectStandardError (Join-Path $atlasData 'server.stderr.log')
    $atlasProcess.Id | Set-Content -LiteralPath (Join-Path $atlasData 'server.pid')
    Write-Host "Atlas started in the background (PID $($atlasProcess.Id)). Open http://127.0.0.1:$Port/"
    exit
}
& $atlasPython (Join-Path $atlasRoot 'atlas.py') --port $Port --inventory-dir $InventoryDirectory
