<#
.SYNOPSIS
    The review report's cases: what waits for the owner's read, judged on made-up repositories. Run by the gate
    (method/gate/verify.ps1), next to the review report's step.

.DESCRIPTION
    The cases are data, in review_cases.json beside this script, so that a reader in another language follows the
    same ones: each a made-up repository (its table, its files written and committed at given hours, its reader's
    record), and what the report must say of it. This script builds each under target/test_reviews-<process id>/
    of the repository it sits in, runs check_reviews.ps1 -Root on it (its data, its rows, its summary), and compares.
    Then the data's form over every case: each entry holds its fields, its words are the report's; nothing is
    written in the repository read and nothing of it is run; the report exits 0 whatever it finds.
    Prints one line a failed check, then "review cases: ALL GREEN (n checks)" or "review cases: n of m checks
    failed"; a green run takes its folder away.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File method/checks/test_reviews.ps1

.NOTES
    Windows PowerShell 5.1 compatible. Writes under target/ only. Exit 0 all green, else 1.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$check = Join-Path $PSScriptRoot 'check_reviews.ps1'
$utf8 = New-Object System.Text.UTF8Encoding($false)
$base = Join-Path $root ('target/test_reviews-{0}' -f $PID)
if (Test-Path -LiteralPath $base) { Remove-Item -LiteralPath $base -Recurse -Force }
New-Item -ItemType Directory -Path $base -Force | Out-Null

