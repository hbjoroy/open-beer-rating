<#
.SYNOPSIS
    Start the Open Tappd PostgreSQL database using Podman.

.DESCRIPTION
    Creates and starts a PostgreSQL 16 container with the correct credentials.
    If the container already exists, it is restarted.
    Also generates .env with real keys if it doesn't exist yet.

.EXAMPLE
    .\scripts\start-db.ps1
    .\scripts\start-db.ps1 -Reset   # destroy volume and start fresh
#>
param(
    [switch]$Reset
)

$ErrorActionPreference = "Stop"

$ContainerName = "opentappd-db"
$VolumeName    = "opentappd-pgdata"
$ImageName     = "docker.io/library/postgres:16-alpine"
$DbName        = "opentappd"
$DbUser        = "opentappd"
$DbPassword    = "opentappd_dev"
$HostPort      = 5432

# ── Reset (optional) ──────────────────────────────────────────────
if ($Reset) {
    Write-Host "Resetting database..." -ForegroundColor Yellow
    podman rm -f $ContainerName 2>$null
    podman volume rm $VolumeName 2>$null
    Write-Host "Old container and volume removed."
}

# ── Ensure podman machine is running ──────────────────────────────
$machineState = podman machine info --format "{{.Host.MachineState}}" 2>$null
if ($LASTEXITCODE -ne 0 -or $machineState -ne "Running") {
    Write-Host "Starting podman machine..." -ForegroundColor Cyan
    podman machine start 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "No podman machine found. Initializing one..." -ForegroundColor Cyan
        podman machine init
        podman machine start
    }
}

# ── Check if container already exists ─────────────────────────────
$existing = podman ps -a --filter "name=$ContainerName" --format "{{.Names}}" 2>$null
if ($existing -eq $ContainerName) {
    $running = podman ps --filter "name=$ContainerName" --format "{{.Names}}" 2>$null
    if ($running -eq $ContainerName) {
        Write-Host "Container '$ContainerName' is already running." -ForegroundColor Green
    } else {
        Write-Host "Starting existing container '$ContainerName'..." -ForegroundColor Cyan
        podman start $ContainerName
    }
} else {
    # ── Create volume and container ───────────────────────────────
    Write-Host "Creating volume '$VolumeName'..." -ForegroundColor Cyan
    podman volume create $VolumeName 2>$null

    Write-Host "Starting PostgreSQL container..." -ForegroundColor Cyan
    podman run -d `
        --name $ContainerName `
        -e POSTGRES_DB=$DbName `
        -e POSTGRES_USER=$DbUser `
        -e POSTGRES_PASSWORD=$DbPassword `
        -p "${HostPort}:5432" `
        -v "${VolumeName}:/var/lib/postgresql/data" `
        --health-cmd "pg_isready -U $DbUser" `
        --health-interval 5s `
        --health-timeout 5s `
        --health-retries 5 `
        $ImageName

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Failed to start container." -ForegroundColor Red
        exit 1
    }
}

# ── Wait for PostgreSQL to be ready ───────────────────────────────
Write-Host "Waiting for PostgreSQL to be ready..." -ForegroundColor Cyan
$maxRetries = 30
for ($i = 0; $i -lt $maxRetries; $i++) {
    $health = podman inspect $ContainerName --format "{{.State.Health.Status}}" 2>$null
    if ($health -eq "healthy") {
        Write-Host "PostgreSQL is ready!" -ForegroundColor Green
        break
    }
    Start-Sleep -Seconds 2
    Write-Host "  waiting... ($($i+1)/$maxRetries)"
}

if ($health -ne "healthy") {
    Write-Host "PostgreSQL did not become healthy in time." -ForegroundColor Red
    podman logs $ContainerName
    exit 1
}

# ── Generate .env if missing ─────────────────────────────────────
$envFile = Join-Path $PSScriptRoot "..\.env"
if (-not (Test-Path $envFile)) {
    Write-Host "Generating .env file with secure keys..." -ForegroundColor Cyan

    # Generate random keys using PowerShell
    $encKeyBytes = [byte[]]::new(32)
    [System.Security.Cryptography.RandomNumberGenerator]::Fill($encKeyBytes)
    $encKey = [Convert]::ToBase64String($encKeyBytes)

    $jwtBytes = [byte[]]::new(48)
    [System.Security.Cryptography.RandomNumberGenerator]::Fill($jwtBytes)
    $jwtSecret = [Convert]::ToBase64String($jwtBytes)

    @"
# Database
DATABASE_URL=postgres://${DbUser}:${DbPassword}@localhost:${HostPort}/${DbName}
POSTGRES_DB=$DbName
POSTGRES_USER=$DbUser
POSTGRES_PASSWORD=$DbPassword
POSTGRES_PORT=$HostPort

# Encryption (auto-generated 32-byte key)
ENCRYPTION_KEY=$encKey

# JWT (auto-generated secret)
JWT_SECRET=$jwtSecret

# Server
API_HOST=0.0.0.0
API_PORT=3000
RUST_LOG=info
"@ | Set-Content $envFile -Encoding UTF8

    Write-Host ".env created with auto-generated secrets." -ForegroundColor Green
} else {
    Write-Host ".env already exists, skipping generation." -ForegroundColor DarkGray
}

# ── Print connection info ─────────────────────────────────────────
Write-Host ""
Write-Host "════════════════════════════════════════════" -ForegroundColor DarkCyan
Write-Host "  Database:  $DbName"
Write-Host "  User:      $DbUser"
Write-Host "  Port:      $HostPort"
Write-Host "  URL:       postgres://${DbUser}:${DbPassword}@localhost:${HostPort}/${DbName}"
Write-Host "════════════════════════════════════════════" -ForegroundColor DarkCyan
Write-Host ""
Write-Host "Next: run '.\scripts\migrate-db.ps1' to create tables." -ForegroundColor Yellow
