# Keep this a plain script: advanced PowerShell parameter binding consumes a
# literal `--` intended for Cargo/Tauri instead of forwarding it to the tool.
if ($args.Count -eq 0) { [Console]::Error.WriteLine('Provide an Atlas task action.'); exit 2 }
$Action = [string]$args[0]
$TaskArguments = @($args | Select-Object -Skip 1)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path $PSScriptRoot)

# The desktop's inherited Windows PATH can keep system Node/Python before
# mise's entries even inside mise exec. Resolve the active bins through mise,
# then put them first for this process and all native/frontend child processes.
$AtlasToolPaths = @(& mise bin-paths)
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$env:Path = ($AtlasToolPaths -join [IO.Path]::PathSeparator) + [IO.Path]::PathSeparator + $env:Path

function Run-AtlasCommand([string]$Command, [string[]]$Arguments) {
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

function Invoke-AtlasDoctor {
    $env:ATLAS_PNPM_VERSION = (& pnpm --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Run-AtlasCommand 'node' @('scripts/doctor.mjs')
}

switch ($Action) {
    'exec' {
        $AtlasArguments = @($TaskArguments)
        if ($AtlasArguments.Count -gt 0 -and $AtlasArguments[0] -eq '--') { $AtlasArguments = @($AtlasArguments | Select-Object -Skip 1) }
        if ($AtlasArguments.Count -eq 0) { Write-Error 'Provide a command after exec.'; exit 2 }
        Run-AtlasCommand $AtlasArguments[0] @($AtlasArguments | Select-Object -Skip 1)
    }
    'deps' { Run-AtlasCommand 'pnpm' @('install', '--frozen-lockfile') }
    'tracker-check' { Run-AtlasCommand 'python' @('agent/check_tracker.py', '--self-test') }
    'doctor' { Invoke-AtlasDoctor }
    'check' {
        Run-AtlasCommand 'pnpm' @('check')
        Run-AtlasCommand 'cargo' @('check', '--workspace', '--locked')
    }
    'test-feature' {
        $Feature = ($TaskArguments | Where-Object { $_ -ne '--' } | Select-Object -First 1)
        if ($Feature -ne 'F01') { Write-Error "No implemented test map for feature '$Feature'."; exit 2 }
        Invoke-AtlasDoctor
        Run-AtlasCommand 'pnpm' @('check')
        Run-AtlasCommand 'python' @('agent/check_tracker.py', '--self-test')
    }
    'test-integration' { Write-Error 'Domain integration suite is not implemented yet (F02 and later).'; exit 2 }
    'dev-desktop' { Run-AtlasCommand 'pnpm' @('tauri', 'dev') }
    'dev-android' { Run-AtlasCommand 'pnpm' @('tauri', 'android', 'dev') }
    'smoke-desktop' { Run-AtlasCommand 'pnpm' @('tauri', 'build', '--debug', '--no-bundle', '--', '--locked') }
    'smoke-android' { Run-AtlasCommand 'pnpm' @('tauri', 'android', 'build', '--debug', '--target', 'aarch64', '--apk', '--', '--locked') }
    'build-desktop' { Run-AtlasCommand 'pnpm' @('tauri', 'build', '--bundles', 'nsis', '--', '--locked') }
    'build-android' { Run-AtlasCommand 'pnpm' @('tauri', 'android', 'build', '--target', 'aarch64', '--apk', '--', '--locked') }
    default { Write-Error "Unknown Atlas task '$Action'."; exit 2 }
}
