$ErrorActionPreference = 'Stop'

function Assert-True {
    param(
        [bool] $Condition,
        [string] $Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$scriptPath = Join-Path $repositoryRoot 'scripts\build-windows-release.ps1'
$script = Get-Content -LiteralPath $scriptPath -Raw -Encoding UTF8

Assert-True (
    $script -match '\[Environment\]::OSVersion\.Platform' -and
    $script -match '\[PlatformID\]::Win32NT'
) 'Windows release packaging must detect the host platform instead of requiring an optional OS environment variable.'
Assert-True (
    $script -notmatch '\$env:OS\s*-ne\s*["'']Windows_NT["'']'
) 'Windows release packaging must not reject a valid Windows host when OS is unset.'
Assert-True (
    $script -match '\$TauriConfigPath' -and
    $script -match '\[System\.IO\.File\]::WriteAllText' -and
    $script -match '"--config",\s*\$TauriConfigPath'
) 'Windows release packaging must pass Tauri configuration through a temporary JSON file so PowerShell preserves JSON quotes.'
Assert-True (
    $script -match '\$TauriCliVersion\s*=\s*["'']2\.6\.2["'']' -and
    $script -match 'tauri-cli.*\$TauriCliVersion' -and
    $script -match '--force'
) 'Windows release packaging must install the pinned Tauri CLI version when the preinstalled CLI differs.'

Write-Output 'Windows release script platform guard passed.'
