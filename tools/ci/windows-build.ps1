# Diagnose native test compilation using the caller's Cargo profile and build budget.
param(
    [string] $Cargo = 'cargo',
    [string] $OutputDirectory = 'target/ci/windows-build',
    [ValidateRange(1, 3600)]
    [int] $SampleIntervalSeconds = 15
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$resourceLog = Join-Path (Resolve-Path $OutputDirectory) 'resources.jsonl'
$snapshot = {
    $processes = @(Get-Process -Name cargo, rustc, link, lld-link, rust-lld -ErrorAction SilentlyContinue |
        ForEach-Object {
            try {
                [ordered]@{
                    name = $_.ProcessName
                    pid = $_.Id
                    cpu_seconds = $_.TotalProcessorTime.TotalSeconds
                    working_set_bytes = $_.WorkingSet64
                    private_bytes = $_.PrivateMemorySize64
                }
            } catch { } # A compiler process can exit during a sample.
        })
    $memory = $null
    if ($IsWindows) {
        $os = Get-CimInstance Win32_OperatingSystem
        $memory = [ordered]@{
            physical_total_kib = $os.TotalVisibleMemorySize
            physical_free_kib = $os.FreePhysicalMemory
            virtual_total_kib = $os.TotalVirtualMemorySize
            virtual_free_kib = $os.FreeVirtualMemory
        }
    }
    [ordered]@{
        timestamp = [DateTime]::UtcNow.ToString('o')
        logical_processors = [Environment]::ProcessorCount
        memory = $memory
        disks = @(Get-PSDrive -PSProvider FileSystem | Select-Object Name, Used, Free)
        processes = $processes
    } | ConvertTo-Json -Depth 5 -Compress
}
# Write one sample synchronously so even early compilation failures have evidence.
& $snapshot | Set-Content -Encoding utf8 $resourceLog
# Recreate the script in the worker's session state. Passing the bound ScriptBlock
# itself lets its invocation use the caller's runspace while Cargo is running.
$monitor = Start-ThreadJob -ArgumentList $snapshot.ToString(), $resourceLog, $SampleIntervalSeconds -ScriptBlock {
    param($SnapshotSource, $Log, $Interval)
    $Snapshot = [scriptblock]::Create($SnapshotSource)
    while ($true) {
        Start-Sleep -Seconds $Interval
        & $Snapshot | Add-Content -Encoding utf8 $Log
    }
}
Write-Host "Windows build resource samples: $resourceLog (every $SampleIntervalSeconds seconds)"
$code = 1
try {
    & $Cargo test --workspace --all-targets --all-features --locked --no-run --timings
    $code = $LASTEXITCODE
} finally {
    Stop-Job $monitor
    Receive-Job $monitor -ErrorAction Continue
    Remove-Job $monitor
    Write-Host "Windows test build exit code: $code"
}
exit $code
