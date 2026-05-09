<#
.SYNOPSIS
    Run database migrations for Open Tappd.

.DESCRIPTION
    Applies all SQLx migrations from the migrations/ folder to the PostgreSQL
    database. Uses sqlx-cli if installed, otherwise falls back to running the
    SQL files directly via psql inside the container.

.EXAMPLE
    .\scripts\migrate-db.ps1
    .\scripts\migrate-db.ps1 -Force   # re-apply all migrations
#>
param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$ContainerName = "opentappd-db"
$DbName        = "opentappd"
$DbUser        = "opentappd"
$MigrationsDir = Join-Path $PSScriptRoot "..\migrations"

# ── Load .env if available ────────────────────────────────────────
$envFile = Join-Path $PSScriptRoot "..\.env"
if (Test-Path $envFile) {
    Get-Content $envFile | ForEach-Object {
        if ($_ -match '^\s*([^#][^=]+)=(.*)$') {
            $key = $matches[1].Trim()
            $val = $matches[2].Trim()
            [System.Environment]::SetEnvironmentVariable($key, $val, "Process")
        }
    }
    Write-Host "Loaded .env" -ForegroundColor DarkGray
}

# ── Check container is running ────────────────────────────────────
$running = podman ps --filter "name=$ContainerName" --format "{{.Names}}" 2>$null
if ($running -ne $ContainerName) {
    Write-Host "Database container '$ContainerName' is not running." -ForegroundColor Red
    Write-Host "Run '.\scripts\start-db.ps1' first." -ForegroundColor Yellow
    exit 1
}

# ── Try sqlx-cli first ───────────────────────────────────────────
$hasSqlx = Get-Command sqlx -ErrorAction SilentlyContinue
if ($hasSqlx) {
    Write-Host "Running migrations with sqlx-cli..." -ForegroundColor Cyan
    $dbUrl = $env:DATABASE_URL
    if (-not $dbUrl) {
        $dbUrl = "postgres://${DbUser}:opentappd_dev@localhost:5432/${DbName}"
    }
    $env:DATABASE_URL = $dbUrl
    Push-Location (Join-Path $PSScriptRoot "..")
    sqlx migrate run --source migrations
    Pop-Location

    if ($LASTEXITCODE -eq 0) {
        Write-Host "Migrations applied successfully with sqlx-cli!" -ForegroundColor Green
        exit 0
    } else {
        Write-Host "sqlx-cli failed, falling back to psql..." -ForegroundColor Yellow
    }
}

# ── Fallback: run SQL files via psql in the container ─────────────
Write-Host "Running migrations via psql in container..." -ForegroundColor Cyan

# Get sorted migration files
$migrationFiles = Get-ChildItem -Path $MigrationsDir -Filter "*.sql" | Sort-Object Name

if ($migrationFiles.Count -eq 0) {
    Write-Host "No migration files found in $MigrationsDir" -ForegroundColor Yellow
    exit 0
}

# Create a tracking table so we don't re-run migrations
$createTracker = @"
CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
    success BOOLEAN NOT NULL DEFAULT true,
    checksum BYTEA NOT NULL DEFAULT '\x00',
    execution_time BIGINT NOT NULL DEFAULT 0
);
"@

podman exec -i $ContainerName psql -U $DbUser -d $DbName -c $createTracker 2>$null

foreach ($file in $migrationFiles) {
    # Extract version number from filename (e.g., 20260508000001 from 20260508000001_initial_schema.sql)
    if ($file.Name -match '^(\d+)_(.+)\.sql$') {
        $version = [long]$matches[1]
        $description = $matches[2] -replace '_', ' '
    } else {
        Write-Host "  Skipping $($file.Name) (unexpected filename format)" -ForegroundColor Yellow
        continue
    }

    # Check if already applied
    if (-not $Force) {
        $check = podman exec $ContainerName psql -U $DbUser -d $DbName -t -c "SELECT COUNT(*) FROM _sqlx_migrations WHERE version = $version;" 2>$null
        if ($check -and $check.Trim() -gt "0") {
            Write-Host "  skip $($file.Name) (already applied)" -ForegroundColor DarkGray
            continue
        }
    }

    Write-Host "  Applying $($file.Name)..." -ForegroundColor Cyan
    $sql = Get-Content $file.FullName -Raw

    # Run the migration
    $sql | podman exec -i $ContainerName psql -U $DbUser -d $DbName -v ON_ERROR_STOP=1

    if ($LASTEXITCODE -ne 0) {
        Write-Host "  FAILED: $($file.Name)" -ForegroundColor Red
        exit 1
    }

    # Record it
    $escapedDesc = $description -replace "'", "''"
    $record = "INSERT INTO _sqlx_migrations (version, description) VALUES ($version, '$escapedDesc') ON CONFLICT (version) DO NOTHING;"
    podman exec $ContainerName psql -U $DbUser -d $DbName -c $record 2>$null

    Write-Host "  done  $($file.Name)" -ForegroundColor Green
}

Write-Host ""
Write-Host "All migrations applied!" -ForegroundColor Green
Write-Host ""
Write-Host "You can now start the API server:" -ForegroundColor Yellow
Write-Host "  cargo run -p open-tappd-api" -ForegroundColor White
