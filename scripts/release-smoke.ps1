param(
    [switch]$SkipSecurity
)

$ErrorActionPreference = "Stop"

function Run-Step {
    param(
        [string]$Name,
        [string]$Command
    )
    Write-Host ""
    Write-Host "==> $Name" -ForegroundColor Cyan
    Write-Host "    $Command"
    Invoke-Expression $Command
}

Write-Host "FresnelFir release smoke run starting..." -ForegroundColor Green

if (-not $SkipSecurity) {
    Write-Host ""
    Write-Host "==> Security checks (best-effort tooling discovery)" -ForegroundColor Cyan

    if (Get-Command cargo-audit -ErrorAction SilentlyContinue) {
        Run-Step -Name "cargo audit" -Command "cargo audit"
    } elseif (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Host "    cargo-audit not installed; skipping dependency audit." -ForegroundColor Yellow
    }

    if (Get-Command gitleaks -ErrorAction SilentlyContinue) {
        Run-Step -Name "gitleaks working-tree scan" -Command "gitleaks detect --no-git --source ."
    } else {
        Write-Host "    gitleaks not installed; skipping secret scan." -ForegroundColor Yellow
    }
}

Run-Step -Name "Format check" -Command "cargo fmt --all -- --check"
Run-Step -Name "Clippy strict check" -Command "cargo clippy --workspace --all-targets --locked -- -D warnings"
Run-Step -Name "Workspace tests" -Command "cargo test --workspace --locked"
Run-Step -Name "Workspace build" -Command "cargo build --workspace --locked"

Write-Host ""
Write-Host "Release smoke run completed successfully." -ForegroundColor Green
