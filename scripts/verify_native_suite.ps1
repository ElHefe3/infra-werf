param([string]$StartAt = 'desktop')
$ErrorActionPreference = 'Stop'
if (-not $env:ATLAS_DESKTOP_BINARY -or -not (Test-Path -LiteralPath $env:ATLAS_DESKTOP_BINARY)) {
    throw 'Set ATLAS_DESKTOP_BINARY to the exact release executable being verified.'
}
$proofRoot = Join-Path (Get-Location) '.tools/full-desktop-proof'
New-Item -ItemType Directory -Force $proofRoot | Out-Null
$steps = @(
    @{ Name='desktop'; File='scripts/verify_desktop.mjs'; Arguments=@() },
    @{ Name='ssh'; File='scripts/verify_native_ssh.mjs'; Arguments=@() },
    @{ Name='health'; File='scripts/verify_native_health.mjs'; Arguments=@() },
    @{ Name='inventory'; File='scripts/verify_native_inventory.mjs'; Arguments=@() },
    @{ Name='files'; File='scripts/verify_native_files.mjs'; Arguments=@() },
    @{ Name='file-changes'; File='scripts/verify_native_files.mjs'; Arguments=@('--mutations') },
    @{ Name='terminal'; File='scripts/verify_native_terminal.mjs'; Arguments=@() },
    @{ Name='elevation'; File='scripts/verify_native_elevation.mjs'; Arguments=@() },
    @{ Name='workloads'; File='scripts/verify_native_workloads.mjs'; Arguments=@() },
    @{ Name='workload-control'; File='scripts/verify_native_workload_control.mjs'; Arguments=@() },
    @{ Name='monitoring'; File='scripts/verify_native_monitoring.mjs'; Arguments=@() },
    @{ Name='apps'; File='scripts/verify_native_apps.mjs'; Arguments=@() },
    @{ Name='packs'; File='scripts/verify_native_packs.mjs'; Arguments=@() },
    @{ Name='pack-map'; File='scripts/verify_native_pack_map.mjs'; Arguments=@() }
)
if ($steps.Name -notcontains $StartAt) { throw 'Unknown starting suite.' }
$started = $false
$results = @()
foreach ($step in $steps) {
    if ($step.Name -eq $StartAt) { $started = $true }
    if (-not $started) { continue }
    Write-Output "Testing installed release: $($step.Name)"
    $stepLog = Join-Path $proofRoot "native-$($step.Name).log"
    # Each harness owns one synthetic database and its loopback fixture. Native
    # suites are sequential because Windows single-instance and CDP are shared.
    # Windows PowerShell 5 treats stderr text as an ErrorRecord even when a
    # child exits successfully. The native exit code determines test success.
    $ErrorActionPreference = 'Continue'
    & node $step.File @($step.Arguments) *> $stepLog
    $stepExit = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    $results += [pscustomobject]@{ Suite=$step.Name; ExitCode=$stepExit; Log=$stepLog }
    $results | ConvertTo-Json -Depth 3 | Set-Content (Join-Path $proofRoot "native-$StartAt-summary.json")
    if ($stepExit -ne 0) {
        Get-Content -LiteralPath $stepLog -Tail 35
        exit $stepExit
    }
    Write-Output "Passed: $($step.Name)"
}
Write-Output 'All selected native desktop suites passed.'