$script:n = 0; $script:failed = 0
function Check([string] $name, [bool] $ok, [string] $got = '') {
    $script:n++
    if (-not $ok) { $script:failed++; Write-Host ("FAIL {0}{1}" -f $name, $(if ($got) { " (got: $got)" } else { '' })) }
}
function Put([string] $path, [string[]] $lines) {
    $dir = Split-Path -Parent $path
    if (-not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    [IO.File]::WriteAllText($path, (($lines -join "`n") + "`n"), $utf8)
}
# A commit of everything in a made-up repository, at a given hour (author and committer alike).
function CommitAt([string] $repo, [string] $when) {
    $env:GIT_AUTHOR_DATE = $when; $env:GIT_COMMITTER_DATE = $when
    $null = & git.exe -C $repo add -A 2>$null
    $null = & git.exe -C $repo -c user.name=fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false commit -q -m $when 2>$null
    Remove-Item Env:GIT_AUTHOR_DATE, Env:GIT_COMMITTER_DATE -ErrorAction SilentlyContinue
}
# The report on a repository, its lines and its exit code.
function Report([string] $repo, [string[]] $more) {
    $prev = $null
    try { $prev = [Console]::OutputEncoding; [Console]::OutputEncoding = $utf8 } catch { $prev = $null }
    try { $lines = @(& powershell -NoProfile -ExecutionPolicy Bypass -File $check -Root $repo @more 2>$null | ForEach-Object { "$_" }) } finally { if ($prev) { try { [Console]::OutputEncoding = $prev } catch { } } }
    return [pscustomobject]@{ lines = $lines; code = $LASTEXITCODE }
}
function J($v) { return (ConvertTo-Json -InputObject $v -Depth 6 -Compress) }
function Files([string] $repo) { return @(Get-ChildItem -LiteralPath $repo -Recurse -File -Force | Where-Object { $_.FullName -notmatch '\\\.git\\' } | ForEach-Object { '{0} {1} {2}' -f $_.FullName.Substring($repo.Length), $_.Length, $_.LastWriteTimeUtc.Ticks } | Sort-Object)
}

$data = [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'review_cases.json'), $utf8) | ConvertFrom-Json
$marker = Join-Path $base 'planted-ran.txt'
$fields = 'file', 'state', 'text', 'answered', 'date', 'mark', 'unknown_mark', 'since', 'hour', 'base', 'change', 'clauses', 'wording_only'
$all = @(); $codes = @(); $untouched = $true
$i = 0
foreach ($c in @($data.cases)) {
    $i++
    $repo = Join-Path $base ('{0:d2}' -f $i)
    New-Item -ItemType Directory -Path $repo -Force | Out-Null
    $null = & git.exe -C $repo init -q 2>$null
    $table = $data.default_table
    if ($c.PSObject.Properties['table']) { $table = $c.table }
    Put (Join-Path $repo 'status.yaml') @($table)
    # a review check of the repository's own, never to be run: it writes a marker when it is
    Put (Join-Path $repo 'scripts/check_reviews.ps1') @(('Set-Content -LiteralPath ''{0}'' -Value ran' -f $marker), 'exit 0')
    foreach ($s in @($c.steps)) {
        foreach ($p in $s.files.PSObject.Properties) { Put (Join-Path $repo $p.Name) @($p.Value) }
        if ($s.PSObject.Properties['at']) { CommitAt $repo $s.at }
    }
    if ($c.PSObject.Properties['reading']) {
        $docs = [ordered]@{}
        foreach ($p in $c.reading.PSObject.Properties) { $docs[$p.Name] = [ordered]@{ marked = [string] $p.Value } }
        Put (Join-Path $repo 'target/reading.json') @((J ([ordered]@{ version = 1; docs = $docs })))
    }
    $before = Files $repo
    $r = Report $repo @('-Json')
    $codes += $r.code
    $j = $null
    $line = @($r.lines | Where-Object { $_.TrimStart().StartsWith('{') }) | Select-Object -Last 1
    if ($line) { try { $j = ConvertFrom-Json -InputObject $line } catch { $j = $null } }
    Check ('{0}: the report gives its data' -f $c.name) ($null -ne $j) ($r.lines -join ' | ')
    if (-not $j) { continue }
    $all += @($j.files)
    $by = @{}; foreach ($e in @($j.files)) { $by[[string] $e.file] = $e }
    if ($c.PSObject.Properties['expect']) {
        foreach ($p in $c.expect.PSObject.Properties) {
            $e = $by[$p.Name]; $bad = @()
            if (-not $e) { $bad += 'not listed' }
            else {
                foreach ($k in $p.Value.PSObject.Properties) {
                    if (-not $e.PSObject.Properties[$k.Name]) { $bad += ('{0} missing' -f $k.Name) }
                    elseif ((J $e.($k.Name)) -cne (J $k.Value)) { $bad += ('{0}={1}' -f $k.Name, (J $e.($k.Name))) }
                }
            }
            Check ('{0}: {1}' -f $c.name, $p.Name) ($bad.Count -eq 0) ($bad -join ', ')
        }
    }
    $got = @($j.files | ForEach-Object { [string] $_.file })
    if ($c.PSObject.Properties['listed']) { Check ('{0}: the list, in its order' -f $c.name) (($got -join ',') -ceq (@($c.listed) -join ',')) ($got -join ',') }
    if ($c.PSObject.Properties['absent']) {
        $there = @(@($c.absent) | Where-Object { $got -contains $_ })
        Check ('{0}: the pages left out' -f $c.name) ($there.Count -eq 0) ($there -join ',')
    }
    if ($c.PSObject.Properties['summary'] -or $c.PSObject.Properties['rows'] -or $c.PSObject.Properties['lines']) {
        $printed = Report $repo @()
        $codes += $printed.code
        if ($c.PSObject.Properties['summary']) {
            $sum = Report $repo @('-Summary')
            $codes += $sum.code
            $shown = @($sum.lines | Where-Object { $_.Trim() })
            Check ('{0}: the summary, in the data, after the rows and alone' -f $c.name) (($j.summary -ceq $c.summary) -and ((@($printed.lines | Where-Object { $_.Trim() }) | Select-Object -Last 1) -ceq $c.summary) -and ($shown.Count -eq 1) -and ($shown[0] -ceq $c.summary)) ('{0} || {1}' -f $j.summary, ($shown -join ' | '))
        }
        if ($c.PSObject.Properties['rows']) {
            $want = @(@($c.rows) | ForEach-Object { '{0,-50} {1}   {2}' -f $_[0], $_[1], $_[2] })
            $rows = @($printed.lines | Where-Object { $_ -and -not $_.StartsWith(' ') -and -not $_.StartsWith('reviews: ') })
            Check ('{0}: the rows, one a file, in its order' -f $c.name) (($rows -join "`n") -ceq ($want -join "`n")) ($rows -join ' | ')
        }
        if ($c.PSObject.Properties['lines']) {
            $miss = @(@($c.lines) | Where-Object { $printed.lines -cnotcontains $_ })
            Check ('{0}: the lines under the rows' -f $c.name) ($miss.Count -eq 0) ($printed.lines -join ' | ')
        }
    }
    if (((Files $repo) -join "`n") -cne ($before -join "`n")) { $untouched = $false }
}

