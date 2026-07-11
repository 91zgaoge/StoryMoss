# StoryMoss Development Runner for Windows
Write-Host "Starting StoryMoss (草苔) Development Environment..." -ForegroundColor Green

# Check if Node.js is installed
$nodeVersion = node --version 2>$null
if (-not $nodeVersion) {
    Write-Host "Error: Node.js is not installed. Please install Node.js 18+ first." -ForegroundColor Red
    exit 1
}
Write-Host "Node.js version: $nodeVersion" -ForegroundColor Cyan

# Check if Rust is installed
$rustVersion = rustc --version 2>$null
if (-not $rustVersion) {
    Write-Host "Error: Rust is not installed. Please install Rust 1.70+ first." -ForegroundColor Red
    exit 1
}
Write-Host "Rust version: $rustVersion" -ForegroundColor Cyan

# Check if Tauri CLI is installed
$tauriVersion = cargo tauri --version 2>$null
if (-not $tauriVersion) {
    Write-Host "Installing Tauri CLI..." -ForegroundColor Yellow
    cargo install tauri-cli
}
Write-Host "Tauri CLI version: $tauriVersion" -ForegroundColor Cyan

# Install frontend dependencies if needed
if (-not (Test-Path "src-frontend/node_modules")) {
    Write-Host "Installing frontend dependencies..." -ForegroundColor Yellow
    cd src-frontend
    npm install
    cd ..
}

# Start development server
Write-Host "Starting Tauri development server..." -ForegroundColor Green
Write-Host "The app window should open automatically." -ForegroundColor Cyan
Write-Host "Press Ctrl+C to stop." -ForegroundColor Gray

cd src-tauri
cargo tauri dev
