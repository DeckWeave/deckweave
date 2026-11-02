param([Parameter(Mandatory=$true)][int]$RootPid)
$ErrorActionPreference = 'Stop'
$processRows = @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId)
$ownedIds = [System.Collections.Generic.HashSet[int]]::new()
[void]$ownedIds.Add($RootPid)
do {
    $added = $false
    foreach ($row in $processRows) {
        if ($ownedIds.Contains([int]$row.ParentProcessId) -and $ownedIds.Add([int]$row.ProcessId)) { $added = $true }
    }
} while ($added)
$metrics = @(foreach ($ownedId in $ownedIds) {
    $item = Get-Process -Id $ownedId -ErrorAction SilentlyContinue
    if ($null -ne $item) { [pscustomobject]@{pid=$item.Id;name=$item.ProcessName;privateBytes=$item.PrivateMemorySize64;workingSetBytes=$item.WorkingSet64;cpuSeconds=$item.CPU} }
})
ConvertTo-Json -InputObject $metrics -Compress
