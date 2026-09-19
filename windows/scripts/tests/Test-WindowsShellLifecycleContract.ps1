$ErrorActionPreference = 'Stop'

function Assert-True {
  param([bool] $Condition, [string] $Message)
  if (-not $Condition) {
    throw $Message
  }
}

function Assert-Match {
  param([string] $Text, [string] $Pattern, [string] $Message)
  Assert-True ($Text -match $Pattern) $Message
}

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$mainSource = Join-Path $repositoryRoot 'windows\apps\codexu-tauri\src-tauri\src\main.rs'
$traySource = Join-Path $repositoryRoot 'windows\apps\codexu-tauri\src-tauri\src\tray.rs'
$settingsCommandSource = Join-Path $repositoryRoot 'windows\apps\codexu-tauri\src-tauri\src\commands\settings.rs'
$headerSource = Join-Path $repositoryRoot 'windows\apps\codexu-tauri\web\src\components\Header.tsx'
$settingsViewSource = Join-Path $repositoryRoot 'windows\apps\codexu-tauri\web\src\windows\Settings.tsx'
$lifecycleEntry = Join-Path $repositoryRoot 'windows\scripts\Test-WindowsShellLifecycle.ps1'

foreach ($path in @($mainSource, $traySource, $settingsCommandSource, $headerSource, $settingsViewSource)) {
  Assert-True (Test-Path -LiteralPath $path -PathType Leaf) "Expected source file is missing: $path"
}

$mainText = Get-Content -LiteralPath $mainSource -Raw -Encoding UTF8
$trayText = Get-Content -LiteralPath $traySource -Raw -Encoding UTF8
$settingsCommandText = Get-Content -LiteralPath $settingsCommandSource -Raw -Encoding UTF8
$headerText = Get-Content -LiteralPath $headerSource -Raw -Encoding UTF8
$settingsViewText = Get-Content -LiteralPath $settingsViewSource -Raw -Encoding UTF8

Assert-Match `
  $mainText `
  'WindowEvent::CloseRequested[\s\S]*tray::hide_to_tray\(&window_clone\)[\s\S]*api\.prevent_close\(\)' `
  'Main window close must remain close-to-tray rather than process exit.'

Assert-Match `
  $trayText `
  'WebviewWindowBuilder::from_config[\s\S]*\.build\(\)[\s\S]*window[\s\S]*\.show\(\)[\s\S]*window[\s\S]*\.set_focus\(\)' `
  'Tray open must show and focus a rebuilt main window, not only build it.'
Assert-True (
  $trayText -notmatch 'panic!\(|unwrap_or_else\(\|_\|\s*panic!'
) 'Tray open rebuild failures must be logged/returned without panicking the app.'

Assert-Match `
  $settingsCommandText `
  'WebviewWindowBuilder::new\(&app, "settings"[\s\S]*\.build\(\)[\s\S]*window\.show\(\)[\s\S]*window\.set_focus\(\)' `
  'Settings launch must explicitly focus a newly-created Settings window.'

Assert-Match `
  $headerText `
  'aria-label=\{t\(''common\.refresh''\)\}' `
  'Header Refresh action must expose a stable native-accessible label.'

Assert-Match `
  $headerText `
  'aria-label=\{t\(''common\.settings''\)\}' `
  'Header Settings action must expose a stable native-accessible label.'

foreach ($id in @(
  'settings-window-root',
  'settings-section-data-paths',
  'settings-section-appearance',
  'settings-section-tray',
  'settings-section-refresh',
  'settings-section-about',
  'settings-refresh-now',
  'settings-clear-cache'
)) {
  Assert-True (
    $settingsViewText.Contains("id=`"$id`"")
  ) "Settings native workflow requires a stable element id: $id"
}

