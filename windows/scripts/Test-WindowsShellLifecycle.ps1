#Requires -Version 5.1

[CmdletBinding()]
param(
  [string] $OutputRoot,
  [switch] $SkipBuild,
  [switch] $PreflightOnly,
  [switch] $ChildRun,
  [switch] $SelfTestTimeoutCleanup,
  [switch] $SelfTestIdentityMismatch,
  [switch] $SelfTestIdentityCaptureGap,
  [switch] $SelfTestParentTimeout,
  [switch] $SelfTestChildHangAfterManifest,
  [int] $WorkflowTimeoutSeconds = 120
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$windowsRoot = Join-Path $repositoryRoot 'windows'
$artifactBase = Join-Path $repositoryRoot '.local-artifacts\windows-shell-lifecycle'
$appPath = Join-Path $windowsRoot 'target\release\codexu-tauri.exe'
$coverage = @(
  'close-to-tray',
  'settings-open',
  'settings-refresh-smoke',
  'refresh',
  'maximize-dpi',
  'quit-cleanup',
  'identity-mismatch-protected'
)
$notObservedByWorkflow = @(
  'tray-left-click-open',
  'tray-menu-settings',
  'tray-menu-refresh',
  'tray-menu-quit',
  'settings-save-error'
)

function Assert-True {
  param([bool] $Condition, [string] $Message)
  if (-not $Condition) {
    throw $Message
  }
}

function Get-NormalizedOutputRoot {
  param([string] $RequestedPath)

  if ([string]::IsNullOrWhiteSpace($RequestedPath)) {
    $leaf = (Get-Date).ToString('yyyy-MM-dd-HHmmss-fff') + '-shell-lifecycle'
    $RequestedPath = Join-Path $artifactBase $leaf
  } elseif (-not [System.IO.Path]::IsPathRooted($RequestedPath)) {
    $RequestedPath = Join-Path $repositoryRoot $RequestedPath
  }

  $fullPath = [System.IO.Path]::GetFullPath($RequestedPath)
  $basePath = [System.IO.Path]::GetFullPath($artifactBase)
  $basePrefix = $basePath.TrimEnd([char[]]"\/") + [System.IO.Path]::DirectorySeparatorChar
  if (-not $fullPath.StartsWith($basePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'OutputRoot must be a new child of .local-artifacts/windows-shell-lifecycle.'
  }
  return $fullPath
}

function Initialize-ShellLifecycleDriver {
  if ($null -ne ('ShellLifecycleDriver' -as [type])) {
    return
  }

  Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class ShellLifecycleDriver
{
    private const uint WM_CLOSE = 0x0010;
    private const int SW_MAXIMIZE = 3;
    private const int SW_RESTORE = 9;

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    private delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hwnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int maxCount);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hwnd, int command);

    [DllImport("user32.dll")]
    private static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hwnd);

    [DllImport("user32.dll")]
    private static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hwnd);

    [DllImport("user32.dll")]
    private static extern bool IsZoomed(IntPtr hwnd);

    public static IntPtr GetForegroundWindowHandle()
    {
        return GetForegroundWindow();
    }

    public static bool FocusWindow(IntPtr hwnd)
    {
        ShowWindow(hwnd, SW_RESTORE);
        return SetForegroundWindow(hwnd);
    }

    public static bool CloseWindow(IntPtr hwnd)
    {
        return PostMessage(hwnd, WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
    }

    public static string MaximizeAndDescribe(IntPtr hwnd)
    {
        ShowWindow(hwnd, SW_MAXIMIZE);
        Thread.Sleep(500);
        RECT rect;
        if (!GetWindowRect(hwnd, out rect))
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }
        return String.Format(
            "maximized={0};outer={1}x{2};dpi={3}",
            IsZoomed(hwnd),
            rect.Right - rect.Left,
            rect.Bottom - rect.Top,
            GetDpiForWindow(hwnd)
        );
    }

    public static string ListTopLevelWindows(int processId)
    {
        var parts = new List<string>();
        EnumWindows(delegate(IntPtr hwnd, IntPtr lParam)
        {
            uint ownerProcessId;
            GetWindowThreadProcessId(hwnd, out ownerProcessId);
            if (ownerProcessId == processId)
            {
                var title = new StringBuilder(256);
                GetWindowText(hwnd, title, title.Capacity);
                parts.Add(String.Format(
                    "{0}|visible={1}|title={2}",
                    hwnd.ToInt64(),
                    IsWindowVisible(hwnd),
                    title.ToString().Replace("|", " ")
                ));
            }
            return true;
        }, IntPtr.Zero);
        return String.Join("\n", parts.ToArray());
    }
}
'@
}

function Get-ProcessIdentity {
  param([int] $ProcessId)
  $process = Get-CimInstance Win32_Process -Filter "ProcessId = $ProcessId" -ErrorAction SilentlyContinue
  if ($null -eq $process) {
    return $null
  }
  $creation = $null
  if ($null -ne $process.CreationDate) {
    try {
      $creationText = [string]$process.CreationDate
      if ($creationText -match '^\d{14}\.') {
        $creation = [System.Management.ManagementDateTimeConverter]::ToDateTime(
          $creationText
        ).ToUniversalTime().ToString('o')
      } else {
        $creation = ([datetime]$process.CreationDate).ToUniversalTime().ToString('o')
      }
    } catch {
      $creation = [string]$process.CreationDate
    }
  }
  return [pscustomobject]@{
    process_id = [int]$process.ProcessId
    parent_process_id = [int]$process.ParentProcessId
    name = [string]$process.Name
    executable_path = [string]$process.ExecutablePath
    creation_utc = $creation
  }
}

