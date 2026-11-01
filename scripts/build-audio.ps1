$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    cargo build -p deckweave-platform --bin deckweave-system --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Audio helper build failed' }
    $taskBundle = Join-Path (Get-Location) 'plugins/dev.deckweave.audio.sdPlugin'
    New-Item -ItemType Directory -Path (Join-Path $taskBundle 'bin') -Force | Out-Null
    Copy-Item -LiteralPath 'target/release/deckweave-system.exe' -Destination (Join-Path $taskBundle 'bin/deckweave-system.exe')
    node (Join-Path $taskBundle 'tools/package.mjs') validate
    if ($LASTEXITCODE -ne 0) { throw 'Audio bundle validation failed' }
} finally { Pop-Location }
