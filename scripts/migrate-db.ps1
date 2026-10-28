<#
.SYNOPSIS
    Apply database migrations using the application's SQLx migrator.
.DESCRIPTION
    DATABASE_URL must be set in the process environment. For local development,
    explicitly select a dotenv file with -EnvFile; existing environment values
    take precedence. No default credentials, container runtime, SQL replay, or
    alternate migration tracker is used. Stop application writers and take a
    restorable backup before applying migrations to an existing database.
.EXAMPLE
    .\scripts\migrate-db.ps1 -EnvFile .env
.EXAMPLE
    .\scripts\migrate-db.ps1
#>
[CmdletBinding()]
param(
    [string]$EnvFile
)

$ErrorActionPreference = "Stop"

# Resolve an explicitly selected file relative to the caller before changing cwd.
$migrationArgs = @()
if ($EnvFile) {
    $resolvedEnvFile = (Resolve-Path -LiteralPath $EnvFile).Path
    $migrationArgs = @("--", "--env-file", $resolvedEnvFile)
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    Write-Host "Applying migrations with SQLx..." -ForegroundColor Cyan
    & cargo run --locked -p open-tappd-api --bin migrate @migrationArgs
    $migrationExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}

if ($migrationExitCode -ne 0) {
    Write-Error "Migration command failed. No fallback or automatic replay was attempted."
    exit $migrationExitCode
}
Write-Host "Migrations applied successfully." -ForegroundColor Green