function Get-TaskDescendantIdentities {
  param([int] $RootProcessId)
  $all = @(Get-CimInstance Win32_Process)
  $pending = New-Object System.Collections.Queue
  $pending.Enqueue($RootProcessId)
  $descendants = @()
  while ($pending.Count -gt 0) {
    $parent = [int]$pending.Dequeue()
    foreach ($child in @($all | Where-Object { [int]$_.ParentProcessId -eq $parent })) {
      $descendants += [int]$child.ProcessId
      $pending.Enqueue([int]$child.ProcessId)
    }
  }
  foreach ($id in $descendants) {
    $identity = Get-ProcessIdentity -ProcessId $id
    if ($null -ne $identity) {
      $identity
    }
  }
}

function Add-RecordedIdentity {
  param([System.Collections.ArrayList] $Records, [pscustomobject] $Identity)
  if (-not (Test-CompleteProcessIdentity -Identity $Identity)) {
    return
  }
  $existing = @($Records | Where-Object {
    $_.process_id -eq $Identity.process_id -and
    $_.creation_utc -eq $Identity.creation_utc -and
    $_.executable_path -eq $Identity.executable_path
  })
  if ($existing.Count -eq 0) {
    [void]$Records.Add($Identity)
  }
}

function Test-CompleteProcessIdentity {
  param([pscustomobject] $Identity)
  if ($null -eq $Identity) {
    return $false
  }
  return (
    [int]$Identity.process_id -gt 0 -and
    -not [string]::IsNullOrWhiteSpace([string]$Identity.name) -and
    -not [string]::IsNullOrWhiteSpace([string]$Identity.creation_utc) -and
    -not [string]::IsNullOrWhiteSpace([string]$Identity.executable_path)
  )
}

function Update-TaskProcessRecords {
  param([int] $RootProcessId, [System.Collections.ArrayList] $Records)
  Add-RecordedIdentity -Records $Records -Identity (Get-ProcessIdentity -ProcessId $RootProcessId)
  foreach ($identity in @(Get-TaskDescendantIdentities -RootProcessId $RootProcessId)) {
    Add-RecordedIdentity -Records $Records -Identity $identity
  }
}

function Test-RecordedIdentity {
  param([pscustomobject] $Recorded, [pscustomobject] $Current)
  if (-not (Test-CompleteProcessIdentity -Identity $Recorded) -or
      -not (Test-CompleteProcessIdentity -Identity $Current)) {
    return $false
  }
  return (
    $Recorded.process_id -eq $Current.process_id -and
    $Recorded.name -eq $Current.name -and
    $Recorded.creation_utc -eq $Current.creation_utc -and
    $Recorded.executable_path -eq $Current.executable_path
  )
}

function Stop-RecordedTaskProcesses {
  param([pscustomobject] $RootIdentity, [System.Collections.ArrayList] $Records)

  $stopped = [System.Collections.ArrayList]::new()
  $alreadyExited = [System.Collections.ArrayList]::new()
  $identityMismatches = [System.Collections.ArrayList]::new()

  if (-not (Test-CompleteProcessIdentity -Identity $RootIdentity)) {
    return [ordered]@{
      cleanup_status = 'unknown'
      cleanup_reason = 'root-identity-not-persisted'
      recorded_count = $Records.Count
      stopped_process_ids = @()
      already_exited_process_ids = @()
      identity_mismatch_process_ids = @()
      remaining_matching_process_ids = @()
    }
  }

  $invalidRecords = @($Records | Where-Object {
    -not (Test-CompleteProcessIdentity -Identity $_)
  })
  if ($invalidRecords.Count -gt 0) {
    return [ordered]@{
      cleanup_status = 'unknown'
      cleanup_reason = 'captured-process-identity-incomplete'
      recorded_count = $Records.Count
      stopped_process_ids = @()
      already_exited_process_ids = @()
      identity_mismatch_process_ids = @()
      remaining_matching_process_ids = @()
    }
  }

  $capturedRecords = @($Records | Where-Object {
    Test-CompleteProcessIdentity -Identity $_
  })
  if ($capturedRecords.Count -eq 0) {
    return [ordered]@{
      cleanup_status = 'unknown'
      cleanup_reason = 'no-captured-process-identities'
      recorded_count = 0
      stopped_process_ids = @()
      already_exited_process_ids = @()
      identity_mismatch_process_ids = @()
      remaining_matching_process_ids = @()
    }
  }

  foreach ($record in @($capturedRecords | Sort-Object process_id -Descending)) {
    $current = Get-ProcessIdentity -ProcessId $record.process_id
    if ($null -eq $current) {
      [void]$alreadyExited.Add($record.process_id)
    } elseif (Test-RecordedIdentity -Recorded $record -Current $current) {
      Stop-Process -Id $record.process_id -Force -ErrorAction SilentlyContinue
      [void]$stopped.Add($record.process_id)
    } else {
      [void]$identityMismatches.Add($record.process_id)
    }
  }

  $deadline = (Get-Date).AddSeconds(8)
  do {
    $remaining = @(
      $capturedRecords | Where-Object {
        $current = Get-ProcessIdentity -ProcessId $_.process_id
        Test-RecordedIdentity -Recorded $_ -Current $current
      }
    )
    if ($remaining.Count -eq 0) {
      break
    }
    Start-Sleep -Milliseconds 100
  } while ((Get-Date) -lt $deadline)

  $cleanupStatus = if ($identityMismatches.Count -gt 0) {
    'not-confirmed'
  } elseif ($remaining.Count -gt 0) {
    'not-confirmed'
  } else {
    'confirmed'
  }
  $cleanupReason = if ($identityMismatches.Count -gt 0) {
    'captured-process-identity-mismatch'
  } elseif ($remaining.Count -gt 0) {
    'captured-processes-remain'
  } else {
    'all-captured-identities-exited-or-were-stopped'
  }

  return [ordered]@{
    cleanup_status = $cleanupStatus
    cleanup_reason = $cleanupReason
    recorded_count = $capturedRecords.Count
    stopped_process_ids = @($stopped)
    already_exited_process_ids = @($alreadyExited)
    identity_mismatch_process_ids = @($identityMismatches)
    remaining_matching_process_ids = @($remaining | ForEach-Object { $_.process_id })
  }
}

