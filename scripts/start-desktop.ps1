# Launch the built native desktop, using a one-time read-only import of the
# reference map if the native application data store does not exist yet.
$ErrorActionPreference = 'Stop'
$AtlasRoot = Split-Path $PSScriptRoot
$AtlasBinary = Join-Path $AtlasRoot 'target/debug/atlas-native.exe'
if (-not (Test-Path -LiteralPath $AtlasBinary)) {
    throw 'Build Atlas first with: mise run smoke:desktop'
}
if (-not $env:ATLAS_LEGACY_DB) {
    $AtlasReference = Join-Path $AtlasRoot 'data/atlas.sqlite3'
    if (Test-Path -LiteralPath $AtlasReference) { $env:ATLAS_LEGACY_DB = $AtlasReference }
}
if (-not $env:ATLAS_REPORTS_DIR) {
    $AtlasReports = Join-Path (Split-Path $AtlasRoot) 'server map/dashboard/data'
    if (Test-Path -LiteralPath $AtlasReports) { $env:ATLAS_REPORTS_DIR = $AtlasReports }
}
# Debugging is opt-in only in the synthetic proof harness, never this launcher.
Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
# This is the interactive app the owner needs to see, not a background helper.
Start-Process -FilePath $AtlasBinary -WorkingDirectory $AtlasRoot -WindowStyle Normal
