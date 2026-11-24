<#
.SYNOPSIS
    The owner's feedback notes of one folder: one row per note, the unread ones, a summary, or one
    note marked read by Lorisse after she answered it.
.DESCRIPTION
    The stack's feedback standard (me/next/app_feedback_standard_2026-11-05.md): a key on every
    screen writes `feedback/<YYYYMMDD-HHMMSS>.png` and `.md` (`.2` for a second note the same
    second) under the app's data folder or the repository root. The `.md`: the title
    `# Feedback <date> <time> - <screen>`, the review line
    `> written by the owner <date> from <the app> - reviewed by Lorisse: <not yet | date>` (each dash a middle
    dot), his
    words, `## State` (`- sent: <time>` once sent), `## Last events`.
    Scanned: `<Root>/feedback/` and, when it exists, `<Root>/reports/_feedback/` (the bench tool's).
    One row per `.md`: the stamp, unread or read <date>, sent or unsent, the picture there or not,
    the screen, the first line of his words (sixty characters).
    -Mark rewrites the review line's field of that note only, nothing else of the file; the note's
    words, its picture and every other note stay as they are. Nothing is ever deleted.
    -Archive moves every note read (its review line carries a date) whose stamp's month is closed
    (before the current month) under `<folder>/done/<yyyy-mm>/`, its `.md` and its `.png` together,
    one line per note and a count; nothing else of a note changes. The listing reads the root only.
    A report: exit 0 always.
.PARAMETER Root
    The repository or data folder holding `feedback/`. Default: the current folder.
.PARAMETER Unread
    Only the notes whose review line says `reviewed by Lorisse: not yet`.
.PARAMETER Summary
    One line: `feedback: n notes, m unread, k sent`.
.PARAMETER Mark
    A note's stamp (`20261105-201503`, `20261105-201503.2`): its review line's field becomes -Date.
.PARAMETER Date
    The date written by -Mark, `YYYY-MM-DD` (the second word after -Mark). Default: today.
.PARAMETER Archive
    The notes read of a closed month moved under `done/<yyyy-mm>/`, with their pictures.
.EXAMPLE
    .\scripts\read_feedback.ps1 -Root <data folder> -Unread
    .\scripts\read_feedback.ps1 -Mark 20261105-201503 2026-11-05
    .\scripts\read_feedback.ps1 -Root <data folder> -Archive
#>
[CmdletBinding()]
param(
    [string] $Root = '.',
    [switch] $Unread,
    [switch] $Summary,
    [switch] $Archive,
    [string] $Mark,
    [Parameter(Position = 0)] [string] $Date
)

$utf8 = New-Object System.Text.UTF8Encoding($false)
$stampRx = '^\d{8}-\d{6}(\.\d{1,3})?$'
# The middle dot matched as \S and the ellipsis as a char code: PowerShell 5.1 reads a file without a BOM as ANSI.
$reviewRx = '^(> written by .* \S reviewed by Lorisse:\s*)(not yet|\d{4}-\d{2}-\d{2})\s*$'
$noWords = '(no words: the picture only)'

# The folders of notes under the root, those that exist.
function Get-Folders([string] $base) {
    foreach ($rel in 'feedback', 'reports\_feedback') {
        $dir = Join-Path $base $rel
        if (Test-Path -LiteralPath $dir -PathType Container) { $dir }
    }
}

# One note read: its stamp, screen, review field, sent, picture, first line of his words.
function Read-Note([IO.FileInfo] $file) {
    $stamp = $file.Name.Substring(0, $file.Name.Length - 3)
    $lines = [IO.File]::ReadAllText($file.FullName, $utf8) -split "\r\n|\n|\r"
    $note = [ordered]@{ Stamp = $stamp; Path = $file.FullName; Screen = '?'; Field = $null; Sent = $false
        Picture = (Test-Path -LiteralPath ([IO.Path]::ChangeExtension($file.FullName, 'png'))); Words = '(not in the feedback shape)' }
    if ($lines[0] -match '^# Feedback \S+ \S+ \S (.*)$') { $note.Screen = $Matches[1].Trim() }
    $review = -1; $state = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($review -lt 0 -and $lines[$i] -match $reviewRx) { $review = $i; $note.Field = $Matches[2] }
        elseif ($review -ge 0 -and $state -lt 0 -and $lines[$i] -eq '## State') { $state = $i }
        elseif ($state -ge 0 -and $lines[$i] -match '^## ') { break }
        elseif ($state -ge 0 -and $lines[$i] -match '^- sent: ') { $note.Sent = $true }
    }
    if ($review -ge 0 -and $state -gt $review) {
        $body = @()
        if ($state -gt $review + 1) { $body = @($lines[($review + 1)..($state - 1)] | Where-Object { $_.Trim() }) }
        $first = if ($body.Count) { $body[0].Trim() -replace '^\\#', '#' } else { '' }
        if (-not $first -or $first -eq $noWords) { $first = '(no words: the picture only)' }
        if ($first.Length -gt 60) { $first = $first.Substring(0, 59) + [char] 0x2026 }
        $note.Words = $first
    }
    [pscustomobject] $note
}

# Every note under the root, oldest first.
function Get-Notes([string] $base) {
    foreach ($dir in Get-Folders $base) {
        Get-ChildItem -LiteralPath $dir -File -Filter '*.md' |
            Where-Object { $_.Name.Substring(0, $_.Name.Length - 3) -match $stampRx } |
            ForEach-Object { Read-Note $_ }
    }
}

