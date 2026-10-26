# SPDX-License-Identifier: MIT OR Apache-2.0
# Runs the same checks as CI. Run from anywhere; the script moves to the repository root.
$ErrorActionPreference = 'Stop'

Set-Location (Join-Path $PSScriptRoot '..')

function Invoke-Check([string]$Name, [scriptblock]$Command) {
    Write-Host "==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

Invoke-Check 'cargo fmt --check' { cargo fmt --all --check }
Invoke-Check 'cargo clippy' { cargo clippy --workspace --all-targets -- -D warnings }
Invoke-Check 'cargo test' { cargo test --workspace }
Invoke-Check 'cargo deny check' { cargo deny check }

Write-Host 'All checks passed.'
