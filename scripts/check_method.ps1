<#
.SYNOPSIS
    Are this project's copies of the method the canonical ones? One row per file; a report.
.DESCRIPTION
    The canonical copies live in the Lorisse repository's method/ folder ($env:LORISSE_REPO,
    else C:\software\lorisse). Each is compared with the project's copy, line endings
    normalised: same, differs (+N -M lines: N only here, M only in the method), missing here,
    missing in the method. For status.yaml only the comment lines before `parts:` count.
    A report, never a failure: exit 0 always. Called by scripts/verify.ps1 with -Summary.
.EXAMPLE
    .\scripts\check_method.ps1 -Summary
#>
[CmdletBinding()]
param([switch] $Summary)

# The repository holding this script (a project's scripts/ or the method's method/checks/).
$root = Split-Path -Parent $PSScriptRoot
$top = & git -C $PSScriptRoot rev-parse --show-toplevel
if ($LASTEXITCODE -eq 0 -and $top) { $root = ([string] $top).Replace('/', '\') }
$lorisse = if ($env:LORISSE_REPO) { $env:LORISSE_REPO } else { 'C:\software\lorisse' }
$method = Join-Path $lorisse 'method'
if (-not (Test-Path -LiteralPath $method -PathType Container)) {
    Write-Host ("method: no canonical folder at {0}, nothing compared" -f $method)
    exit 0
}

# The method's revision: the `revision:` line of method/README.md.
$revision = 'r?'
$readme = Join-Path $method 'README.md'
if (Test-Path -LiteralPath $readme) {
    $found = [regex]::Match([System.IO.File]::ReadAllText($readme), '(?m)^revision:\s*(r\d+)')
    if ($found.Success) { $revision = $found.Groups[1].Value }
}

# Canonical file under method/ -> the project's copy.
$pairs = [ordered]@{
    'contract_spec.md'      = 'docs/contract_spec.md'
    'checks/check_reviews.ps1'  = 'scripts/check_reviews.ps1'
    'checks/check_comments.ps1' = 'scripts/check_comments.ps1'
    'hooks/guard_edits.ps1'     = 'scripts/hooks/guard_edits.ps1'
    'checks/check_method.ps1'   = 'scripts/check_method.ps1'
    'status_header.yaml'    = 'status.yaml'
}

# A file's lines, LF only; for a status table, its comment lines at column 0 (the head above the
# rows and the explanation under them; a group's comment is indented and belongs to the rows).
function Get-Lines([string] $path, [bool] $header) {
    $text = [System.IO.File]::ReadAllText($path).Replace("`r`n", "`n").Replace("`r", "`n")
    $lines = $text -split "`n"
    if ($header) {
        $lines = @($lines | Where-Object { $_ -match '^#' })
    }
    , @($lines)
}

# One row's verdict for a canonical file and the project's copy.
function Get-Verdict([string] $canon, [string] $copy, [bool] $header) {
    if (-not (Test-Path -LiteralPath $canon)) { return 'missing in the method' }
    if (-not (Test-Path -LiteralPath $copy)) { return 'missing here' }
    $a = Get-Lines $canon $header
    $b = Get-Lines $copy $header
    if (($a -join "`n") -ceq ($b -join "`n")) { return 'same' }
    if ($a.Count -eq 0 -or $b.Count -eq 0) { return ('differs (+{0} -{1} lines)' -f $b.Count, $a.Count) }
    $diff = @(Compare-Object -ReferenceObject $a -DifferenceObject $b -CaseSensitive)
    $plus = @($diff | Where-Object { $_.SideIndicator -eq '=>' }).Count
    $minus = @($diff | Where-Object { $_.SideIndicator -eq '<=' }).Count
    return ('differs (+{0} -{1} lines)' -f $plus, $minus)
}

$rows = foreach ($key in $pairs.Keys) {
    $rel = $pairs[$key]
    $header = $key -eq 'status_header.yaml'
    $verdict = Get-Verdict (Join-Path $method $key) (Join-Path $root $rel) $header
    [pscustomobject]@{ File = $rel; Verdict = $verdict }
}

if (-not $Summary) {
    Write-Host ('method {0}' -f $revision)
    $rows | ForEach-Object { Write-Host ('{0,-34} {1}' -f $_.File, $_.Verdict) }
    Write-Host ''
}
$kinds = 'same', 'differs', 'missing here', 'missing in the method'
$counts = foreach ($k in $kinds) { '{0} {1}' -f @($rows | Where-Object { $_.Verdict.StartsWith($k) }).Count, $k }
Write-Host ('method {0}: {1} file(s) against {2}: {3}' -f $revision, @($rows).Count, $method, ($counts -join ', '))
exit 0
