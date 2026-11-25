<#
.SYNOPSIS
    Makes a fresh copy of the sample memory folder for the walk: target/walk-sample.

.DESCRIPTION
    Removes target/walk-sample when it is there, copies sample/ to it, makes it a git folder
    with one commit and a name for its commits set in the copy only (so Lorisse can write in
    it on any machine; no setting outside the copy is changed), and prints
    "sample ready: target/walk-sample" as its last line. Run twice, it starts clean again.
    sample/ itself is only read.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/walk_sample.ps1

.NOTES
    Plain ASCII, Windows PowerShell 5.1 compatible. Exit code 0 = ready, 1 = a step failed.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$from = Join-Path $root 'sample'
$to = Join-Path $root 'target/walk-sample'

if (-not (Test-Path -LiteralPath $from -PathType Container)) {
    Write-Host ('no sample folder at {0}' -f $from)
    exit 1
}
if (Test-Path -LiteralPath $to) { Remove-Item -LiteralPath $to -Recurse -Force }
New-Item -ItemType Directory -Path $to -Force | Out-Null
Copy-Item -Path (Join-Path $from '*') -Destination $to -Recurse -Force

$ErrorActionPreference = 'Continue'
$id = @('-c', 'user.name=sample', '-c', 'user.email=sample@example.invalid', '-c', 'commit.gpgsign=false')
& git -C $to init -q
if ($LASTEXITCODE -ne 0) { Write-Host 'git init failed'; exit 1 }
& git -C $to config user.name sample
if ($LASTEXITCODE -ne 0) { Write-Host 'git config failed'; exit 1 }
& git -C $to config user.email sample@example.invalid
if ($LASTEXITCODE -ne 0) { Write-Host 'git config failed'; exit 1 }
& git -C $to -c core.autocrlf=false add -A
if ($LASTEXITCODE -ne 0) { Write-Host 'git add failed'; exit 1 }
& git -C $to @id commit -q -m 'the sample, as the repository holds it'
if ($LASTEXITCODE -ne 0) { Write-Host 'git commit failed'; exit 1 }

Write-Host 'sample ready: target/walk-sample'
exit 0