function Get-ExactExecutableProcesses {
  param([string] $ExecutablePath)
  if (-not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
    return @()
  }
  $name = [System.IO.Path]::GetFileName($ExecutablePath).Replace("'", "''")
  return @(
    Get-CimInstance Win32_Process -Filter "Name = '$name'" |
      Where-Object {
        -not [string]::IsNullOrWhiteSpace($_.ExecutablePath) -and
        [string]::Equals(
          [System.IO.Path]::GetFullPath($_.ExecutablePath),
          [System.IO.Path]::GetFullPath($ExecutablePath),
          [System.StringComparison]::OrdinalIgnoreCase
        )
      }
  )
}

function Get-TopLevelWindows {
  param([int] $ProcessId)
  $raw = [ShellLifecycleDriver]::ListTopLevelWindows($ProcessId)
  if ([string]::IsNullOrWhiteSpace($raw)) {
    return @()
  }
  return @($raw -split "`n" | ForEach-Object {
    $parts = "$_".Split('|')
    [pscustomobject]@{
      hwnd = [IntPtr]([int64]$parts[0])
      visible = $parts[1] -eq 'visible=True'
      title = $parts[2].Substring('title='.Length)
    }
  })
}

function Wait-VisibleWindow {
  param([int] $ProcessId, [string] $TitlePattern = '.', [int] $TimeoutSeconds = 45)
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  do {
    $window = @(
      Get-TopLevelWindows -ProcessId $ProcessId |
        Where-Object { $_.visible -and $_.title -match $TitlePattern } |
        Select-Object -First 1
    )
    if ($window.Count -eq 1) {
      return $window[0]
    }
    Start-Sleep -Milliseconds 150
  } while ((Get-Date) -lt $deadline)
  throw "Timed out waiting for visible window matching $TitlePattern."
}

function Find-ElementByAutomationId {
  param([IntPtr] $Window, [string] $AutomationId)
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($Window)
  $condition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
    $AutomationId
  )
  return $root.FindFirst(
    [System.Windows.Automation.TreeScope]::Descendants,
    $condition
  )
}

function Invoke-ElementByAutomationId {
  param([IntPtr] $Window, [string] $AutomationId, [int] $TimeoutSeconds = 30)
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  do {
    $element = Find-ElementByAutomationId -Window $Window -AutomationId $AutomationId
    if ($null -ne $element) {
      $pattern = $element.GetCurrentPattern(
        [System.Windows.Automation.InvokePattern]::Pattern
      )
      $pattern.Invoke()
      return
    }
    Start-Sleep -Milliseconds 150
  } while ((Get-Date) -lt $deadline)
  throw "Timed out waiting for invokable UI element $AutomationId."
}

function Read-Preflight {
  param([string] $ResolvedOutputRoot)
  return [ordered]@{
    workflow = 'Windows shell lifecycle'
    app_executable_relative = 'windows/target/release/codexu-tauri.exe'
    build_command = 'cargo +stable-x86_64-pc-windows-msvc tauri build --no-bundle'
    output_root = $ResolvedOutputRoot
    coverage = @($coverage)
    not_observed_by_workflow = @($notObservedByWorkflow)
    cleanup_policy = 'record root and descendants; stop only identity-matching process id, creation_utc, name, and executable_path'
    watchdog = 'parent process enforces WorkflowTimeoutSeconds and identity-checked cleanup'
    writes_performed = $false
  }
}

