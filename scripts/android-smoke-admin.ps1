# Run only with explicit permission to elevate this native build. This script
# does not change Developer Mode, security policy or global tool defaults.
$ErrorActionPreference = 'Stop'
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    [Console]::Error.WriteLine('This helper requires an explicitly approved administrator process for Tauri native-library links.')
    exit 2
}
Set-Location -LiteralPath (Split-Path $PSScriptRoot)
$AtlasLogDirectory = Join-Path (Get-Location) '.tools/logs'
New-Item -ItemType Directory -Path $AtlasLogDirectory -Force | Out-Null
$AtlasExitPath = Join-Path $AtlasLogDirectory 'android-smoke.exit'
Set-Content -LiteralPath $AtlasExitPath -Value 'running'
Start-Transcript -Path (Join-Path $AtlasLogDirectory 'android-smoke.log') -Force | Out-Null
$AtlasExit = 1
try {
    & mise run smoke:android
    $AtlasExit = $LASTEXITCODE
    if ($AtlasExit -eq 0) {
        Write-Output 'First APK build passed; verifying incremental cache reuse.'
        & mise run smoke:android
        $AtlasExit = $LASTEXITCODE
    }
    if ($AtlasExit -eq 0) {
        & mise run test:feature -- F01
        $AtlasExit = $LASTEXITCODE
    }
} finally {
    Set-Content -LiteralPath $AtlasExitPath -Value $AtlasExit
    Stop-Transcript | Out-Null
}
exit $AtlasExit
