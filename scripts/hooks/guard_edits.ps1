<#
.SYNOPSIS
    Hook on file changes: apply the `edits` state of status.yaml, asking once per session.

.DESCRIPTION
    PreToolUse (default): reads the tool call as JSON on stdin, finds the status.yaml row whose
    path is the longest prefix of the file, and answers with its `edits`: open -> silent;
    auto -> silent (open, and buildable by a scheduled run); locked -> refused, naming the row;
    ask -> a permission prompt the first time in a session, then silent for that row (a marker
    under %LOCALAPPDATA%\lorisse-grants\edits-<repo>\<session>-<row>, the repo being the root
    folder's name). PostToolUse (-Grant): the edit went through, so the owner said yes: write
    the marker. A file under no row, or outside any repository with a status.yaml, is open.
    Under a locked row two additions are judged as an ask edit, with a marker of their own
    (lorisse-grants\additions-<repo>\<session>-<row>): in the part's own VALIDATION.md, new rows
    at the end of the run log's table (the heading holding "Run log"), each with the header's
    number of cells; in its own CONTRACT.md, new `- ` items (with their indented lines) in the
    section whose heading holds "Wanted, not promised yet", a lone "none" there replaceable by
    the first. The file after the call is computed (Edit: the replacement applied, replace_all
    honoured; Write: its content) and compared with the file before, by lines, line endings
    normalised; any other change, any other tool, a file that cannot be read, an old text not
    found or found twice without replace_all: refused.
    Rows are the lines `<name>: { ... path: <p> ... edits: <state> ... }` under `parts:`, the
    keys in any order. Every folder above the edited file holding a status.yaml is read, each table
    by its own rows (the longest prefix inside each), and the strictest answer wins (locked over ask
    over open), the nearest table on a tie: a table planted deeper only tightens; the hook guards
    from any session, user-level or the project's. Under several locked rows an addition must pass
    under each. A script the lists let run without asking (an allow rule of lists/settings.user.json
    of this hook's repository, or of a repository's own .claude/settings*.json, names it; read by
    data_folders.ps1 beside the hook) is asked whatever its row says, one marker for them all in a
    session (lorisse-grants\scripts\<session>); under a locked row it is refused; the lists
    unreadable, every script file is asked. A file inside a .git folder, or a worktree's .git file,
    is refused.
    The path is compared after full resolution (`..` resolved, case ignored); a path in a form
    no row can be compared with (an 8.3 short name, a stream suffix, a device or network path)
    is refused, and so is a file in the grant markers' folder. Input that is not JSON, a call
    with no file, an unreadable table or any error of the hook: refused with one line on the
    error stream and exit 2 (a block whatever the rule lists say).

.EXAMPLE
    echo '{"session_id":"s1","tool_name":"Edit","tool_input":{"file_path":"C:\repo\CLAUDE.md"}}' |
        powershell -NoProfile -ExecutionPolicy Bypass -File scripts/hooks/guard_edits.ps1
#>
param([switch]$Grant)
$ErrorActionPreference = 'Stop'
$GrantsRoot = Join-Path $env:LOCALAPPDATA 'lorisse-grants'
# The repository of this hook (its lists name the scripts run without asking), and the lists' reader.
# Either missing: the lists cannot be read, and every script file is asked (C-4.8).
$HookRepo = $null
try { . (Join-Path $PSScriptRoot 'repo_root.ps1'); $HookRepo = Get-HookRepo $PSScriptRoot } catch { $HookRepo = $null }
try { . (Join-Path $PSScriptRoot 'data_folders.ps1') } catch { }

# A refusal when the hook cannot judge: one line on the error stream, exit 2. After the call
# (-Grant) nothing can be blocked: the line only.
function Stop-Refused([string]$why) {
    [Console]::Error.WriteLine(('guard_edits: refused, {0}' -f $why))
    if ($Grant) { exit 0 }
    exit 2
}

# The tables of a file: every folder above it holding a status.yaml, the nearest first; none -> open.
function Find-Roots([string]$file) {
    $out = @()
    $dir = Split-Path -Parent $file
    while ($dir) {
        if (Test-Path -LiteralPath (Join-Path $dir 'status.yaml')) { $out += $dir }
        $parent = Split-Path -Parent $dir
        if (-not $parent -or $parent -eq $dir) { break }
        $dir = $parent
    }
    return $out
}

# In one table, the row whose path is the longest prefix of the file's (relative to that table's folder).
function Get-TableRow([string]$root, [string]$full) {
    $rootFull = [System.IO.Path]::GetFullPath($root).TrimEnd('\')
    $rel = $full.Substring($rootFull.Length).TrimStart('\', '/').Replace('\', '/')
    $statusFile = Join-Path $root 'status.yaml'
    try { $lines = Get-Content -LiteralPath $statusFile -Encoding UTF8 -ErrorAction Stop }
    catch { Stop-Refused ('{0} cannot be read' -f $statusFile) }
    $best = $null
    foreach ($line in $lines) {
        if ($line -notmatch '^\s*([A-Za-z0-9_.-]+):\s*\{(.*)\}\s*$') { continue }
        $name = $Matches[1]
        $body = $Matches[2]
        if ($body -notmatch 'path:\s*([^,}\s]+)') { continue }
        $path = $Matches[1].Trim('"', "'").Replace('\', '/').TrimEnd('/')
        if (-not $path -or $path -eq '-') { continue }
        $state = if ($body -match 'edits:\s*([A-Za-z]+)') { $Matches[1].ToLower() } else { 'open' }
        $hit = $rel.Equals($path, [System.StringComparison]::OrdinalIgnoreCase) -or
            $rel.StartsWith($path + '/', [System.StringComparison]::OrdinalIgnoreCase)
        if ($hit -and (-not $best -or $path.Length -gt $best.Path.Length)) {
            $best = @{ Name = $name; Path = $path; State = $state; Root = $rootFull; Rel = $rel; Repo = (Split-Path -Leaf $rootFull) }
        }
    }
    return $best
}
function Get-Rank([string]$state) { switch ($state) { 'locked' { 3 } 'ask' { 2 } default { 1 } } }

# A text as lines: line endings normalised, one final empty line (the final newline) dropped.
function Get-Lines([string]$text) {
    $t = $text.Replace("`r`n", "`n").Replace("`r", "`n")
    if ($t.EndsWith("`n")) { $t = $t.Substring(0, $t.Length - 1) }
    if ($t -eq '') { return ,([string[]]@()) }
    return ,([string[]]$t.Split("`n"))
}

# The cells of a table row: `| a | b |` gives 2; a `\|` is not a separator.
function Get-CellCount([string]$line) {
    $t = $line.Trim()
    if (-not ($t.StartsWith('|') -and $t.EndsWith('|') -and $t.Length -ge 2)) { return -1 }
    $inner = $t.Substring(1, $t.Length - 2)
    return ([regex]::Split($inner, '(?<!\\)\|')).Count
}

function Test-Same([string[]]$a, [int]$aFrom, [string[]]$b, [int]$bFrom, [int]$n) {
    for ($i = 0; $i -lt $n; $i++) { if (-not $a[$aFrom + $i].Equals($b[$bFrom + $i])) { return $false } }
    return $true
}

# The file as it would be after the call, as lines; a string starting with '!' says why it cannot be.
function Get-After($call, [string]$before) {
    $in = $call.tool_input
    $tool = [string]$call.tool_name
    if ($tool -eq 'Write') {
        if ($null -eq $in.content) { return '!a Write with no content' }
        return [string]$in.content
    }
    if ($tool -ne 'Edit') { return ('!a {0} call is not judged under a lock, only an Edit or a Write' -f $tool) }
    if ($null -eq $in.old_string -or $null -eq $in.new_string) { return '!an Edit with no old or new text' }
    $b = $before.Replace("`r`n", "`n")
    $old = ([string]$in.old_string).Replace("`r`n", "`n")
    $new = ([string]$in.new_string).Replace("`r`n", "`n")
    if ($old -eq '') { return '!an Edit with an empty old text' }
    $count = 0
    $at = $b.IndexOf($old, [StringComparison]::Ordinal)
    $first = $at
    while ($at -ge 0) { $count++; $at = $b.IndexOf($old, $at + $old.Length, [StringComparison]::Ordinal) }
    if ($count -eq 0) { return '!the old text is not in the file' }
    $all = ($in.replace_all -eq $true)
    if ($count -gt 1 -and -not $all) { return '!the old text is in the file several times, without replace_all' }
    if ($all) { return $b.Replace($old, $new) }
    return $b.Substring(0, $first) + $new + $b.Substring($first + $old.Length)
}

# Under the run log's heading, its table: the header's index, its cell count, the last row's index.
function Test-RunLogRows([string[]]$bl, [string[]]$al) {
    $heads = @()
    for ($i = 0; $i -lt $bl.Count; $i++) { if ($bl[$i] -match '^#{1,6}\s.*Run log') { $heads += $i } }
    if ($heads.Count -ne 1) { return 'the page has no run log heading, or more than one' }
    $h = $heads[0]
    $top = -1
    for ($i = $h + 1; $i -lt $bl.Count; $i++) {
        if ($bl[$i] -match '^#{1,6}\s') { break }
        if ($bl[$i].TrimStart().StartsWith('|')) { $top = $i; break }
    }
    if ($top -lt 0 -or $top + 1 -ge $bl.Count -or $bl[$top + 1] -notmatch '^\s*\|(\s*:?-+:?\s*\|)+\s*$') { return 'no table under the run log heading' }
    $cells = Get-CellCount $bl[$top]
    $end = $top + 1
    while ($end + 1 -lt $bl.Count -and $bl[$end + 1].TrimStart().StartsWith('|')) { $end++ }
    $k = $al.Count - $bl.Count
    if ($k -lt 1) { return 'nothing added' }
    if (-not (Test-Same $al 0 $bl 0 ($end + 1))) { return 'a line before the end of the run log changed' }
    $rest = $bl.Count - $end - 1
    if (-not (Test-Same $al ($end + 1 + $k) $bl ($end + 1) $rest)) { return 'a line after the run log changed' }
    for ($i = $end + 1; $i -le $end + $k; $i++) {
        if ((Get-CellCount $al[$i]) -ne $cells) { return ('a new line is not a row of {0} cells' -f $cells) }
        if ($al[$i] -match '^\s*\|(\s*:?-+:?\s*\|)+\s*$') { return 'a new line is a separator, not a row' }
    }
    return 'ok'
}

# A section's body as blocks: a `- ` item with its indented lines, else one line alone.
function Get-Blocks([string[]]$lines, [int]$from, [int]$to) {
    $blocks = New-Object System.Collections.ArrayList
    for ($i = $from; $i -lt $to; $i++) {
        $l = $lines[$i]
        if ($l -match '^\s+\S' -and $blocks.Count -gt 0 -and $blocks[$blocks.Count - 1].item) {
            $last = $blocks[$blocks.Count - 1]
            $last.text = $last.text + "`n" + $l
            continue
        }
        [void]$blocks.Add([pscustomobject]@{ text = $l; item = ($l -match '^- \S') })
    }
    return ,$blocks
}

# New `- ` items in the "Wanted, not promised yet" section, a lone "none" replaceable.
function Test-WantedLines([string[]]$bl, [string[]]$al) {
    $heads = @()
    for ($i = 0; $i -lt $bl.Count; $i++) { if ($bl[$i] -match '^#{1,6}\s.*Wanted, not promised yet') { $heads += $i } }
    if ($heads.Count -ne 1) { return 'the contract has no "Wanted, not promised yet" heading, or more than one' }
    $s = $heads[0]
    $t = $bl.Count
    for ($i = $s + 1; $i -lt $bl.Count; $i++) { if ($bl[$i] -match '^#{1,6}\s') { $t = $i; break } }
    $rest = $bl.Count - $t
    if ($al.Count -lt $bl.Count) { return 'a line removed' }
    if (-not (Test-Same $al 0 $bl 0 ($s + 1))) { return 'a line before the section changed' }
    if (-not (Test-Same $al ($al.Count - $rest) $bl $t $rest)) { return 'a line after the section changed' }
    $old = Get-Blocks $bl ($s + 1) $t
    $new = Get-Blocks $al ($s + 1) ($al.Count - $rest)
    # A lone "none" (the section's only line with text) may give its place to the first wish.
    $texts = @($old | Where-Object { $_.text.Trim() -ne '' })
    $noneAt = -1
    if ($texts.Count -eq 1 -and $texts[0].text.Trim() -match '^(- )?none$') { $noneAt = $old.IndexOf($texts[0]) }
    $j = 0
    $added = 0
    $replaced = $false
    foreach ($blk in $new) {
        if ($j -lt $old.Count -and $blk.text.Equals($old[$j].text)) { $j++; continue }
        if ($j -eq $noneAt -and -not $replaced -and $blk.item) { $replaced = $true; $j++; $added++; continue }
        if (-not $blk.item) { return ('a line that is not a new `- ` item: ' + $blk.text.Split("`n")[0]) }
        $added++
    }
    if ($j -ne $old.Count) { return 'a line of the section changed or removed' }
    if ($added -lt 1) { return 'nothing added' }
    return 'ok'
}

# Under a locked row: why = 'ok' when the call only adds what a lock lets through, else why not.
function Test-LockedAddition($call, [string]$full, [string]$rel, $row) {
    $leaf = ''
    foreach ($name in 'VALIDATION.md', 'CONTRACT.md') {
        $own = if ($row.Path -eq '.') { $name } else { $row.Path + '/' + $name }
        if ($rel.Equals($own, [StringComparison]::OrdinalIgnoreCase)) { $leaf = $name }
    }
    if (-not $leaf) { return @{ leaf = ''; why = 'not the part''s VALIDATION.md or CONTRACT.md' } }
    try {
        if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { return @{ leaf = $leaf; why = 'the file cannot be read' } }
        $before = [IO.File]::ReadAllText($full, [Text.Encoding]::UTF8)
        $after = Get-After $call $before
        if ($after.StartsWith('!')) { return @{ leaf = $leaf; why = $after.Substring(1) } }
        $bl = Get-Lines $before
        $al = Get-Lines $after
        $why = if ($leaf -eq 'VALIDATION.md') { Test-RunLogRows $bl $al } else { Test-WantedLines $bl $al }
        # Only the word 'ok' lets it through: anything else, nothing included, is a refusal.
        if ($null -eq $why -or ($why -isnot [string]) -or $why -eq '') { $why = 'the hook could not judge it' }
        return @{ leaf = $leaf; why = $why }
    } catch {
        return @{ leaf = $leaf; why = ('the hook could not judge it: ' + (($_.Exception.Message -split "`r?`n")[0])) }
    }
}

function Write-Decision([string]$decision, [string]$reason) {
    $answer = @{
        hookSpecificOutput = @{
            hookEventName            = 'PreToolUse'
            permissionDecision       = $decision
            permissionDecisionReason = $reason
        }
    }
    Write-Output ($answer | ConvertTo-Json -Compress -Depth 4)
}

try {
    # Claude Code sends UTF-8: read so, whatever the console's code page, so a page's text compares as it is.
    $reader = New-Object System.IO.StreamReader([Console]::OpenStandardInput(), (New-Object System.Text.UTF8Encoding($false)))
    $raw = $reader.ReadToEnd()
    if (-not $raw -or -not $raw.Trim()) { Stop-Refused 'the hook got no input' }
    try { $call = $raw | ConvertFrom-Json } catch { Stop-Refused 'the hook input is not JSON' }

    $file = $null
    if ($call.tool_input) {
        if ($call.tool_input.file_path) { $file = [string]$call.tool_input.file_path }
        elseif ($call.tool_input.notebook_path) { $file = [string]$call.tool_input.notebook_path }
    }
    if (-not $file) { Stop-Refused 'an edit with no file path' }
    $session = if ($call.session_id) { [string]$call.session_id } else { 'nosession' }

    # The forms no row can be compared with.
    if ($file -match '(?i)::\$|:\$DATA' -or ($file.Length -gt 2 -and $file.IndexOf(':', 2) -ge 0)) { Stop-Refused ('{0}: a stream suffix' -f $file) }
    if ($file -match '(^|[\\/])[^\\/\s]{1,6}~\d') { Stop-Refused ('{0}: an 8.3 short name; give the long path' -f $file) }
    if ($file -match '^(\\\\|//)') { Stop-Refused ('{0}: a device or network path' -f $file) }
    if ($file -match '^/([A-Za-z])/') { $file = $Matches[1] + ':' + $file.Substring(2) }

    # Relative to the file's repository, forward slashes, case-insensitive; no repository = open.
    try { $full = [System.IO.Path]::GetFullPath($file) } catch { Stop-Refused ('{0}: a path the hook cannot resolve' -f $file) }
    $g = $GrantsRoot.TrimEnd('\')
    if ($full.Equals($g, [StringComparison]::OrdinalIgnoreCase) -or $full.StartsWith($g + '\', [StringComparison]::OrdinalIgnoreCase)) {
        if ($Grant) { exit 0 }
        Write-Decision 'deny' ('{0} is in the grant markers'' folder: only the hooks and grant.ps1 write there' -f $full)
        exit 0
    }
    # C-4.9: inside a repository's .git folder, or a worktree's .git file: git writes there, an agent never.
    foreach ($seg in $full.Split('\')) {
        if ($seg.TrimEnd('.', ' ').Equals('.git', [StringComparison]::OrdinalIgnoreCase)) {
            if ($Grant) { exit 0 }
            Write-Decision 'deny' ('{0} is inside a .git folder or is a worktree''s .git file: git writes there, an agent never does' -f $full)
            exit 0
        }
    }

    # C-4.4: every table above the file, each by its own rows; the strictest answer wins (locked over
    # ask over open), the nearest table on a tie, so a table written deeper only tightens.
    $found = @()
    foreach ($root in @(Find-Roots $full)) {
        $row = Get-TableRow $root $full
        if ($row) { $found += $row }
    }
    $best = $null
    foreach ($row in $found) { if (-not $best -or (Get-Rank $row.State) -gt (Get-Rank $best.State)) { $best = $row } }

    # C-4.8: a script the lists let run without asking is asked, whatever its row says; the lists
    # unreadable (or their reader missing): every script file is.
    $isScript = $false
    if (Get-Command Test-ListedScript -ErrorAction SilentlyContinue) {
        $listed = $null
        if ($HookRepo) { $listed = Get-ListedScripts $HookRepo @($full, [string]$call.cwd) }
        $isScript = Test-ListedScript $full $listed
    } else {
        $isScript = $full -match '(?i)\.(ps1|psm1|psd1|sh|bash|bat|cmd|py|js|mjs|cjs|rb|pl)$'
    }
    $sessionSafe = $session -replace '[^A-Za-z0-9._-]', '_'
    $scriptGrants = Join-Path $GrantsRoot 'scripts'
    $scriptMarker = Join-Path $scriptGrants $sessionSafe

    if (-not $best -and -not $isScript) { exit 0 }
    $rel = $full
    $grants = $marker = $addGrants = $addMarker = ''
    if ($best) {
        $rel = $best.Rel
        $grants = Join-Path $GrantsRoot ("edits-{0}" -f $best.Repo)
        $marker = Join-Path $grants ("{0}-{1}" -f $sessionSafe, $best.Name)
        # The additions a lock lets through keep a marker of their own: a yes to them never covers an ask row.
        $addGrants = Join-Path $GrantsRoot ("additions-{0}" -f $best.Repo)
        $addMarker = Join-Path $addGrants ("{0}-{1}" -f $sessionSafe, $best.Name)
    }
    $state = if ($best) { $best.State } else { 'open' }

    if ($Grant) {
        if ($state -eq 'ask') {
            New-Item -ItemType Directory -Force -Path $grants | Out-Null
            Set-Content -LiteralPath $marker -Value (Get-Date -Format o)
        }
        # One yes covers every listed script for the session; under a lock none ran.
        if ($isScript -and $state -ne 'locked') {
            New-Item -ItemType Directory -Force -Path $scriptGrants | Out-Null
            Set-Content -LiteralPath $scriptMarker -Value (Get-Date -Format o)
        }
        # Under a lock only an addition the guard let through before the call can have run.
        $ownDoc = $false
        if ($best) {
            foreach ($name in 'VALIDATION.md', 'CONTRACT.md') {
                $own = if ($best.Path -eq '.') { $name } else { $best.Path + '/' + $name }
                if ($rel.Equals($own, [StringComparison]::OrdinalIgnoreCase)) { $ownDoc = $true }
            }
        }
        if ($state -eq 'locked' -and -not $isScript -and $ownDoc -and [string]$call.tool_name -in @('Edit', 'Write')) {
            New-Item -ItemType Directory -Force -Path $addGrants | Out-Null
            Set-Content -LiteralPath $addMarker -Value (Get-Date -Format o)
        }
        exit 0
    }
    switch ($state) {
        'locked' {
            $refusal = "{0} is locked by status.yaml (row {1}); only the owner unlocks it" -f $rel, $best.Name
            if ($isScript) {
                Write-Decision 'deny' ("{0}. It is also a script the lists let run without asking: refused" -f $refusal)
                break
            }
            $judged = Test-LockedAddition $call $full $rel $best
            # Several tables locking the file: the addition must pass under each locked row found.
            foreach ($other in @($found | Where-Object { $_.State -eq 'locked' -and -not [object]::ReferenceEquals($_, $best) })) {
                if (-not ($judged.leaf -and $judged.why -ceq 'ok')) { break }
                $j2 = Test-LockedAddition $call $full $other.Rel $other
                if (-not ($j2.leaf -and $j2.why -ceq 'ok')) {
                    $judged = $j2
                    $refusal = "{0} is locked by status.yaml (row {1}); only the owner unlocks it" -f $other.Rel, $other.Name
                }
            }
            if ($judged.leaf -and $judged.why -ceq 'ok') {
                if (-not (Test-Path -LiteralPath $addMarker)) {
                    $what = if ($judged.leaf -eq 'VALIDATION.md') { 'a new row at the end of the run log' } else { 'a new wish under "Wanted, not promised yet"' }
                    Write-Decision 'ask' ("{0}: status.yaml row {1} is locked; {2} passes as an ask edit; a yes covers such additions to this row for the session" -f $rel, $best.Name, $what)
                }
            }
            elseif ($judged.leaf -eq 'VALIDATION.md') { Write-Decision 'deny' ("{0}. locked: only a new row at the end of the run log passes here ({1})" -f $refusal, $judged.why) }
            elseif ($judged.leaf -eq 'CONTRACT.md') { Write-Decision 'deny' ("{0}. locked: only a new line under ""Wanted, not promised yet"" passes here ({1})" -f $refusal, $judged.why) }
            else { Write-Decision 'deny' ("{0}. locked: only a new run-log row of its VALIDATION.md or a new wish in its CONTRACT.md passes" -f $refusal) }
        }
        default {
            $why = @()
            if ($state -eq 'ask' -and -not (Test-Path -LiteralPath $marker)) {
                $why += ("{0}: status.yaml row {1} says ask; a yes covers this row for the session" -f $rel, $best.Name)
            }
            if ($isScript -and -not (Test-Path -LiteralPath $scriptMarker)) {
                $why += ("{0}: a script the lists let run without asking (an allow rule names it); a yes covers such scripts for the session" -f $full)
            }
            if ($why.Count) { Write-Decision 'ask' ($why -join '; ') }
        }
    }
    exit 0
} catch {
    Stop-Refused ('the hook failed: ' + (($_.Exception.Message -split "`r?`n")[0]) + (' ({0} line {1})' -f (Split-Path -Leaf ([string]$_.InvocationInfo.ScriptName)), $_.InvocationInfo.ScriptLineNumber))
}
