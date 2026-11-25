<#
.SYNOPSIS
    Tries the plugin as it stands in this folder, with Claude, on a fresh copy of the sample.

.DESCRIPTION
    One call for the whole try, so nothing is asked in the middle:
      1. makes the sample's copy (scripts/walk_sample.ps1);
      2. builds the program from the source (cargo build --release --offline);
      3. copies plugin/ to target/try-plugin and puts the built program in its server/
         (plugin/ itself is never touched);
      4. runs the newest claude program found under the VS Code extensions, once, in the
         sample's copy, with that plugin and the prompt given;
      5. prints what Claude answered, then the history of the sample's copy.
    Without -Allow, no tool is allowed beforehand: a tool that runs was let through by the
    plugin's own guard. With -Allow, Lorisse's tools are allowed on the command line.

.PARAMETER Prompt
    What to say to Claude.

.PARAMETER Allow
    Allow Lorisse's tools on the command line, the guard aside.

.PARAMETER FirstDay
    Run in an empty folder with no memory at all, to try the first day: the folder Lorisse
    makes goes under target/try-home, never under this machine's own home.

.PARAMETER Model
    The model to run; haiku by default.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/try_plugin.ps1 -Prompt "Find boiler"

.NOTES
    Plain ASCII, Windows PowerShell 5.1 compatible. Exit code 0 = the run ended, 1 = a step failed.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Prompt,
    [switch]$Allow,
    [switch]$FirstDay,
    [string]$Model = 'haiku'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$sample = Join-Path $root 'target/walk-sample'
$try = Join-Path $root 'target/try-plugin'

& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'walk_sample.ps1')
if ($LASTEXITCODE -ne 0) { Write-Host 'the sample could not be made'; exit 1 }

$ErrorActionPreference = 'Continue'
& cargo build --release --offline -p lorisse --manifest-path (Join-Path $root 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { Write-Host 'the build failed'; exit 1 }
$ErrorActionPreference = 'Stop'

if (Test-Path -LiteralPath $try) { Remove-Item -LiteralPath $try -Recurse -Force }
Copy-Item -LiteralPath (Join-Path $root 'plugin') -Destination $try -Recurse -Force
$built = Join-Path $root 'target/release/lorisse.exe'
if (-not (Test-Path -LiteralPath $built)) { $built = Join-Path $root 'target/release/lorisse' }
Copy-Item -LiteralPath $built -Destination (Join-Path $try 'server') -Force

$claude = Get-ChildItem -Path (Join-Path $HOME '.vscode/extensions') -Filter 'anthropic.claude-code-*' -Directory -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path $_.FullName 'resources/native-binary/claude.exe' } |
    Where-Object { Test-Path -LiteralPath $_ } |
    Sort-Object |
    Select-Object -Last 1
if (-not $claude) { Write-Host 'no claude program found'; exit 1 }

$more = @()
if ($Allow) { $more = @('--allowedTools', 'mcp__plugin_lorisse_lorisse') }

# The try never touches this machine's own memory nor its home: the program is given a home
# made for the try, and no setting. With -FirstDay it runs in an empty folder, with no memory.
$tryHome = Join-Path $root 'target/try-home'
if (Test-Path -LiteralPath $tryHome) { Remove-Item -LiteralPath $tryHome -Recurse -Force }
New-Item -ItemType Directory -Path $tryHome -Force | Out-Null
$homeText = $tryHome.Replace('\', '/')
$mcp = '{"mcpServers":{"lorisse":{"type":"stdio","command":"${CLAUDE_PLUGIN_ROOT}/server/lorisse","args":[],"env":{"LORISSE_MEMORY_FOLDER":"","CLAUDE_PLUGIN_OPTION_MEMORY_FOLDER":"","LORISSE_HOME":"' + $homeText + '"}}}}'
[IO.File]::WriteAllText((Join-Path $try '.mcp.json'), $mcp, (New-Object Text.UTF8Encoding $false))
$where = $sample
if ($FirstDay) {
    $where = Join-Path $root 'target/try-empty'
    if (Test-Path -LiteralPath $where) { Remove-Item -LiteralPath $where -Recurse -Force }
    New-Item -ItemType Directory -Path $where -Force | Out-Null
}

Write-Host ('--- Claude ({0}) answers:' -f $Model)
$ErrorActionPreference = 'Continue'
Push-Location -LiteralPath $where
try {
    & $claude -p $Prompt --plugin-dir $try --model $Model @more
} finally {
    Pop-Location
}
if ($FirstDay) {
    $made = Join-Path $tryHome 'Lorisse'
    Write-Host '--- the folder made on the first day:'
    if (Test-Path -LiteralPath $made) {
        Get-ChildItem -LiteralPath $made -Recurse -File | Where-Object { $_.FullName -notmatch '[\\/]\.git[\\/]' } | ForEach-Object { Write-Host ('  ' + $_.FullName.Substring($made.Length + 1)) }
        Write-Host '--- its history:'
        & git -C $made log --oneline
    } else {
        Write-Host '  none'
    }
} else {
    Write-Host '--- the sample''s history:'
    & git -C $sample log --oneline
    Write-Host '--- the sample''s state:'
    & git -C $sample status --short
}
Write-Host 'try ended'
exit 0
