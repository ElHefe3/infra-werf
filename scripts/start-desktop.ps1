# Open the registered primary release. Development uses mise run dev:desktop.
$ErrorActionPreference = 'Stop'
$AtlasRoot = Split-Path $PSScriptRoot
$InfraWerfRegistration = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\InfraWerf' -ErrorAction SilentlyContinue
$InfraWerfDirectory = if ($InfraWerfRegistration.InstallLocation) { $InfraWerfRegistration.InstallLocation.Trim('"') } else { Join-Path $env:LOCALAPPDATA 'InfraWerf' }
$AtlasBinary = Join-Path $InfraWerfDirectory 'atlas-native.exe'
if (-not (Test-Path -LiteralPath $AtlasBinary)) {
    throw 'Install InfraWerf first. For a development window, use mise run dev:desktop.'
}
# Test/data/debug overrides never leak into the ordinary owner launcher.
Remove-Item Env:ATLAS_NATIVE_DB,Env:ATLAS_LEGACY_DB,Env:ATLAS_REPORTS_DIR,Env:WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue
Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
# This is the interactive app the owner needs to see, not a background helper.
Start-Process -FilePath $AtlasBinary -WorkingDirectory $AtlasRoot -WindowStyle Normal
