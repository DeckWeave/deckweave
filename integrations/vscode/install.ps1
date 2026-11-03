param([string]$ExtensionsRoot = (Join-Path $env:USERPROFILE '.vscode\extensions'))
$ErrorActionPreference = 'Stop'
$manifest = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'package.json') -Raw | ConvertFrom-Json
$destination = Join-Path ([IO.Path]::GetFullPath($ExtensionsRoot)) ('deckweave.workspace-' + $manifest.version)
if (Test-Path -LiteralPath $destination) {
    $old = Get-Content -LiteralPath (Join-Path $destination 'package.json') -Raw | ConvertFrom-Json
    if ($old.publisher -ne 'deckweave' -or $old.name -ne 'workspace') { throw 'Refusing to replace an unrelated extension directory' }
    $backupRoot = Join-Path (Split-Path -Parent ([IO.Path]::GetFullPath($ExtensionsRoot))) 'deckweave-extension-backups'
    New-Item -ItemType Directory -Path $backupRoot -Force | Out-Null
    $backup = Join-Path $backupRoot ('workspace-' + $manifest.version + '-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
    Copy-Item -LiteralPath $destination -Destination $backup -Recurse
}
New-Item -ItemType Directory -Path $destination -Force | Out-Null
foreach ($file in @('package.json', 'extension.cjs', 'core.cjs', 'server.cjs', 'secure-directory.ps1', 'README.md')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination (Join-Path $destination $file) -Force
}
Write-Output "Installed source companion: $destination"
Write-Output 'Reload the intended local VS Code window. No keybindings or user/workspace settings were changed.'