Assert-True (Test-Path -LiteralPath $lifecycleEntry -PathType Leaf) 'The shell lifecycle workflow entry point is missing.'
$lifecycleText = Get-Content -LiteralPath $lifecycleEntry -Raw -Encoding UTF8
Assert-Match `
  $lifecycleText `
  'Test-RecordedIdentity[\s\S]*creation_utc[\s\S]*executable_path' `
  'Shell lifecycle cleanup must verify PID identity by creation time and executable path before stopping.'
Assert-True (
  $lifecycleText -match 'root_identity_persisted'
) 'Shell lifecycle must persist the root process identity before timeout-sensitive work.'
Assert-True (
  $lifecycleText -match 'cleanup_status[\s\S]*not-confirmed[\s\S]*unknown'
) 'Shell lifecycle cleanup must distinguish confirmed cleanup from not-confirmed/unknown identity outcomes.'
$stopFunctionStart = $lifecycleText.IndexOf('function Stop-RecordedTaskProcesses')
$stopFunctionEnd = $lifecycleText.IndexOf('function Get-ExactExecutableProcesses')
Assert-True ($stopFunctionStart -ge 0 -and $stopFunctionEnd -gt $stopFunctionStart) 'Cleanup function boundaries must remain inspectable.'
$stopFunctionText = $lifecycleText.Substring($stopFunctionStart, $stopFunctionEnd - $stopFunctionStart)
Assert-True (
  $stopFunctionText -notmatch 'Update-TaskProcessRecords'
) 'Cleanup must use only identities captured before cleanup; it must not rediscover PIDs.'
Assert-Match `
  $stopFunctionText `
  'Get-ProcessIdentity[\s\S]*Test-RecordedIdentity[\s\S]*Stop-Process' `
  'Cleanup must validate the captured process identity immediately before termination.'
Assert-Match `
  $lifecycleText `
  'current_stage' `
  'Shell lifecycle workflow must record current_stage before blocking native steps.'
Assert-Match `
  $lifecycleText `
  'Start-Watchdog|Invoke-Watchdog|SelfTestTimeoutCleanup' `
  'Shell lifecycle workflow needs a watchdog or equivalent timeout cleanup path.'

$powershell = (Get-Command powershell.exe -ErrorAction Stop).Source
$preflightOutput = @(
  & $powershell -NoProfile -ExecutionPolicy Bypass -File $lifecycleEntry -PreflightOnly 2>&1
)
Assert-True ($LASTEXITCODE -eq 0) 'Shell lifecycle preflight must execute successfully.'
$preflightLine = @(
  $preflightOutput | Where-Object { "$_".StartsWith('WINDOWS_SHELL_LIFECYCLE_PREFLIGHT=') }
)
Assert-True ($preflightLine.Count -eq 1) 'Shell lifecycle preflight must emit exactly one manifest line.'
$preflight = "$($preflightLine[0])".Substring('WINDOWS_SHELL_LIFECYCLE_PREFLIGHT='.Length) |
  ConvertFrom-Json
foreach ($required in @(
  'close-to-tray',
  'settings-open',
  'settings-refresh-smoke',
  'refresh',
  'maximize-dpi',
  'quit-cleanup',
  'identity-mismatch-protected'
)) {
  Assert-True (
    @($preflight.coverage) -contains $required
  ) "Shell lifecycle preflight must declare behavior coverage for $required."
}
Assert-True (
  @($preflight.coverage) -notcontains 'settings-save-success'
) 'Settings refresh smoke must not be reported as settings-save-success.'
foreach ($notObserved in @(
  'tray-left-click-open',
  'tray-menu-settings',
  'tray-menu-refresh',
  'tray-menu-quit',
  'settings-save-error'
)) {
  Assert-True (
    @($preflight.not_observed_by_workflow) -contains $notObserved
  ) "Shell lifecycle preflight must explicitly mark $notObserved as NOT OBSERVED."
}

