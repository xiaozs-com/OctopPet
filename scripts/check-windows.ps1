# Windows equivalent of make all; invoked by the normal Git hook.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
Invoke-Checked cargo @('fmt', '--manifest-path', 'src-tauri/Cargo.toml')
Invoke-Checked npm.cmd @('exec', '--', 'prettier', '--write', '.')
Invoke-Checked cargo @('fmt', '--manifest-path', 'src-tauri/Cargo.toml', '--check')
Invoke-Checked cargo @('clippy', '--offline', '--manifest-path', 'src-tauri/Cargo.toml', '--', '-D', 'warnings')
Invoke-Checked npm.cmd @('exec', '--', 'prettier', '--check', '.')
Invoke-Checked npm.cmd @('run', 'lint')
Invoke-Checked npm.cmd @('exec', '--', 'tsc', '--noEmit')
Invoke-Checked cargo @('check', '--offline', '--manifest-path', 'src-tauri/Cargo.toml')
Invoke-Checked npm.cmd @('test')
Invoke-Checked cargo @('test', '--offline', '--manifest-path', 'src-tauri/Cargo.toml')
