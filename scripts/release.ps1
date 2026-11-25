<#
.SYNOPSIS
    Finishes a release after a push: waits for GitHub's build, raises the plugin's version,
    pushes it, updates this machine's copy and checks that it answers with the new version.

.DESCRIPTION
    Run once the source is pushed to main, on the owner's word. In order, stopping at the
    first step that fails:
      1. waits until GitHub's build of the pushed commit has ended, and ended well;
      2. pulls the commit that carries the two built programs;
      3. raises the version of plugin/.claude-plugin/plugin.json to the program's own
         (crates/tools/Cargo.toml), when the program's is the higher;
      4. runs the gate (scripts/verify.ps1): green or nothing is pushed;
      5. commits that one line and pushes it;
      6. refreshes this machine's copy of the marketplace and updates the installed plugin;
      7. asks the installed plugin where it is, and checks the version it answers.
    So an installed copy never gets a new version number with an old program inside: the
    number moves last, after the programs are in the repository.
    With -NoInstall, steps 6 and 7 are left out.

.PARAMETER NoInstall
    Stop after the push: do not touch this machine's installed plugin.

.PARAMETER Minutes
    How long to wait for GitHub's build; 25 by default.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1

.NOTES
    Plain ASCII, Windows PowerShell 5.1 compatible. Exit code 0 = released, 1 = a step failed.
#>
[CmdletBinding()]
param([switch]$NoInstall, [int]$Minutes = 25)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$repo = 'FreakingDuck/lorisse-plugin'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

function Stop-Release([string] $Why) { Write-Host ('release stopped: ' + $Why); exit 1 }

# 1. GitHub's build of the pushed commit.
$ErrorActionPreference = 'Continue'
$pushed = (& git -C $root rev-parse origin/main).Trim()
$ErrorActionPreference = 'Stop'
if (-not $pushed) { Stop-Release 'no pushed commit found' }
$until = (Get-Date).AddMinutes($Minutes)
$ended = $null
$missed = 0
while ((Get-Date) -lt $until) {
    try {
        $runs = Invoke-RestMethod -Uri ('https://api.github.com/repos/{0}/actions/runs?per_page=5' -f $repo) -Headers @{ 'User-Agent' = 'release-script' }
        $run = @($runs.workflow_runs | Where-Object { $_.head_sha -eq $pushed }) | Select-Object -First 1
        if ($run -and $run.status -eq 'completed') { $ended = $run; break }
        if (-not $run) {
            # GitHub takes a moment to start a build. After two minutes without one, there is
            # none for this commit: nothing of the program changed, or the programs are in.
            $missed++
            if ($missed -ge 4) { $ended = 'none'; break }
        }
    } catch { }
    Start-Sleep -Seconds 30
}
if ($null -eq $ended) { Stop-Release ('GitHub''s build did not end within {0} minutes' -f $Minutes) }
if ($ended -eq 'none') {
    Write-Host 'build: none for the pushed commit'
} elseif ($ended.conclusion -ne 'success') {
    # Say which machine and which step failed, so nobody has to go and look.
    try {
        $jobs = Invoke-RestMethod -Uri $ended.jobs_url -Headers @{ 'User-Agent' = 'release-script' }
        foreach ($job in @($jobs.jobs | Where-Object { $_.conclusion -ne 'success' })) {
            $steps = @($job.steps | Where-Object { $_.conclusion -eq 'failure' } | ForEach-Object { $_.name }) -join ', '
            Write-Host ('  {0}: {1}; failed step(s): {2}' -f $job.name, $job.conclusion, $steps)
            if ($job.conclusion -eq 'failure') {
                $notes = Invoke-RestMethod -Uri ('https://api.github.com/repos/{0}/check-runs/{1}/annotations' -f $repo, $job.id) -Headers @{ 'User-Agent' = 'release-script' }
                foreach ($note in @($notes)) { Write-Host ('    ' + $note.message) }
            }
        }
        Write-Host ('  the run: ' + $ended.html_url)
    } catch { }
    Stop-Release ('GitHub''s build ended as ' + $ended.conclusion)
} else {
    Write-Host 'build: ended well'
}

# 2. The built programs.
$ErrorActionPreference = 'Continue'
& git -C $root pull --ff-only
if ($LASTEXITCODE -ne 0) { Stop-Release 'the pull failed' }
$ErrorActionPreference = 'Stop'

# 3. The version: the program's own, when it is the higher.
$cargo = Get-Content -LiteralPath (Join-Path $root 'crates/tools/Cargo.toml') -Raw
if ($cargo -notmatch '(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"') { Stop-Release 'no version in crates/tools/Cargo.toml' }
$program = [version]$Matches[1]
$cardPath = Join-Path $root 'plugin/.claude-plugin/plugin.json'
$card = Get-Content -LiteralPath $cardPath -Raw
if ($card -notmatch '"version"\s*:\s*"([0-9]+\.[0-9]+\.[0-9]+)"') { Stop-Release 'no version in the plugin''s card' }
$current = [version]$Matches[1]
$released = $current
if ($program -gt $current) {
    $card = $card -replace '("version"\s*:\s*")[0-9]+\.[0-9]+\.[0-9]+(")', ('${1}' + $program.ToString() + '${2}')
    [IO.File]::WriteAllText($cardPath, $card, (New-Object Text.UTF8Encoding $false))
    $released = $program
    Write-Host ('version: {0} -> {1}' -f $current, $program)
} else {
    Write-Host ('version: stays {0} (the program is {1})' -f $current, $program)
}

# 4. The gate.
$ErrorActionPreference = 'Continue'
$gate = & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify.ps1')
$gateCode = $LASTEXITCODE
$gate | ForEach-Object { Write-Host $_ }
if ($gateCode -ne 0) { Stop-Release 'the gate is red; nothing pushed' }

# 5. The one line, committed and pushed.
if ($released -ne $current) {
    & git -C $root commit -m ('version {0}: with the programs GitHub built' -f $released) -- plugin/.claude-plugin/plugin.json
    if ($LASTEXITCODE -ne 0) { Stop-Release 'the commit failed' }
}
& git -C $root push origin main
if ($LASTEXITCODE -ne 0) { Stop-Release 'the push failed' }
Write-Host ('pushed: version {0}' -f $released)
if ($NoInstall) { Write-Host ('released {0}' -f $released); exit 0 }

# 6 and 7. This machine's copy, and what it answers.
$claude = Get-ChildItem -Path (Join-Path $HOME '.vscode/extensions') -Filter 'anthropic.claude-code-*' -Directory -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path $_.FullName 'resources/native-binary/claude.exe' } |
    Where-Object { Test-Path -LiteralPath $_ } |
    Sort-Object |
    Select-Object -Last 1
if (-not $claude) { Stop-Release 'pushed, but no claude program found to update this machine''s copy' }
& $claude plugin marketplace update lorisse
if ($LASTEXITCODE -ne 0) { Stop-Release 'pushed, but the marketplace could not be refreshed' }
& $claude plugin update lorisse@lorisse
if ($LASTEXITCODE -ne 0) { Stop-Release 'pushed, but the installed plugin could not be updated' }
$said = & $claude -p 'Call the lorisse tool named where and print its answer word for word, nothing else.' --model haiku
$said | ForEach-Object { Write-Host $_ }
$programSaid = ($said -join ' ')
if ($programSaid -notmatch [regex]::Escape($program.ToString())) {
    Stop-Release ('pushed and updated, but the installed plugin does not answer with version {0}' -f $program)
}
Write-Host ('released {0}: installed here and answering' -f $released)
exit 0