$watchdogOutputRoot = Join-Path $repositoryRoot (
  '.local-artifacts\windows-shell-lifecycle\watchdog-contract-' +
  [guid]::NewGuid().ToString('N')
)
$watchdogOutput = @(
  & $powershell -NoProfile -ExecutionPolicy Bypass -File $lifecycleEntry `
    -SelfTestTimeoutCleanup `
    -WorkflowTimeoutSeconds 2 `
    -OutputRoot $watchdogOutputRoot 2>&1
)
Assert-True ($LASTEXITCODE -eq 0) "Watchdog self-test failed: $($watchdogOutput -join "`n")"
$watchdogManifestPath = Join-Path $watchdogOutputRoot 'manifest.json'
Assert-True (Test-Path -LiteralPath $watchdogManifestPath -PathType Leaf) 'Watchdog self-test did not write its manifest.'
$watchdogManifest = Get-Content -LiteralPath $watchdogManifestPath -Raw -Encoding UTF8 |
  ConvertFrom-Json
Assert-True (
  $watchdogManifest.status -eq 'timeout-cleanup-confirmed'
) 'Watchdog self-test must record timeout-cleanup-confirmed.'
Assert-True (
  $watchdogManifest.final_process_cleanup -eq 'confirmed'
) 'Watchdog self-test must identity-clean task-owned timeout processes.'

$identityMismatchOutputRoot = Join-Path $repositoryRoot (
  '.local-artifacts\windows-shell-lifecycle\identity-mismatch-contract-' +
  [guid]::NewGuid().ToString('N')
)
$identityMismatchOutput = @(
  & $powershell -NoProfile -ExecutionPolicy Bypass -File $lifecycleEntry `
    -SelfTestIdentityMismatch `
    -OutputRoot $identityMismatchOutputRoot 2>&1
)
Assert-True ($LASTEXITCODE -eq 0) "Identity mismatch self-test failed: $($identityMismatchOutput -join "`n")"
$identityMismatchManifest = Get-Content -LiteralPath (Join-Path $identityMismatchOutputRoot 'manifest.json') -Raw -Encoding UTF8 |
  ConvertFrom-Json
Assert-True (
  $identityMismatchManifest.final_process_cleanup -eq 'not-confirmed'
) 'PID reuse/identity mismatch must never be reported as confirmed cleanup.'
Assert-True (
  @($identityMismatchManifest.cleanup.identity_mismatch_process_ids).Count -eq 1
) 'PID reuse/identity mismatch self-test must record the protected process identity mismatch.'

$identityCaptureGapOutputRoot = Join-Path $repositoryRoot (
  '.local-artifacts\windows-shell-lifecycle\identity-capture-gap-contract-' +
  [guid]::NewGuid().ToString('N')
)
$identityCaptureGapOutput = @(
  & $powershell -NoProfile -ExecutionPolicy Bypass -File $lifecycleEntry `
    -SelfTestIdentityCaptureGap `
    -OutputRoot $identityCaptureGapOutputRoot 2>&1
)
Assert-True ($LASTEXITCODE -eq 0) "Identity capture gap self-test failed: $($identityCaptureGapOutput -join "`n")"
$identityCaptureGapManifest = Get-Content -LiteralPath (Join-Path $identityCaptureGapOutputRoot 'manifest.json') -Raw -Encoding UTF8 |
  ConvertFrom-Json
Assert-True (
  $identityCaptureGapManifest.final_process_cleanup -eq 'unknown'
) 'Missing persisted root identity must be reported as unknown cleanup, never confirmed.'

$parentTimeoutOutputRoot = Join-Path $repositoryRoot (
  '.local-artifacts\windows-shell-lifecycle\parent-timeout-contract-' +
  [guid]::NewGuid().ToString('N')
)
$parentTimeoutOutput = @(
  & $powershell -NoProfile -ExecutionPolicy Bypass -File $lifecycleEntry `
    -SelfTestParentTimeout `
    -WorkflowTimeoutSeconds 2 `
    -OutputRoot $parentTimeoutOutputRoot 2>&1
)
Assert-True (
  $LASTEXITCODE -eq 2
) "Parent timeout self-test must exit 2 with structured timeout output: $($parentTimeoutOutput -join "`n")"
$incompleteLine = @(
  $parentTimeoutOutput | Where-Object { "$($_)".StartsWith('WINDOWS_SHELL_LIFECYCLE_INCOMPLETE=') }
)
Assert-True (
  $incompleteLine.Count -eq 1
) 'Parent timeout self-test must emit exactly one structured incomplete manifest line.'
$incompleteManifest = "$($incompleteLine[0])".Substring('WINDOWS_SHELL_LIFECYCLE_INCOMPLETE='.Length) |
  ConvertFrom-Json
Assert-True (
  $incompleteManifest.current_stage -eq 'watchdog-timeout'
) 'Parent timeout self-test must record watchdog-timeout current_stage.'
Assert-True (
  $incompleteManifest.final_process_cleanup -ne 'confirmed' -and
  $incompleteManifest.cleanup.cleanup_status -ne 'confirmed'
) 'Parent timeout cleanup must not be falsely confirmed when root identity was not persisted.'

Write-Output 'PASS: Windows shell lifecycle source and workflow contract'
