# Skills Hub Custom Skill Runtime V1 producer example.
# Disabled by default. This file is sample code, not a Hook installer.
[CmdletBinding()]
param(
    [switch]$EnableExample,
    [ValidateSet("codex", "claude", "opencode", "pi")]
    [string]$Agent = "codex",
    [ValidateSet("session.started", "skill.called", "skill.loaded", "context.compacted", "session.ended")]
    [string]$Event = "session.started",
    [ValidatePattern("^[A-Za-z0-9._:-]{1,128}$")]
    [string]$EventId = "example-event",
    [ValidatePattern("^[A-Za-z0-9._:-]{1,128}$")]
    [string]$SessionId = "example-session",
    [AllowEmptyString()]
    [string]$Skill = "",
    [string]$ObservedAt = "2026-08-31T00:00:00Z"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $EnableExample) {
    Write-Output "Disabled: pass -EnableExample only for a synthetic local event."
    return
}

if ($Event -in @("session.started", "context.compacted", "session.ended") -and $Skill -ne "") {
    throw "Session-wide events must not carry a Skill."
}
if ($Event -in @("skill.called", "skill.loaded") -and $Skill -notmatch "^[a-z0-9][a-z0-9._-]{0,63}$") {
    throw "Skill must be a lowercase safe Skill ID."
}
if ($ObservedAt -notmatch "^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$") {
    throw "ObservedAt must be strict RFC 3339 UTC."
}
try {
    $observed = [DateTimeOffset]::Parse($ObservedAt, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::AssumeUniversal)
} catch {
    throw "ObservedAt is not a valid timestamp."
}
if ($observed.Offset -ne [TimeSpan]::Zero) { throw "ObservedAt must use UTC Z." }
$runtimeNow = [DateTimeOffset]::UtcNow
if ($observed -gt $runtimeNow.AddMinutes(5)) { throw "ObservedAt is too far in the future." }
if ($observed -lt $runtimeNow.AddDays(-30)) { throw "ObservedAt is too old." }

$localApplicationData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
if ([string]::IsNullOrWhiteSpace($localApplicationData)) { throw "Local application data is unavailable." }
$inbox = Join-Path (Join-Path $localApplicationData "com.dandan812.skillshubcustom") "runtime-hooks\skill-runtime-v1.jsonl"
$parent = Split-Path -LiteralPath $inbox -Parent
$parentInfo = Get-Item -LiteralPath $parent -ErrorAction Stop
if (-not $parentInfo.PSIsContainer -or ($parentInfo.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw "The fixed inbox parent is unavailable or a reparse point."
}
if (Test-Path -LiteralPath $inbox -PathType Leaf) {
    $inboxInfo = Get-Item -LiteralPath $inbox -ErrorAction Stop
    if ($inboxInfo.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "The inbox is a reparse point." }
} elseif (Test-Path -LiteralPath $inbox) {
    throw "The inbox path is not a regular file."
}

$fields = [ordered]@{
    schemaVersion = 1
    eventId = $EventId
    agent = $Agent
    sessionId = $SessionId
    event = $Event
}
if ($Event -in @("skill.called", "skill.loaded")) { $fields.skill = $Skill }
$fields.observedAt = $ObservedAt
$line = $fields | ConvertTo-Json -Compress -Depth 2
if ($line -match "[\r\n]" -or $line.Length -gt 4096) { throw "Generated event is not one bounded JSONL line." }

[IO.File]::AppendAllText($inbox, $line + [Environment]::NewLine, (New-Object Text.UTF8Encoding($false)))
