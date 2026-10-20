# Exercise the real wrapper with successful and failing Cargo substitutes.
$ErrorActionPreference = 'Stop'
$directory = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory $directory | Out-Null
try {
    foreach ($code in @(0, 7)) {
        $fake = Join-Path $directory "cargo-$code.ps1"
        @'
$expected = @('test', '--workspace', '--all-targets', '--all-features', '--locked', '--no-run', '--timings')
if (($args -join '|') -ne ($expected -join '|')) { exit 99 }
Write-Host 'Mock Cargo diagnostic output'
'@ + "`nif ($code -eq 0) { Start-Sleep -Seconds 18 }`nexit $code`n" | Set-Content $fake
        $output = Join-Path $directory "output-$code"
        $messages = & (Join-Path $PSHOME 'pwsh') -NoProfile -File "$PSScriptRoot/windows-build.ps1" `
            -Cargo $fake -OutputDirectory $output 2>&1
        if ($LASTEXITCODE -ne $code) {
            throw "Cargo exit code $code was not preserved: $messages"
        }
        if (($messages -join "`n") -notmatch 'Mock Cargo diagnostic output') {
            throw 'Cargo output was hidden'
        }
        $samples = @(Get-Content (Join-Path $output 'resources.jsonl') | ConvertFrom-Json)
        if ($samples.Count -lt 1 -or $samples[0].logical_processors -lt 1) {
            throw 'Initial resource sample missing'
        }
        if ($code -eq 0 -and $samples.Count -lt 2) {
            throw 'Periodic resource sampler did not run during compilation'
        }
        if ($IsWindows -and $samples[0].memory.physical_total_kib -le 0) {
            throw 'Windows memory sample missing'
        }
    }
    Write-Host 'PASS: build diagnostics preserve success, failure, output and resource evidence'
} finally {
    # Only this test's uniquely allocated directory is owned by this cleanup.
    Remove-Item -Recurse -Force $directory
}
# Actions' PowerShell launcher propagates LASTEXITCODE from native children.
# The deliberate failing mock must not become this successful self-test's status.
exit 0