function Write-ManifestObject {
  param([string] $Path, [object] $Value)
  $json = $Value | ConvertTo-Json -Depth 12
  $temporaryPath = $Path + '.' + [guid]::NewGuid().ToString('N') + '.tmp'
  $backupPath = $Path + '.' + [guid]::NewGuid().ToString('N') + '.bak'
  try {
    [System.IO.File]::WriteAllText(
      $temporaryPath,
      $json,
      [System.Text.UTF8Encoding]::new($false)
    )
    if (Test-Path -LiteralPath $Path -PathType Leaf) {
      [System.IO.File]::Replace($temporaryPath, $Path, $backupPath, $true)
    } else {
      [System.IO.File]::Move($temporaryPath, $Path)
    }
  } finally {
    if (Test-Path -LiteralPath $temporaryPath -PathType Leaf) {
      Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $backupPath -PathType Leaf) {
      Remove-Item -LiteralPath $backupPath -Force -ErrorAction SilentlyContinue
    }
  }
}

function Read-ManifestObject {
  param([string] $Path)
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    return $null
  }
  try {
    return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
  } catch {
    return $null
  }
}

function ConvertTo-OrderedManifest {
  param([object] $Manifest)
  if ($null -eq $Manifest) {
    return $null
  }
  if ($Manifest -is [System.Collections.IDictionary]) {
    return $Manifest
  }
  $ordered = [ordered]@{}
  foreach ($property in $Manifest.PSObject.Properties) {
    $ordered[$property.Name] = $property.Value
  }
  return $ordered
}

function ConvertTo-RecordList {
  param([object] $Manifest)
  $records = [System.Collections.ArrayList]::new()
  if ($null -ne $Manifest.root_process) {
    Add-RecordedIdentity -Records $records -Identity $Manifest.root_process
  }
  foreach ($record in @($Manifest.process_records)) {
    Add-RecordedIdentity -Records $records -Identity $record
  }
  return ,$records
}

function Invoke-RecordedManifestCleanup {
  param([object] $Manifest)
  $records = ConvertTo-RecordList -Manifest $Manifest
  if ($records.Count -eq 0 -or $null -eq $Manifest.root_process) {
    return [ordered]@{
      cleanup_status = 'unknown'
      cleanup_reason = 'root-identity-not-persisted'
      recorded_count = $records.Count
      stopped_process_ids = @()
      already_exited_process_ids = @()
      identity_mismatch_process_ids = @()
      remaining_matching_process_ids = @()
    }
  }
  return Stop-RecordedTaskProcesses `
    -RootIdentity $Manifest.root_process `
    -Records $records
}

function Add-TimeoutUnobservedStates {
  param([object] $Manifest)
  $existing = @($Manifest['unobserved'] | ForEach-Object { $_.name })
  foreach ($name in @($coverage + $notObservedByWorkflow)) {
    if ($existing -notcontains $name) {
      $Manifest['unobserved'] += [ordered]@{
        name = $name
        reason = 'Workflow timed out before this behavior was observed.'
      }
    }
  }
}

function Complete-TimeoutManifest {
  param([string] $ManifestPath, [string] $Reason)
  $manifest = Read-ManifestObject -Path $ManifestPath
  $manifest = ConvertTo-OrderedManifest -Manifest $manifest
  if ($null -eq $manifest) {
    $manifest = [ordered]@{
      status = 'timeout-before-manifest'
      started_utc = $null
      completed_utc = $null
      current_stage = 'watchdog-timeout'
      coverage = @($coverage)
      observations = @()
      unobserved = @()
      root_process = $null
      process_records = @()
      cleanup = $null
      final_process_cleanup = 'unknown'
      error = $Reason
    }
  }
  $manifest['current_stage'] = 'watchdog-timeout'
  $manifest['error'] = $Reason
  Add-TimeoutUnobservedStates -Manifest $manifest
  $manifest['cleanup'] = Invoke-RecordedManifestCleanup -Manifest $manifest
  $manifest['final_process_cleanup'] = $manifest['cleanup'].cleanup_status
  $manifest['status'] = switch ($manifest['final_process_cleanup']) {
    'confirmed' { 'timeout-cleanup-confirmed'; break }
    'not-confirmed' { 'timeout-cleanup-not-confirmed'; break }
    default { 'timeout-cleanup-unknown' }
  }
  $manifest['completed_utc'] = (Get-Date).ToUniversalTime().ToString('o')
  Write-ManifestObject -Path $ManifestPath -Value $manifest
  return $manifest
}

