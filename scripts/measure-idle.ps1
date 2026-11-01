param(
    [string]$Executable = (Join-Path $PSScriptRoot '..\target\release\deckweave-desktop.exe'),
    [string]$Output = (Join-Path $PSScriptRoot '..\.local\release-idle.json'),
    [string]$Scenario = 'Release, registry and one virtual device editor open, idle',
    [ValidateRange(1, 30)][int]$Seconds = 2
)
$ErrorActionPreference = 'Stop'
$deckweaveExecutable = (Resolve-Path -LiteralPath $Executable).Path
$deckweaveProcesses = Get-CimInstance Win32_Process
$deckweaveRoot = @($deckweaveProcesses | Where-Object { $_.ExecutablePath -eq $deckweaveExecutable })
if ($deckweaveRoot.Count -ne 1) { throw 'Expected exactly one matching DeckWeave process' }
function Get-DeckWeaveTree($processes, $root) {
    $ids = @([int]$root.ProcessId)
    do {
        $previousCount = $ids.Count
        $children = @($processes | Where-Object { $_.ParentProcessId -in $ids -and $_.CreationDate -ge $root.CreationDate } | ForEach-Object { [int]$_.ProcessId })
        $ids = @($ids + $children | Select-Object -Unique)
    } while ($ids.Count -ne $previousCount)
    return $ids
}
$deckweaveIds = @(Get-DeckWeaveTree $deckweaveProcesses $deckweaveRoot[0])
$deckweaveBefore = @(Get-Process -Id $deckweaveIds -ErrorAction SilentlyContinue)
$deckweaveCpuBefore = @{}
foreach ($process in $deckweaveBefore) { $deckweaveCpuBefore[$process.Id] = @{ cpu = $process.CPU; started = $process.StartTime } }
$deckweaveClock = [System.Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds $Seconds
$deckweaveAfterIds = @(Get-DeckWeaveTree (Get-CimInstance Win32_Process) $deckweaveRoot[0])
$deckweaveAfter = @(Get-Process -Id $deckweaveAfterIds -ErrorAction SilentlyContinue)
$deckweaveClock.Stop()
if (-not ($deckweaveAfter.Id -contains $deckweaveRoot[0].ProcessId)) { throw 'DeckWeave exited during the observation' }
$deckweaveCpuSeconds = 0.0
foreach ($process in $deckweaveAfter) { if ($deckweaveCpuBefore.ContainsKey($process.Id) -and $deckweaveCpuBefore[$process.Id].started -eq $process.StartTime) { $deckweaveCpuSeconds += $process.CPU - $deckweaveCpuBefore[$process.Id].cpu } }
$deckweaveChanged = @(Compare-Object @($deckweaveBefore.Id) @($deckweaveAfter.Id)).Count -gt 0
$deckweaveSample = [ordered]@{
    measuredAtUtc = [DateTime]::UtcNow.ToString('o')
    scenario = $Scenario
    processCount = $deckweaveAfter.Count
    privateCommitMiB = [Math]::Round((($deckweaveAfter | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB), 1)
    summedWorkingSetMiB = [Math]::Round((($deckweaveAfter | Measure-Object WorkingSet64 -Sum).Sum / 1MB), 1)
    cpuSeconds = [Math]::Round($deckweaveCpuSeconds, 4)
    processTreeChanged = $deckweaveChanged
    sampleSeconds = [Math]::Round($deckweaveClock.Elapsed.TotalSeconds, 3)
    notes = 'Includes all descendants, including Node and Edge. CPU covers processes alive at both observations; process churn makes it a lower bound. Short observation, not a resource budget or endurance test. Summed working sets may count shared pages repeatedly.'
} | ConvertTo-Json
Set-Content -LiteralPath $Output -Value $deckweaveSample -Encoding utf8
Write-Output $deckweaveSample