# The data's form over every case (C-5.7): each entry its fields, its words the report's.
$missing = @($all | Where-Object { $e = $_; @($fields | Where-Object { -not $e.PSObject.Properties[$_] }).Count })
Check 'the data: every entry holds its fields' (($all.Count -gt 0) -and ($missing.Count -eq 0)) (@($missing | ForEach-Object { $_.file }) -join ',')
$words = @($all | Where-Object { ($_.state -notin 'reviewed', 'changed since', 'answered', 'never reviewed', 'no line') -or ($_.text -notin 'reviewed', 'changed since', 'never reviewed', 'no line') -or ($_.mark -notin 'read', 'ok', 'to rework', '-') })
Check 'the data: a state, a text and a mark of the report''s words' ($words.Count -eq 0) (@($words | ForEach-Object { '{0} {1} {2} {3}' -f $_.file, $_.state, $_.text, $_.mark }) -join ', ')
$state = @($all | Where-Object { $_.state -cne $(if ($_.answered) { 'answered' } else { $_.text }) })
Check 'the data: the state is its text''s, or answered when a note is' ($state.Count -eq 0) (@($state | ForEach-Object { $_.file }) -join ',')
$moment = @($all | Where-Object { ($null -ne $_.date) -ne ($null -ne $_.since -and $null -ne $_.hour) -or ($_.since -and $_.since -notin 'hour', 'day') })
Check 'the data: a page read has the moment compared with, hour or day; another has none' ($moment.Count -eq 0) (@($moment | ForEach-Object { '{0} {1} {2}' -f $_.file, $_.since, $_.hour }) -join ', ')
$chg = @($all | Where-Object { ($_.text -eq 'changed since') -ne [bool] $_.change })
Check 'the data: the day of the last change for a page changed, and for it only' ($chg.Count -eq 0) (@($chg | ForEach-Object { $_.file }) -join ',')
$cl = @($all | Where-Object { [bool] $_.clauses -ne (($_.text -eq 'changed since') -and ((Split-Path -Leaf $_.file) -ceq 'CONTRACT.md')) -or ($_.wording_only -and -not $_.clauses) })
Check 'the data: the clauses for a contract changed, and for it only' ($cl.Count -eq 0) (@($cl | ForEach-Object { $_.file }) -join ',')
Check 'the report exits 0 whatever it finds (C-5.3)' (@($codes | Where-Object { $_ -ne 0 }).Count -eq 0) ($codes -join ',')
Check 'nothing written in the repository read' $untouched
Check 'nothing of the repository read is run: its own review check never ran' (-not (Test-Path -LiteralPath $marker))

if ($script:failed) { Write-Host ("review cases: {0} of {1} checks failed (the fixtures kept: {2})" -f $script:failed, $script:n, $base); exit 1 }
Remove-Item -LiteralPath $base -Recurse -Force -ErrorAction SilentlyContinue
Write-Host ("review cases: ALL GREEN ({0} checks)" -f $script:n)
exit 0