function Invoke-ParentWatchdog {
  param(
    [string] $ResolvedOutputRoot,
    [switch] $ChildHangAfterManifest
  )
  if (Test-Path -LiteralPath $ResolvedOutputRoot) {
    throw 'Refusing to overwrite an existing shell lifecycle output directory.'
  }
  New-Item -ItemType Directory -Path $ResolvedOutputRoot | Out-Null
  $manifestPath = Join-Path $ResolvedOutputRoot 'manifest.json'
  $powershell = (Get-Command powershell.exe -ErrorAction Stop).Source
  $arguments = @(
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-File',
    $PSCommandPath,
    '-ChildRun',
    '-OutputRoot',
    $ResolvedOutputRoot,
    '-WorkflowTimeoutSeconds',
    [string]$WorkflowTimeoutSeconds
  )
  if ($SkipBuild) {
    $arguments += '-SkipBuild'
  }
  if ($ChildHangAfterManifest) {
    $arguments += '-SelfTestChildHangAfterManifest'
  }
  $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
  $startInfo.FileName = $powershell
  $startInfo.Arguments = ($arguments | ForEach-Object {
    if ($_ -match '[\s"]') {
      '"' + ($_ -replace '"', '\"') + '"'
    } else {
      $_
    }
  }) -join ' '
  $startInfo.UseShellExecute = $false
  $startInfo.CreateNoWindow = $true
  $startInfo.RedirectStandardOutput = $true
  $startInfo.RedirectStandardError = $true
  $child = [System.Diagnostics.Process]::new()
  $child.StartInfo = $startInfo
  if (-not $child.Start()) {
    throw 'Could not start shell lifecycle child workflow.'
  }
  $childIdentity = Get-ProcessIdentity -ProcessId $child.Id
  $watchdogRecords = [System.Collections.ArrayList]::new()
  Add-RecordedIdentity -Records $watchdogRecords -Identity $childIdentity
  if (-not (Test-CompleteProcessIdentity -Identity $childIdentity)) {
    throw 'Could not persist a complete watchdog child process identity.'
  }
  Write-ManifestObject `
    -Path (Join-Path $ResolvedOutputRoot 'watchdog.json') `
    -Value ([ordered]@{
      status = 'running'
      child_process = $childIdentity
      child_identity_persisted = $true
      timeout_seconds = $WorkflowTimeoutSeconds
    })
  $stdoutTask = $child.StandardOutput.ReadToEndAsync()
  $stderrTask = $child.StandardError.ReadToEndAsync()
  if (-not $child.WaitForExit($WorkflowTimeoutSeconds * 1000)) {
    $watchdogCleanup = Stop-RecordedTaskProcesses `
      -RootIdentity $childIdentity `
      -Records $watchdogRecords
    $manifest = Complete-TimeoutManifest `
      -ManifestPath $manifestPath `
      -Reason "Workflow exceeded ${WorkflowTimeoutSeconds}s watchdog timeout."
    $manifest['watchdog_process'] = $childIdentity
    $manifest['watchdog_cleanup'] = $watchdogCleanup
    Write-ManifestObject -Path $manifestPath -Value $manifest
    Write-Output (
      'WINDOWS_SHELL_LIFECYCLE_INCOMPLETE=' +
      ($manifest | ConvertTo-Json -Depth 8 -Compress)
    )
    exit 2
  }
  [void]$stdoutTask.Wait(5000)
  [void]$stderrTask.Wait(5000)
  if ($stdoutTask.IsCompleted -and -not [string]::IsNullOrWhiteSpace($stdoutTask.Result)) {
    Write-Output $stdoutTask.Result
  }
  if ($stderrTask.IsCompleted -and -not [string]::IsNullOrWhiteSpace($stderrTask.Result)) {
    Write-Error $stderrTask.Result
  }
  exit $child.ExitCode
}