# The note's review field rewritten to the date; the file's bytes otherwise kept (its line breaks, its BOM).
function Set-Read([string] $base, [string] $stamp, [string] $day) {
    if ($stamp -notmatch $stampRx) { return "read_feedback: not a feedback stamp: $stamp (nothing written)" }
    if ($day -notmatch '^\d{4}-\d{2}-\d{2}$') { return "read_feedback: not a date: $day (nothing written)" }
    $path = $null
    foreach ($dir in Get-Folders $base) { $p = Join-Path $dir "$stamp.md"; if (-not $path -and (Test-Path -LiteralPath $p)) { $path = $p } }
    if (-not $path) { return "read_feedback: no note $stamp under $base (nothing written)" }
    $bytes = [IO.File]::ReadAllBytes($path)
    $bom = $bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF
    $text = [IO.File]::ReadAllText($path, $utf8)
    $m = [regex]::Match($text, '(?m)^(> written by .* \S reviewed by Lorisse:[ \t]*)(not yet|\d{4}-\d{2}-\d{2})(?=[ \t]*\r?$)')
    if (-not $m.Success) { return "read_feedback: $stamp has no review line (nothing written)" }
    $new = $text.Substring(0, $m.Groups[2].Index) + $day + $text.Substring($m.Groups[2].Index + $m.Groups[2].Length)
    [IO.File]::WriteAllText($path, $new, (New-Object System.Text.UTF8Encoding($bom)))
    return "read_feedback: $stamp marked read by Lorisse $day (was $($m.Groups[2].Value))"
}

# The notes read whose month is closed, moved with their picture under <folder>/done/<yyyy-mm>/.
function Move-Archived([string] $base) {
    $month = (Get-Date).ToString('yyyyMM')
    $moved = 0
    foreach ($dir in Get-Folders $base) {
        $leaf = (Split-Path -Leaf $dir)
        $notes = @(Get-ChildItem -LiteralPath $dir -File -Filter '*.md' |
            Where-Object { $_.Name.Substring(0, $_.Name.Length - 3) -match $stampRx } | ForEach-Object { Read-Note $_ })
        foreach ($n in $notes) {
            if ("$($n.Field)" -notmatch '^\d{4}-\d{2}-\d{2}$') { continue }
            if ([string]::CompareOrdinal($n.Stamp.Substring(0, 6), $month) -ge 0) { continue }
            $sub = '{0}-{1}' -f $n.Stamp.Substring(0, 4), $n.Stamp.Substring(4, 2)
            $to = Join-Path (Join-Path $dir 'done') $sub
            $md = Join-Path $to ([IO.Path]::GetFileName($n.Path))
            $png = [IO.Path]::ChangeExtension($n.Path, 'png')
            $pngTo = Join-Path $to ([IO.Path]::GetFileName($png))
            if ((Test-Path -LiteralPath $md) -or (Test-Path -LiteralPath $pngTo)) {
                Write-Output "read_feedback: $($n.Stamp) already under $leaf/done/$sub/ (nothing moved)"
                continue
            }
            New-Item -ItemType Directory -Force -Path $to | Out-Null
            Move-Item -LiteralPath $n.Path -Destination $md
            $what = 'its .md'
            if (Test-Path -LiteralPath $png) { Move-Item -LiteralPath $png -Destination $pngTo; $what = 'its .md and its .png' }
            Write-Output "read_feedback: $($n.Stamp) moved to $leaf/done/$sub/ ($what)"
            $moved++
        }
    }
    $word = if ($moved -eq 1) { 'note' } else { 'notes' }
    Write-Output "read_feedback: $moved $word archived"
}

$base = (Resolve-Path -LiteralPath $Root -ErrorAction SilentlyContinue).Path
if (-not $base) { Write-Output "read_feedback: no folder $Root"; Write-Output 'feedback: 0 notes, 0 unread, 0 sent'; exit 0 }

if ($Mark) {
    if (-not $Date) { $Date = (Get-Date).ToString('yyyy-MM-dd') }
    Write-Output (Set-Read -base $base -stamp $Mark -day $Date)
    exit 0
}
if ($Archive) {
    Move-Archived -base $base
    exit 0
}

$notes = @(Get-Notes $base | Sort-Object Stamp)
$unreadCount = @($notes | Where-Object { $_.Field -eq 'not yet' }).Count
$sentCount = @($notes | Where-Object { $_.Sent }).Count
if (-not $Summary) {
    if (-not @(Get-Folders $base).Count) { Write-Output "read_feedback: no feedback folder under $base" }
    $shown = if ($Unread) { @($notes | Where-Object { $_.Field -eq 'not yet' }) } else { $notes }
    foreach ($n in $shown) {
        $read = if ($n.Field -eq 'not yet') { 'unread' } elseif ($n.Field) { "read $($n.Field)" } else { 'no review line' }
        $sent = if ($n.Sent) { 'sent' } else { 'unsent' }
        $pic = if ($n.Picture) { 'picture' } else { 'no picture' }
        Write-Output ('{0,-18} {1,-15} {2,-6} {3,-10} {4}: {5}' -f $n.Stamp, $read, $sent, $pic, $n.Screen, $n.Words)
    }
}
Write-Output ('feedback: {0} notes, {1} unread, {2} sent' -f $notes.Count, $unreadCount, $sentCount)
exit 0
