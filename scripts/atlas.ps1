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
    'contracts-generate' { Run-AtlasCommand 'cargo' @('run', '-p', 'atlas-core', '--bin', 'export-contracts', '--locked', '--', '--write') }
    'contracts-check' { Run-AtlasCommand 'cargo' @('run', '-p', 'atlas-core', '--bin', 'export-contracts', '--locked', '--', '--check') }
    'doctor' { Invoke-AtlasDoctor }
    'check' {
        Run-AtlasCommand 'cargo' @('run', '-p', 'atlas-core', '--bin', 'export-contracts', '--locked', '--', '--check')
        Run-AtlasCommand 'pnpm' @('check')
        Run-AtlasCommand 'cargo' @('fmt', '--all', '--', '--check')
        Run-AtlasCommand 'cargo' @('check', '--workspace', '--locked')
    }
    'test-feature' {
        $Feature = ($TaskArguments | Where-Object { $_ -ne '--' } | Select-Object -First 1)
        if ($Feature -eq 'F01') {
            Invoke-AtlasDoctor
            Run-AtlasCommand 'pnpm' @('check')
        } elseif ($Feature -eq 'F02') {
            Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--locked')
            Run-AtlasCommand 'cargo' @('run', '-p', 'atlas-core', '--bin', 'export-contracts', '--locked', '--', '--check')
            Run-AtlasCommand 'pnpm' @('check')
            Run-AtlasCommand 'node' @('--test', 'ui/client.test.ts')
        } elseif ($Feature -eq 'F03' -or $Feature -eq 'F04') {
            Run-AtlasCommand 'pnpm' @('check')
            Run-AtlasCommand 'node' @('--test', 'ui/client.test.ts', 'ui/map-model.test.ts')
            Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-native', '--lib', '--locked')
            if ($Feature -eq 'F04') {
                Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--lib', 'operations::', '--locked')
                Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--test', 'operations', '--locked')
            }
        } elseif ($Feature -eq 'T032') {
            Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--lib', 'operations::', '--locked')
            Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--test', 'operations', '--locked')
            Run-AtlasCommand 'cargo' @('run', '-p', 'atlas-core', '--bin', 'export-contracts', '--locked', '--', '--check')
            Run-AtlasCommand 'pnpm' @('check')
        } elseif ($Feature -eq 'T019') {
            Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--lib', 'health::', '--locked')
            Run-AtlasCommand 'cargo' @('run', '-p', 'atlas-core', '--bin', 'export-contracts', '--locked', '--', '--check')
            Run-AtlasCommand 'pnpm' @('check')
            Run-AtlasCommand 'node' @('--test', 'ui/map-model.test.ts')
        } elseif ($Feature -eq 'T020') {
            Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--lib', 'inventory::', '--locked')
            Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--test', 'inventory', '--locked')
            Run-AtlasCommand 'cargo' @('run', '-p', 'atlas-core', '--bin', 'export-contracts', '--locked', '--', '--check')
            Run-AtlasCommand 'pnpm' @('check')
            Run-AtlasCommand 'node' @('--test', 'ui/client.test.ts', 'ui/map-model.test.ts')
        } elseif ($Feature -eq 'T022') {
            Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--lib', 'workloads::', '--locked')
            Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--test', 'inventory', '--locked')
            Run-AtlasCommand 'cargo' @('run', '-p', 'atlas-core', '--bin', 'export-contracts', '--locked', '--', '--check')
            Run-AtlasCommand 'pnpm' @('check')
            Run-AtlasCommand 'node' @('--test', 'ui/client.test.ts', 'ui/map-model.test.ts')
        } elseif ($Feature -in @('T033', 'T014', 'T015', 'T016', 'T017')) {
            Run-AtlasCommand 'cargo' @('test', '--workspace', '--locked')
            Run-AtlasCommand 'cargo' @('run', '-p', 'atlas-core', '--bin', 'export-contracts', '--locked', '--', '--check')
            Run-AtlasCommand 'pnpm' @('check')
            Run-AtlasCommand 'node' @('--test', 'ui/client.test.ts', 'ui/map-model.test.ts')
        } else { Write-Error "No implemented test map for feature '$Feature'."; exit 2 }
        Run-AtlasCommand 'python' @('agent/check_tracker.py', '--self-test')
    }
    'test-integration' {
        Run-AtlasCommand 'cargo' @('build', '-p', 'atlas-core', '--bin', 'atlas-data', '--locked')
        Run-AtlasCommand 'python' @('scripts/verify_data.py')
    }
    'check-android-core' {
        $AtlasNdkBins = Join-Path $env:NDK_HOME 'toolchains/llvm/prebuilt/windows-x86_64/bin'
        $AtlasAndroidCompiler = Join-Path $AtlasNdkBins 'aarch64-linux-android24-clang.cmd'
        if (-not (Test-Path -LiteralPath $AtlasAndroidCompiler)) { Write-Error 'Configured Android NDK ARM64 compiler is unavailable.'; exit 2 }
        $env:CC_aarch64_linux_android = $AtlasAndroidCompiler
        $env:AR_aarch64_linux_android = Join-Path $AtlasNdkBins 'llvm-ar.exe'
        $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $AtlasAndroidCompiler
        Run-AtlasCommand 'cargo' @('check', '-p', 'atlas-core', '--target', 'aarch64-linux-android', '--locked')
    }
    'dev-desktop' {
        if (-not $env:ATLAS_LEGACY_DB -and (Test-Path -LiteralPath 'data/atlas.sqlite3')) { $env:ATLAS_LEGACY_DB = Join-Path (Get-Location) 'data/atlas.sqlite3' }
        Run-AtlasCommand 'pnpm' @('tauri', 'dev')
    }
    'start-desktop' { & (Join-Path $PSScriptRoot 'start-desktop.ps1') }
    'test-desktop' { Run-AtlasCommand 'node' @('scripts/verify_desktop.mjs') }
    'ssh-fixture' {
        if (-not (Test-Path -LiteralPath '.tools/ssh-fixture-venv/Scripts/python.exe')) { Run-AtlasCommand 'python' @('-m', 'venv', '.tools/ssh-fixture-venv') }
        Run-AtlasCommand '.tools/ssh-fixture-venv/Scripts/python.exe' @('-m', 'pip', 'install', '--disable-pip-version-check', '-r', 'scripts/ssh-fixture-requirements.txt')
    }
    'test-ssh' {
        Run-AtlasCommand 'cargo' @('build', '-p', 'atlas-core', '--bin', 'atlas-ssh', '--locked')
        Run-AtlasCommand '.tools/ssh-fixture-venv/Scripts/python.exe' @('scripts/verify_ssh.py')
    }
    'test-ssh-native' { Run-AtlasCommand 'node' @('scripts/verify_native_ssh.mjs') }
    'test-files' { Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--test', 'files', '--locked', '--', '--ignored', '--nocapture') }
    'test-elevation' { Run-AtlasCommand 'cargo' @('test', '-p', 'atlas-core', '--test', 'elevation', '--locked', '--', '--ignored', '--nocapture') }
    'test-elevation-native' { Run-AtlasCommand 'node' @('scripts/verify_native_elevation.mjs') }
    'test-health-native' { Run-AtlasCommand 'node' @('scripts/verify_native_health.mjs') }
    'test-inventory-native' {
        Run-AtlasCommand 'podman' @('--connection', 'podman-machine-default', 'build', '--tag', 'localhost/atlas-linux-fixture:20261026', '--file', 'scripts/fixtures/linux/Containerfile', 'scripts/fixtures/linux')
        Run-AtlasCommand 'node' @('scripts/verify_native_inventory.mjs')
    }
    'test-workloads-native' {
        Run-AtlasCommand 'podman' @('--connection', 'podman-machine-default', 'build', '--tag', 'localhost/atlas-linux-fixture:20261026', '--file', 'scripts/fixtures/linux/Containerfile', 'scripts/fixtures/linux')
        Run-AtlasCommand 'node' @('scripts/verify_native_workloads.mjs')
    }
    'test-files-native' { Run-AtlasCommand 'node' @('scripts/verify_native_files.mjs') }
    'test-file-changes-native' { Run-AtlasCommand 'node' @('scripts/verify_native_files.mjs', '--mutations') }
    'dev-android' { Run-AtlasCommand 'pnpm' @('tauri', 'android', 'dev') }
    'smoke-desktop' { Run-AtlasCommand 'pnpm' @('tauri', 'build', '--debug', '--no-bundle', '--', '--locked') }
    'smoke-android' { Run-AtlasCommand 'pnpm' @('tauri', 'android', 'build', '--debug', '--target', 'aarch64', '--apk', '--', '--locked') }
    'build-desktop' { Run-AtlasCommand 'pnpm' @('tauri', 'build', '--bundles', 'nsis', '--', '--locked') }
    'build-android' { Run-AtlasCommand 'pnpm' @('tauri', 'android', 'build', '--target', 'aarch64', '--apk', '--', '--locked') }
    default { Write-Error "Unknown Atlas task '$Action'."; exit 2 }
}
