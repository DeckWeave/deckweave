param(
    [string]$Executable = (Join-Path $PSScriptRoot '..\target\release\deckweave-desktop.exe'),
    [string]$Output = (Join-Path $PSScriptRoot '..\.local\release-idle.json'),
    [string]$Scenario = 'Release, registry and one virtual device editor open, idle',
    [ValidateRange(1, 30)][int]$Seconds = 2
)
$ErrorActionPreference = 'Stop'
$deckweaveExecutable = (Resolve-Path -LiteralPath $Executable).Path
$deckweaveProcesses = Get-CimInstance Win32_Process -Filter "Name = 'deckweave-desktop.exe' OR Name = 'msedgewebview2.exe'"
$deckweaveRoot = @($deckweaveProcesses | Where-Object { $_.ExecutablePath -eq $deckweaveExecutable })
if ($deckweaveRoot.Count -ne 1) { throw 'Expected exactly one matching DeckWeave process' }
$deckweaveIds = @([int]$deckweaveRoot[0].ProcessId)
do {
    $deckweavePreviousCount = $deckweaveIds.Count
    $deckweaveChildren = @($deckweaveProcesses | Where-Object { $_.ParentProcessId -in $deckweaveIds } | ForEach-Object { [int]$_.ProcessId })
    $deckweaveIds = @($deckweaveIds + $deckweaveChildren | Select-Object -Unique)
} while ($deckweaveIds.Count -ne $deckweavePreviousCount)
$deckweaveBefore = @(Get-Process -Id $deckweaveIds)
$deckweaveCpuBefore = ($deckweaveBefore | Measure-Object CPU -Sum).Sum
$deckweaveClock = [System.Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds $Seconds
$deckweaveAfter = @(Get-Process -Id $deckweaveIds)
$deckweaveClock.Stop()
$deckweaveSample = [ordered]@{
    measuredAtUtc = [DateTime]::UtcNow.ToString('o')
    scenario = $Scenario
    processCount = $deckweaveAfter.Count
    privateCommitMiB = [Math]::Round((($deckweaveAfter | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB), 1)
    summedWorkingSetMiB = [Math]::Round((($deckweaveAfter | Measure-Object WorkingSet64 -Sum).Sum / 1MB), 1)
    cpuSeconds = [Math]::Round((($deckweaveAfter | Measure-Object CPU -Sum).Sum - $deckweaveCpuBefore), 4)
    sampleSeconds = [Math]::Round($deckweaveClock.Elapsed.TotalSeconds, 3)
    notes = 'Short observation, not a resource budget or endurance test. Summed working sets may count shared pages repeatedly.'
} | ConvertTo-Json
Set-Content -LiteralPath $Output -Value $deckweaveSample -Encoding utf8
Write-Output $deckweaveSample