function Invoke-WatchdogSelfTest {
  param([string] $ResolvedOutputRoot)
  if (Test-Path -LiteralPath $ResolvedOutputRoot) {
    throw 'Refusing to overwrite an existing watchdog self-test output directory.'
  }
  New-Item -ItemType Directory -Path $ResolvedOutputRoot | Out-Null
  $manifestPath = Join-Path $ResolvedOutputRoot 'manifest.json'
  $powershell = (Get-Command powershell.exe -ErrorAction Stop).Source
  $sleeper = Start-Process `
    -FilePath $powershell `
    -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Seconds 60') `
    -WindowStyle Hidden `
    -PassThru
  $records = [System.Collections.ArrayList]::new()
  Update-TaskProcessRecords -RootProcessId $sleeper.Id -Records $records
  $manifest = [ordered]@{
    status = 'running'
    started_utc = (Get-Date).ToUniversalTime().ToString('o')
    completed_utc = $null
    current_stage = 'self-test-sleeper-running'
    coverage = @($coverage)
    observations = @()
    unobserved = @()
    root_process = Get-ProcessIdentity -ProcessId $sleeper.Id
    process_records = @($records)
    cleanup = $null
    final_process_cleanup = 'pending'
    error = $null
  }
  Write-ManifestObject -Path $manifestPath -Value $manifest
  Start-Sleep -Seconds $WorkflowTimeoutSeconds
  $manifest = Complete-TimeoutManifest `
    -ManifestPath $manifestPath `
    -Reason 'Watchdog self-test simulated a hung child workflow.'
  Write-Output (
    'WINDOWS_SHELL_LIFECYCLE_WATCHDOG_SELFTEST=' +
    ($manifest | ConvertTo-Json -Depth 8 -Compress)
  )
}

function Invoke-IdentityMismatchSelfTest {
  param([string] $ResolvedOutputRoot)
  if (Test-Path -LiteralPath $ResolvedOutputRoot) {
    throw 'Refusing to overwrite an existing identity mismatch self-test output directory.'
  }
  New-Item -ItemType Directory -Path $ResolvedOutputRoot | Out-Null
  $manifestPath = Join-Path $ResolvedOutputRoot 'manifest.json'
  $powershell = (Get-Command powershell.exe -ErrorAction Stop).Source
  $sleeper = Start-Process `
    -FilePath $powershell `
    -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Seconds 60') `
    -WindowStyle Hidden `
    -PassThru
  $captured = $null
  $records = [System.Collections.ArrayList]::new()
  $manifest = $null
  try {
    $captured = Get-ProcessIdentity -ProcessId $sleeper.Id
    Assert-True (
      Test-CompleteProcessIdentity -Identity $captured
    ) 'Identity mismatch self-test could not capture a complete process identity.'
    Update-TaskProcessRecords -RootProcessId $sleeper.Id -Records $records
    $tampered = [pscustomobject]@{
      process_id = $captured.process_id
      parent_process_id = $captured.parent_process_id
      name = $captured.name
      creation_utc = $captured.creation_utc
      executable_path = Join-Path $env:SystemRoot 'System32\pid-reuse-placeholder.exe'
    }
    $manifest = [ordered]@{
      status = 'running'
      started_utc = (Get-Date).ToUniversalTime().ToString('o')
      completed_utc = $null
      current_stage = 'watchdog-timeout'
      coverage = @($coverage)
      observations = @()
      unobserved = @()
      root_process = $tampered
      root_identity_persisted = $true
      process_records = @($tampered)
      cleanup = $null
      final_process_cleanup = 'pending'
      error = $null
    }
    Write-ManifestObject -Path $manifestPath -Value $manifest
    $manifest = Complete-TimeoutManifest `
      -ManifestPath $manifestPath `
      -Reason 'Identity mismatch self-test simulated PID reuse before cleanup.'
    Assert-True (
      $manifest.final_process_cleanup -eq 'not-confirmed'
    ) 'Identity mismatch self-test must refuse to confirm cleanup.'
  } finally {
    if (Test-CompleteProcessIdentity -Identity $captured) {
      $safeCleanup = Stop-RecordedTaskProcesses `
        -RootIdentity $captured `
        -Records ([System.Collections.ArrayList]@($captured))
      if ($null -ne $manifest) {
        $manifest | Add-Member -MemberType NoteProperty -Name test_process_cleanup -Value $safeCleanup -Force
        Write-ManifestObject -Path $manifestPath -Value $manifest
      }
    }
  }
  Write-Output (
    'WINDOWS_SHELL_LIFECYCLE_IDENTITY_MISMATCH_SELFTEST=' +
    ((Read-ManifestObject -Path $manifestPath) | ConvertTo-Json -Depth 8 -Compress)
  )
}

function Invoke-IdentityCaptureGapSelfTest {
  param([string] $ResolvedOutputRoot)
  if (Test-Path -LiteralPath $ResolvedOutputRoot) {
    throw 'Refusing to overwrite an identity capture gap self-test output directory.'
  }
  New-Item -ItemType Directory -Path $ResolvedOutputRoot | Out-Null
  $manifestPath = Join-Path $ResolvedOutputRoot 'manifest.json'
  $powershell = (Get-Command powershell.exe -ErrorAction Stop).Source
  $sleeper = Start-Process `
    -FilePath $powershell `
    -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Seconds 60') `
    -WindowStyle Hidden `
    -PassThru
  $captured = $null
  $manifest = $null
  try {
    $captured = Get-ProcessIdentity -ProcessId $sleeper.Id
    Assert-True (
      Test-CompleteProcessIdentity -Identity $captured
    ) 'Identity capture gap self-test could not capture its test process identity.'
    $manifest = [ordered]@{
      status = 'running'
      started_utc = (Get-Date).ToUniversalTime().ToString('o')
      completed_utc = $null
      current_stage = 'starting-release-executable'
      coverage = @($coverage)
      observations = @()
      unobserved = @()
      root_process = $null
      root_identity_persisted = $false
      process_records = @()
      cleanup = $null
      final_process_cleanup = 'pending'
      error = $null
    }
    Write-ManifestObject -Path $manifestPath -Value $manifest
    $manifest = Complete-TimeoutManifest `
      -ManifestPath $manifestPath `
      -Reason 'Identity capture gap self-test simulated timeout before root identity persistence.'
    Assert-True (
      $manifest.final_process_cleanup -eq 'unknown'
    ) 'Identity capture gap self-test must report unknown cleanup.'
  } finally {
    if (Test-CompleteProcessIdentity -Identity $captured) {
      $safeCleanup = Stop-RecordedTaskProcesses `
        -RootIdentity $captured `
        -Records ([System.Collections.ArrayList]@($captured))
      if ($null -ne $manifest) {
        $manifest | Add-Member -MemberType NoteProperty -Name test_process_cleanup -Value $safeCleanup -Force
        Write-ManifestObject -Path $manifestPath -Value $manifest
      }
    }
  }
  Write-Output (
    'WINDOWS_SHELL_LIFECYCLE_IDENTITY_CAPTURE_GAP_SELFTEST=' +
    ((Read-ManifestObject -Path $manifestPath) | ConvertTo-Json -Depth 8 -Compress)
  )
}

$resolvedOutputRoot = Get-NormalizedOutputRoot -RequestedPath $OutputRoot
$preflight = Read-Preflight -ResolvedOutputRoot $resolvedOutputRoot
if ($PreflightOnly) {
  Write-Output ('WINDOWS_SHELL_LIFECYCLE_PREFLIGHT=' + ($preflight | ConvertTo-Json -Depth 5 -Compress))
  return
}

if ($SelfTestTimeoutCleanup) {
  Invoke-WatchdogSelfTest -ResolvedOutputRoot $resolvedOutputRoot
  return
}

if ($SelfTestIdentityMismatch) {
  Invoke-IdentityMismatchSelfTest -ResolvedOutputRoot $resolvedOutputRoot
  return
}

if ($SelfTestIdentityCaptureGap) {
  Invoke-IdentityCaptureGapSelfTest -ResolvedOutputRoot $resolvedOutputRoot
  return
}

if ($SelfTestParentTimeout) {
  Invoke-ParentWatchdog `
    -ResolvedOutputRoot $resolvedOutputRoot `
    -ChildHangAfterManifest
  return
}

if (-not $ChildRun) {
  Invoke-ParentWatchdog -ResolvedOutputRoot $resolvedOutputRoot
  return
}

if (-not (Test-Path -LiteralPath $resolvedOutputRoot -PathType Container)) {
  New-Item -ItemType Directory -Path $resolvedOutputRoot | Out-Null
}
$runtimeRoot = Join-Path $resolvedOutputRoot 'runtime'
$logsRoot = Join-Path $resolvedOutputRoot 'logs'
New-Item -ItemType Directory -Path $runtimeRoot, $logsRoot | Out-Null
$manifestPath = Join-Path $resolvedOutputRoot 'manifest.json'

$manifest = [ordered]@{
  status = 'running'
  started_utc = (Get-Date).ToUniversalTime().ToString('o')
  completed_utc = $null
  current_stage = 'initializing'
  checkout = [ordered]@{
    branch = (& git -C $repositoryRoot branch --show-current).Trim()
    sha = (& git -C $repositoryRoot rev-parse HEAD).Trim()
  }
  coverage = @($coverage)
  not_observed_by_workflow = @($notObservedByWorkflow)
  observations = @()
  unobserved = @()
  root_process = $null
  root_identity_persisted = $false
  root_identity_persisted_utc = $null
  process_records = @()
  cleanup = $null
  final_process_cleanup = 'pending'
  error = $null
}

function Save-Manifest {
  Write-ManifestObject -Path $manifestPath -Value $manifest
}

function Set-Stage {
  param([string] $Stage)
  $manifest.current_stage = $Stage
  Save-Manifest
}

function Add-Observation {
  param([string] $Name, [hashtable] $Data = @{})
  $entry = [ordered]@{ name = $Name; observed = $true }
  foreach ($key in $Data.Keys) {
    $entry[$key] = $Data[$key]
  }
  $manifest.observations += $entry
  Save-Manifest
}

function Add-Unobserved {
  param([string] $Name, [string] $Reason)
  $manifest.unobserved += [ordered]@{ name = $Name; reason = $Reason }
  Save-Manifest
}

Save-Manifest
if ($SelfTestChildHangAfterManifest) {
  Start-Sleep -Seconds ($WorkflowTimeoutSeconds + 30)
  return
}
Initialize-ShellLifecycleDriver
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$process = $null
$records = [System.Collections.ArrayList]::new()
try {
  if (-not $SkipBuild) {
    Set-Stage 'building-release-executable'
    $cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
    if ($null -eq $cargo) {
      $cargo = Get-Command cargo -ErrorAction Stop
    }
    Push-Location $windowsRoot
    $previousErrorActionPreference = $ErrorActionPreference
    try {
      $ErrorActionPreference = 'Continue'
      $buildOutput = @(
        & $cargo.Source '+stable-x86_64-pc-windows-msvc' 'tauri' 'build' '--no-bundle' 2>&1
      )
      $buildExitCode = $LASTEXITCODE
    } finally {
      $ErrorActionPreference = $previousErrorActionPreference
      Pop-Location
    }
    $buildOutput |
      Set-Content -LiteralPath (Join-Path $logsRoot 'tauri-build.log') -Encoding UTF8
    Assert-True ($buildExitCode -eq 0) "Tauri release build failed with exit code $buildExitCode."
  }
  Set-Stage 'checking-release-executable'
  Assert-True (Test-Path -LiteralPath $appPath -PathType Leaf) 'The release executable is missing.'
  Assert-True (@(Get-ExactExecutableProcesses -ExecutablePath $appPath).Count -eq 0) 'The exact target release executable is already running; refusing to manage it.'

  Set-Stage 'preparing-task-local-runtime'
  $appData = Join-Path $runtimeRoot 'appdata'
  $localAppData = Join-Path $runtimeRoot 'localappdata'
  $webViewData = Join-Path $runtimeRoot 'webview2'
  New-Item -ItemType Directory -Path $appData, $localAppData, $webViewData | Out-Null

  $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
  $startInfo.FileName = $appPath
  $startInfo.WorkingDirectory = Split-Path -Parent $appPath
  $startInfo.UseShellExecute = $false
  $startInfo.CreateNoWindow = $true
  $startInfo.RedirectStandardOutput = $true
  $startInfo.RedirectStandardError = $true
  $startInfo.EnvironmentVariables['APPDATA'] = $appData
  $startInfo.EnvironmentVariables['LOCALAPPDATA'] = $localAppData
  $startInfo.EnvironmentVariables['WEBVIEW2_USER_DATA_FOLDER'] = $webViewData

  $process = [System.Diagnostics.Process]::new()
  $process.StartInfo = $startInfo
  Set-Stage 'starting-release-executable'
  Assert-True ($process.Start()) 'Could not start the shell lifecycle task application.'
  Set-Stage 'capturing-root-process-identity'
  $manifest.root_process = Get-ProcessIdentity -ProcessId $process.Id
  Assert-True (
    Test-CompleteProcessIdentity -Identity $manifest.root_process
  ) 'Could not capture a complete root process identity before timeout-sensitive workflow steps.'
  $manifest.root_identity_persisted = $true
  $manifest.root_identity_persisted_utc = (Get-Date).ToUniversalTime().ToString('o')
  Add-RecordedIdentity -Records $records -Identity $manifest.root_process
  $manifest.process_records = @($records)
  Save-Manifest
  Set-Stage 'recording-started-process-identity'
  Update-TaskProcessRecords -RootProcessId $process.Id -Records $records
  $manifest.process_records = @($records)
  Save-Manifest

  Set-Stage 'waiting-for-main-window'
  $mainWindow = Wait-VisibleWindow -ProcessId $process.Id -TitlePattern '^codexU$'
  Add-Observation 'startup-main-window' @{ hwnd = [string]$mainWindow.hwnd }

  Set-Stage 'invoking-header-refresh'
  [void][ShellLifecycleDriver]::FocusWindow($mainWindow.hwnd)
  Invoke-ElementByAutomationId -Window $mainWindow.hwnd -AutomationId 'header-refresh'
  Add-Observation 'refresh' @{ source = 'header action with same command path as tray refresh' }

  Set-Stage 'opening-settings-window'
  Invoke-ElementByAutomationId -Window $mainWindow.hwnd -AutomationId 'header-open-settings'
  $settingsWindow = Wait-VisibleWindow -ProcessId $process.Id -TitlePattern 'Settings|设置'
  Add-Observation 'settings-open' @{ title = $settingsWindow.title }
  Set-Stage 'invoking-settings-refresh-smoke'
  Invoke-ElementByAutomationId -Window $settingsWindow.hwnd -AutomationId 'settings-refresh-now'
  Add-Observation 'settings-refresh-smoke' @{ source = 'settings refresh action completed without process exit' }
  Add-Unobserved 'settings-save-error' 'No deterministic task-local filesystem failure was injected in v0; Settings error UI remains covered by source contract only.'

  Set-Stage 'maximizing-main-window-and-reading-dpi'
  $maximizedDescription = [ShellLifecycleDriver]::MaximizeAndDescribe($mainWindow.hwnd)
  Add-Observation 'maximize-dpi' @{ window = $maximizedDescription }

  Set-Stage 'posting-main-window-close'
  Assert-True ([ShellLifecycleDriver]::CloseWindow($mainWindow.hwnd)) 'Failed to post WM_CLOSE to the main window.'
  Start-Sleep -Milliseconds 1000
  $process.Refresh()
  Assert-True (-not $process.HasExited) 'Main close exited the app instead of hiding to tray.'
  Add-Observation 'close-to-tray' @{ process_alive = $true }

  Add-Unobserved 'tray-left-click-open' 'PowerShell v0 workflow does not click the system tray overflow or menu; source/menu command contract is covered separately.'
  Add-Unobserved 'tray-menu-settings' 'PowerShell v0 workflow does not click the system tray menu; Settings command is exercised through the same Tauri IPC path from the header.'
  Add-Unobserved 'tray-menu-refresh' 'PowerShell v0 workflow does not click the system tray menu; Refresh command is smoke-tested through the header action only.'
  Add-Unobserved 'tray-menu-quit' 'PowerShell v0 workflow uses identity-checked cleanup instead of tray Quit to avoid uncontrolled tray automation.'
  Add-Observation 'quit-cleanup' @{ source = 'identity-checked task cleanup' }
} catch {
  $manifest.status = 'failed'
  $manifest.error = $_.Exception.Message
  Add-TimeoutUnobservedStates -Manifest $manifest
  throw
} finally {
  if ($null -ne $process) {
    Set-Stage 'identity-checked-cleanup'
    $manifest.cleanup = Stop-RecordedTaskProcesses -RootIdentity $manifest.root_process -Records $records
    $manifest.final_process_cleanup = $manifest.cleanup.cleanup_status
    $manifest.process_records = @($records)
  }
  if ($manifest.status -eq 'running') {
    $manifest.status = 'complete'
  }
  $manifest.current_stage = 'completed'
  $manifest.completed_utc = (Get-Date).ToUniversalTime().ToString('o')
  Save-Manifest
}

Assert-True ($manifest.final_process_cleanup -eq 'confirmed') 'Task-owned shell lifecycle processes remained after cleanup.'
$summary = [ordered]@{
  status = $manifest.status
  observations = @($manifest.observations).Count
  unobserved = @($manifest.unobserved).Count
  process_cleanup = $manifest.final_process_cleanup
  output_root = $resolvedOutputRoot
}
Write-Output ('WINDOWS_SHELL_LIFECYCLE_COMPLETE=' + ($summary | ConvertTo-Json -Depth 5 -Compress))
