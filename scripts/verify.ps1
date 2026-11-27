<#
.SYNOPSIS
    The gate: every check of this repository in one command, one line a check, one exit code.

.DESCRIPTION
    Runs, in order, every check below, and prints one line for each (ok, red or skipped), then
    "green" or "red" as its last line. A red check prints the few lines that say why above its
    own line; the run goes on, so one run shows every red.
      1. the workspace's tests          cargo test --workspace --offline
      2. the reviews                    scripts/check_reviews.ps1 -Summary (a report, never red),
                                        then its cases, scripts/test_reviews.ps1, run from a copy
                                        under target/ (two folders deep, where the script looks
                                        for its root, so its scratch folder stays in target/)
      3. the method's copies            scripts/check_method.ps1: red when a copy differs or
                                        is missing; status.yaml's comment lines against the
                                        method's head are said on the line, never red (the
                                        table is Lorisse's, not a builder's to port)
      4. the short-comment rule         scripts/check_comments.ps1
      5. the plugin's validation        claude plugin validate, on the root (the marketplace)
                                        and on plugin/, with the newest claude.exe of the VS Code
                                        extension; none found: skipped, never red
      6. the package                    plugin/ holds only the card, its page, its image, .mcp.json, skills/, server/, hooks/
      7. the versions                   the marketplace entry names no version, the card one
      8. the scripts                    every .ps1 of the repository is plain ASCII and opens on
                                        a help block (<# with .SYNOPSIS)
      9. the clauses                    in every part, each clause of its CONTRACT.md has one row
                                        in the VALIDATION.md beside it; no contract past 4 themes
                                        or 20 clauses, no validation past 12 steps by hand
     10. the sample                     sample/ has no change against git (the index) after the
                                        tests, and no file git does not know

    Nothing is downloaded: cargo runs with --offline. Writes under target/ only.

.PARAMETER Root
    Another checkout of this repository to run the gate on (a worktree). Default: the
    repository this script sits in.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1

.NOTES
    Plain ASCII, Windows PowerShell 5.1 compatible (no &&, no ternaries). Exit code 0 = green,
    1 = at least one check red.
#>
[CmdletBinding()]
param([string] $Root)

$ErrorActionPreference = 'Continue'
$repo = Split-Path -Parent $PSScriptRoot
if ($Root) { $repo = (Resolve-Path -LiteralPath $Root).Path }
$scripts = Join-Path $repo 'scripts'
Set-Location $repo
$script:red = 0

# One line a check: its verdict, its name, what it found.
function Write-Check([string] $verdict, [string] $name, [string] $said) {
    if ($verdict -eq 'red') { $script:red++ }
    Write-Host ('{0,-7} {1}: {2}' -f $verdict, $name, $said)
}

# The last lines of an output, for a red check.
function Write-Why([object[]] $lines, [int] $count = 8) {
    $text = @($lines | ForEach-Object { "$_" } | Where-Object { $_.Trim() })
    $text | Select-Object -Last $count | ForEach-Object { Write-Host ('        {0}' -f $_) }
}

# A PowerShell script of this repository, its lines and its exit code.
function Invoke-Script([string] $path, [string[]] $more) {
    $lines = @(& powershell -NoProfile -ExecutionPolicy Bypass -File $path @more 2>&1 | ForEach-Object { "$_" })
    return [pscustomobject]@{ lines = $lines; code = $LASTEXITCODE }
}

# 1. The workspace's tests.
$out = @(& cargo test --workspace --offline 2>&1 | ForEach-Object { "$_" })
$code = $LASTEXITCODE
$passed = 0; $failed = 0
foreach ($l in $out) {
    if ($l -match '^test result: \w+\. (\d+) passed; (\d+) failed') { $passed += [int] $Matches[1]; $failed += [int] $Matches[2] }
}
if ($code -eq 0 -and $failed -eq 0) {
    Write-Check 'ok' 'tests' ('{0} passed, 0 failed' -f $passed)
} else {
    Write-Why $out
    Write-Check 'red' 'tests' ('{0} passed, {1} failed, exit {2}' -f $passed, $failed, $code)
}

# 2. The review report (a report, never red) and its cases, from a copy two folders deep under target/.
$r = Invoke-Script (Join-Path $scripts 'check_reviews.ps1') @('-Summary')
$report = @($r.lines | Where-Object { $_.Trim() }) | Select-Object -Last 1
$stage = Join-Path $repo 'target/gate/reviews/checks'
if (Test-Path -LiteralPath (Join-Path $repo 'target/gate')) { Remove-Item -LiteralPath (Join-Path $repo 'target/gate') -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
foreach ($f in 'check_reviews.ps1', 'test_reviews.ps1', 'review_cases.json') {
    Copy-Item -LiteralPath (Join-Path $scripts $f) -Destination (Join-Path $stage $f)
}
$r = Invoke-Script (Join-Path $stage 'test_reviews.ps1') @()
$last = @($r.lines | Where-Object { $_.Trim() }) | Select-Object -Last 1
if ($r.code -eq 0) { Write-Check 'ok' 'reviews' ('{0}; {1}' -f $report, $last) } else { Write-Why $r.lines; Write-Check 'red' 'reviews' ('{0}; {1}' -f $report, $last) }

# 3. The method's copies: every one the same.
$r = Invoke-Script (Join-Path $scripts 'check_method.ps1') @()
$off = @($r.lines | Where-Object { $_ -match '\s(differs|missing here|missing in the method)' -and $_ -notmatch '^method ' })
$table = @($off | Where-Object { $_ -match '^status\.yaml\s' })
$off = @($off | Where-Object { $_ -notmatch '^status\.yaml\s' })
$last = @($r.lines | Where-Object { $_.Trim() }) | Select-Object -Last 1
if ($table.Count) { $last = '{0}; status.yaml''s comment lines differ from the method''s head (the table is Lorisse''s: noted, not red)' -f $last }
if ($off.Count -eq 0 -and $r.code -eq 0) { Write-Check 'ok' 'method copies' $last } else { Write-Why $off; Write-Check 'red' 'method copies' $last }

# 4. The short-comment rule.
$r = Invoke-Script (Join-Path $scripts 'check_comments.ps1') @()
$last = @($r.lines | Where-Object { $_.Trim() }) | Select-Object -Last 1
if ($r.code -eq 0) { Write-Check 'ok' 'comment rule' $last } else { Write-Why $r.lines; Write-Check 'red' 'comment rule' $last }

# 5. The plugin's validation, with the newest claude.exe of the VS Code extension.
$claude = $null
$ext = Join-Path $env:USERPROFILE '.vscode/extensions'
if (Test-Path -LiteralPath $ext) {
    $found = @(Get-ChildItem -LiteralPath $ext -Directory -Filter 'anthropic.claude-code-*' | ForEach-Object {
        $exe = Join-Path $_.FullName 'resources/native-binary/claude.exe'
        $v = [regex]::Match($_.Name, 'anthropic\.claude-code-(\d+)\.(\d+)\.(\d+)')
        if ($v.Success -and (Test-Path -LiteralPath $exe)) {
            [pscustomobject]@{ exe = $exe; key = [version] ('{0}.{1}.{2}' -f $v.Groups[1].Value, $v.Groups[2].Value, $v.Groups[3].Value) }
        }
    } | Sort-Object key)
    if ($found.Count) { $claude = $found[-1] }
}
if (-not $claude) {
    Write-Check 'skipped' 'plugin validation' 'no claude.exe under ~/.vscode/extensions/anthropic.claude-code-*'
} else {
    $bad = @()
    foreach ($target in @($repo, (Join-Path $repo 'plugin'))) {
        $out = @(& $claude.exe plugin validate $target 2>&1 | ForEach-Object { "$_" })
        if ($LASTEXITCODE -ne 0) { Write-Why $out; $bad += $target }
    }
    if ($bad.Count -eq 0) {
        Write-Check 'ok' 'plugin validation' ('the marketplace and plugin/ pass, claude {0}' -f $claude.key)
    } else {
        Write-Check 'red' 'plugin validation' ('refused: {0}' -f ($bad -join ', '))
    }
}

# 6. The package: plugin/ holds only what the Claude app installs.
$allowed = @('.claude-plugin', '.mcp.json', 'skills', 'server', 'hooks', 'README.md', 'icon.svg')
$plugin = Join-Path $repo 'plugin'
$extra = @()
if (Test-Path -LiteralPath $plugin) {
    $extra = @(Get-ChildItem -LiteralPath $plugin -Force | Where-Object { $allowed -notcontains $_.Name } | ForEach-Object { $_.Name })
    $card = @(Get-ChildItem -LiteralPath (Join-Path $plugin '.claude-plugin') -Force -ErrorAction SilentlyContinue | Where-Object { $_.Name -ne 'plugin.json' } | ForEach-Object { '.claude-plugin/' + $_.Name })
    $extra += $card
    $missing = @($allowed | Where-Object { -not (Test-Path -LiteralPath (Join-Path $plugin $_)) })
} else {
    $missing = $allowed
}
if ($extra.Count -eq 0 -and $missing.Count -eq 0) {
    Write-Check 'ok' 'package' 'plugin/ holds the card, its page and its image, .mcp.json, skills/, server/, hooks/ and nothing else'
} else {
    Write-Check 'red' 'package' ('more: [{0}], missing: [{1}]' -f ($extra -join ', '), ($missing -join ', '))
}

# 7. The versions: none in the marketplace entry, one on the card.
try {
    $market = Get-Content -LiteralPath (Join-Path $repo '.claude-plugin/marketplace.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $cardJson = Get-Content -LiteralPath (Join-Path $plugin '.claude-plugin/plugin.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $named = @($market.plugins | Where-Object { $_.PSObject.Properties.Name -contains 'version' } | ForEach-Object { $_.name })
    if ($named.Count -eq 0 -and $cardJson.version) {
        Write-Check 'ok' 'versions' ('the card names {0}, the marketplace entry none' -f $cardJson.version)
    } else {
        Write-Check 'red' 'versions' ('entries with a version: [{0}], the card: [{1}]' -f ($named -join ', '), $cardJson.version)
    }
} catch {
    Write-Check 'red' 'versions' ('cannot read the cards: {0}' -f $_.Exception.Message)
}

# 8. The scripts: plain ASCII, a help block on top.
$ps = @(Get-ChildItem -LiteralPath $repo -Recurse -File -Filter '*.ps1' -Force | Where-Object { $_.FullName -notmatch '[\\/](target|\.git)[\\/]' })
$off = @()
foreach ($f in $ps) {
    $bytes = [IO.File]::ReadAllBytes($f.FullName)
    $ascii = -not ($bytes | Where-Object { $_ -gt 127 } | Select-Object -First 1)
    $text = [IO.File]::ReadAllText($f.FullName)
    $help = $text.TrimStart().StartsWith('<#') -and ($text -match '\.SYNOPSIS')
    if (-not $ascii) { $off += ('{0}: not plain ASCII' -f $f.FullName.Substring($repo.Length + 1)) }
    if (-not $help) { $off += ('{0}: no help block on top' -f $f.FullName.Substring($repo.Length + 1)) }
}
if ($off.Count -eq 0) { Write-Check 'ok' 'scripts' ('{0} .ps1 file(s), plain ASCII, each with its help block' -f $ps.Count) } else { Write-Why $off 20; Write-Check 'red' 'scripts' ('{0} finding(s)' -f $off.Count) }

# 9. The clauses: in every part (a folder holding a CONTRACT.md), each clause one row in the
# VALIDATION.md beside it; and a contract a person reads to its end: at most 4 themes and 20
# clauses, its validation at most 12 steps by hand. Past that the part is cut in two.
$maxThemes = 4
$maxClauses = 20
$maxSteps = 12
$fixed = @('Role', 'Does and does not', 'Depends on', 'Behaviour', 'Wanted, not promised yet', 'Non-goals')
$contracts = @(Get-ChildItem -LiteralPath $repo -Recurse -File -Filter 'CONTRACT.md' |
    Where-Object { $_.FullName.Substring($repo.Length + 1) -notmatch '^(target|docs|\.git|\.claude)[\\/]' })
$off = @()
$total = 0
foreach ($file in $contracts) {
    $where = $file.FullName.Substring($repo.Length + 1).Replace('\', '/')
    $contract = Get-Content -LiteralPath $file.FullName -Encoding UTF8
    $beside = Join-Path $file.DirectoryName 'VALIDATION.md'
    if (-not (Test-Path -LiteralPath $beside)) { $off += ('{0}: no VALIDATION.md beside it' -f $where); continue }
    $validation = Get-Content -LiteralPath $beside -Encoding UTF8
    $clauses = @($contract | ForEach-Object { if ($_ -match '^\s*- \*\*(C-\d+\.\d+)\*\*') { $Matches[1] } })
    $total += $clauses.Count
    foreach ($c in $clauses) {
        $rows = @($validation | Where-Object { $_ -match ('^\|\s*{0}\s*\|' -f [regex]::Escape($c)) }).Count
        if ($rows -ne 1) { $off += ('{0}: {1}: {2} row(s)' -f $where, $c, $rows) }
    }
    $themes = @($contract | ForEach-Object { if ($_ -match '^## \d+\.\s+(.+?)\s*$') { $Matches[1] } } | Where-Object { $fixed -notcontains $_ })
    if ($themes.Count -gt $maxThemes) { $off += ('{0}: {1} themes, {2} at most: cut the part in two' -f $where, $themes.Count, $maxThemes) }
    if ($clauses.Count -gt $maxClauses) { $off += ('{0}: {1} clauses, {2} at most: cut the part in two' -f $where, $clauses.Count, $maxClauses) }
    $steps = @($validation | Where-Object { $_ -match '^\s*- \[[ xX]\] \*\*H-\d+\*\*' }).Count
    if ($steps -gt $maxSteps) { $off += ('{0}: {1} steps by hand, {2} at most' -f $where, $steps, $maxSteps) }
}
if ($total -gt 0 -and $off.Count -eq 0) { Write-Check 'ok' 'clauses' ('{0} clause(s) in {1} contract(s), one row each, none past {2} themes or {3} clauses' -f $total, $contracts.Count, $maxThemes, $maxClauses) } else { Write-Why $off 20; Write-Check 'red' 'clauses' ('{0} clause(s) in {1} contract(s), {2} off' -f $total, $contracts.Count, $off.Count) }

# 10. The sample: as git holds it, after the tests.
$changed = @(& git -C $repo diff --name-only -- sample 2>$null)
$unknown = @(& git -C $repo ls-files --others --exclude-standard -- sample 2>$null)
if ($changed.Count -eq 0 -and $unknown.Count -eq 0) {
    Write-Check 'ok' 'sample' 'no change against git'
} else {
    Write-Why ($changed + $unknown) 20
    Write-Check 'red' 'sample' ('{0} changed, {1} unknown to git' -f $changed.Count, $unknown.Count)
}

if ($script:red -eq 0) { Write-Host 'green'; exit 0 }
Write-Host 'red'
exit 1
