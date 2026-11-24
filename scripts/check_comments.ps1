<#
.SYNOPSIS
    Enforce the short-comment rule (docs/coding_conventions.md, "Comments and documentation")
    on the swept paths: no comment block longer than -MaxBlock consecutive lines, file header
    at most -MaxHeader lines.

.DESCRIPTION
    Scans the code files under the given paths, whatever the stack: .rs .c .h .cpp .hpp .js .ts
    .vue .py. A "comment line" starts with //, ///, //!, /*, /**, * or <!-- (and every line of an
    HTML comment until -->); in a .py file, with #. Section banners (// ====, // ----, # ====)
    and fenced code inside doc comments (``` ... ```) are not counted.
    The FIRST comment block of a file is its header and gets the -MaxHeader budget.

    Swept: every part of the table (each row's `path` in status.yaml; a target/, node_modules/,
    dist/ or parked/ folder skipped), plus the paths listed one per line in
    scripts/comment_paths.txt when the project has one (# starts a comment there).
    Called by scripts/verify.ps1.

.PARAMETER Paths
    Files or directories to check. Default: the table's paths and scripts/comment_paths.txt.

.EXAMPLE
    .\scripts\check_comments.ps1
    .\scripts\check_comments.ps1 -Paths <part path> -MaxBlock 6
#>
[CmdletBinding()]
param(
    [string[]]$Paths,
    [int]$MaxBlock = 6,
    [int]$MaxHeader = 10
)

$ErrorActionPreference = 'Stop'
# The repository holding this script (a project's scripts/ or the method's method/checks/).
$root = Split-Path -Parent $PSScriptRoot
$top = & git -C $PSScriptRoot rev-parse --show-toplevel
if ($LASTEXITCODE -eq 0 -and $top) { $root = ([string] $top).Replace('/', '\') }

# Every part of the table is under the rule; a project adds its other swept paths in comment_paths.txt.
$Swept = @()
$table = Join-Path $root 'status.yaml'
if (Test-Path -LiteralPath $table) {
    foreach ($line in Get-Content -LiteralPath $table -Encoding UTF8) {
        if ($line -notmatch '^\s*[A-Za-z0-9_.-]+:\s*\{(.*)\}\s*$') { continue }
        if ($Matches[1] -notmatch 'path:\s*([^,}\s]+)') { continue }
        $p = $Matches[1].Trim('"', "'").Replace('\', '/').TrimEnd('/')
        if ($p -and $p -ne '-' -and $Swept -notcontains $p) { $Swept += $p }
    }
}
$extra = Join-Path $PSScriptRoot 'comment_paths.txt'
if (Test-Path -LiteralPath $extra) {
    $Swept += @(Get-Content -LiteralPath $extra -Encoding UTF8 |
        ForEach-Object { ($_ -replace '#.*$', '').Trim() } | Where-Object { $_ })
}
if (-not $Paths) { $Paths = $Swept }
# `powershell -File ... -Paths a,b,c` hands the list over as ONE string: split it here.
$Paths = @($Paths | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })

$kinds = '.rs', '.c', '.h', '.cpp', '.hpp', '.js', '.ts', '.vue', '.py'
$slashLine = '^\s*(///?|//!|/\*\*?|\*|<!--)'
$slashBanner = '^\s*//\s*[=\-]{4,}\s*$'
$hashLine = '^\s*#'
$hashBanner = '^\s*#\s*[=\-]{4,}\s*$'

function Get-Violations([string]$file) {
    $commentLine = $slashLine; $banner = $slashBanner
    if ([IO.Path]::GetExtension($file) -ieq '.py') { $commentLine = $hashLine; $banner = $hashBanner }
    $lines = Get-Content -LiteralPath $file -Encoding UTF8
    $out = @()
    $run = 0; $runStart = 0; $inFence = $false; $inHtml = $false
    $n = 0
    foreach ($line in $lines) {
        $n++
        $isComment = $false
        if ($inHtml) {
            $isComment = $true
            if ($line -match '-->') { $inHtml = $false }
        } elseif ($line -match $banner) {
            # A banner separates blocks without counting.
            if ($run -gt 0) { $out += ,@($runStart, $run) }
            $run = 0
            continue
        } elseif ($line -match $commentLine) {
            $isComment = $true
            if ($line -match '<!--' -and $line -notmatch '-->') { $inHtml = $true }
            if ($line -match '```') { $inFence = -not $inFence; continue }
            if ($inFence) { continue }
        } elseif ($line.Trim() -eq '') {
            # Blank lines inside a block are neutral.
            continue
        }
        if ($isComment) {
            if ($run -eq 0) { $runStart = $n }
            $run++
        } else {
            if ($run -gt 0) { $out += ,@($runStart, $run) }
            $run = 0
        }
    }
    if ($run -gt 0) { $out += ,@($runStart, $run) }

    $violations = @()
    $first = $true
    foreach ($block in $out) {
        $start, $len = $block
        $limit = if ($first -and $start -eq 1) { $MaxHeader } else { $MaxBlock }
        $first = $false
        if ($len -gt $limit) {
            $violations += "{0}:{1}: comment block of {2} lines (max {3})" -f $file, $start, $len, $limit
        }
    }
    return $violations
}

$files = foreach ($p in $Paths) {
    $full = if ([System.IO.Path]::IsPathRooted($p)) { $p } else { Join-Path $root $p }
    if (Test-Path -LiteralPath $full -PathType Leaf) { $full }
    elseif (Test-Path -LiteralPath $full -PathType Container) {
        # Filtered by extension here: `-Include` is ignored with `-LiteralPath` on PowerShell 5.1.
        Get-ChildItem -LiteralPath $full -Recurse -File |
            Where-Object { $kinds -contains $_.Extension.ToLowerInvariant() } |
            Where-Object { $_.FullName -notmatch '[\\/](target|node_modules|dist|parked)[\\/]' } |
            ForEach-Object { $_.FullName }
    }
}

if (@($files).Count -eq 0) {
    Write-Host ("comment rule: no code file ({0}) found under [{1}] - check the paths" -f ($kinds -join ' '), ($Paths -join ', '))
    exit 1
}

$all = @()
foreach ($f in $files) { $all += Get-Violations $f }

if ($all.Count -gt 0) {
    $all | ForEach-Object { Write-Host $_ }
    Write-Host ("`n{0} comment block(s) over the limit in {1} file(s) checked" -f $all.Count, @($files).Count)
    exit 1
}
Write-Host ("comment rule: {0} file(s) checked, all blocks within limits" -f @($files).Count)
exit 0
