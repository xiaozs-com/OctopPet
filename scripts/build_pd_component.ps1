# Local Windows x64 component build. No installation, keys, catalog merge or upload.
param(
    [Parameter(Mandatory=$true)][string]$BridgeRoot,
    [string]$OutputDirectory = ""
)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed: $LASTEXITCODE" }
}
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Windows x64 is required' }
Invoke-Checked npm.cmd @('run', 'tauri', '--', 'build', '--no-bundle', '--target', 'x86_64-pc-windows-msvc')
$componentRustFlags = $env:RUSTFLAGS
try {
    $env:RUSTFLAGS = (($componentRustFlags + ' -C target-feature=+crt-static').Trim())
    Invoke-Checked cargo @('build', '--release', '--offline', '--locked', '--target', 'x86_64-pc-windows-msvc', '--manifest-path', (Join-Path $BridgeRoot 'native/Cargo.toml'))
} finally {
    $env:RUSTFLAGS = $componentRustFlags
}
$packageArguments = @(
    'scripts/package_pd_component.py',
    '--pet', 'src-tauri/target/x86_64-pc-windows-msvc/release/paldee-pet.exe',
    '--bridge', (Join-Path $BridgeRoot 'native/target/x86_64-pc-windows-msvc/release/pd-device-bridge.exe'),
    '--bridge-root', $BridgeRoot
)
if ($OutputDirectory) { $packageArguments += @('--output', $OutputDirectory) }
Invoke-Checked python $packageArguments
