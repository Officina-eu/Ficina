# run-loop.ps1 — the Business-track build loop engine (docs/autonomy/LOOP.md).
#
# Runs Claude Code headless, one queue item per invocation, forever — until
# QUEUE.md is complete (STATE.md gains "LOOP COMPLETE") or the loop halts
# ("LOOP HALT"). Safe to stop any time with Ctrl+C: every finished item was
# already committed and pushed by the iteration that built it.
#
# Usage (PowerShell, on the build PC):
#   powershell -ExecutionPolicy Bypass -File scripts\run-loop.ps1 -RepoPath "C:\dev\Ficina"
param(
  [string]$RepoPath = "C:\dev\Ficina",
  [string]$Track = "business",       # business | sites (LOOP.md Tracks table)
  [int]$MaxIterations = 500,         # hard backstop against runaway loops
  [int]$IdleKillMin = 20,            # kill a worker SILENT this long (true hang)
  [int]$IterationCeilingMin = 240    # absolute per-iteration backstop
)
$ErrorActionPreference = "Continue"
Set-Location $RepoPath

$StateFile = if ($Track -eq "sites") { "docs/autonomy/sites/STATE.md" } else { "docs/autonomy/STATE.md" }
$prompt = "Read docs/autonomy/LOOP.md and execute exactly ONE iteration of the loop for track '$Track', then exit."

# Resolve the claude CLI: PATH first, else the newest VSCode-extension binary.
$claude = (Get-Command claude -ErrorAction SilentlyContinue).Source
if (-not $claude) {
  $claude = Get-ChildItem "$env:USERPROFILE\.vscode\extensions\anthropic.claude-code-*\resources\native-binary\claude.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $claude) { Write-Host "[loop] claude CLI not found - npm install -g @anthropic-ai/claude-code"; exit 1 }
Write-Host "[loop] using claude at $claude"

for ($i = 1; $i -le $MaxIterations; $i++) {
  Write-Host ("=" * 60)
  Write-Host "[loop] iteration $i  $(Get-Date -Format 'yyyy-MM-dd HH:mm')"

  git pull --rebase origin main 2>&1 | Out-Null

  $state = Get-Content $StateFile -Raw -ErrorAction SilentlyContinue
  if ($state -match "LOOP COMPLETE") { Write-Host "[loop] queue complete - stopping."; break }
  if ($state -match "LOOP HALT")     { Write-Host "[loop] halted by the agent - fix the reason in STATE.md, remove the marker, restart."; break }

  # One iteration, with an IDLE-based hang guard: a truly hung worker goes
  # silent (its session transcript stops growing), while an honest long item
  # keeps writing constantly — a duration-only guard once executed 90 minutes
  # of honest work. So: kill after $IdleKillMin of transcript silence, with
  # $IterationCeilingMin as the absolute backstop. The killed item is simply
  # redone next iteration. --dangerously-skip-permissions is required for
  # unattended runs; the hard safety rails live in LOOP.md + repo deny rules.
  if ($claude -like "*.ps1") {
    $file = "powershell"
    $cliArgs = @("-ExecutionPolicy","Bypass","-File",$claude,"-p","`"$prompt`"","--dangerously-skip-permissions")
  } else {
    $file = $claude
    $cliArgs = @("-p","`"$prompt`"","--dangerously-skip-permissions")
  }
  # Transcript dir for this repo (path with [:\ ] as '-'), where activity shows.
  $projKey = ($RepoPath -replace "[:\\ ]", "-")
  $transcripts = Join-Path $env:USERPROFILE ".claude\projects\$projKey"
  $started = Get-Date

  $proc = Start-Process -FilePath $file -ArgumentList $cliArgs -NoNewWindow -PassThru
  $killed = $false
  while (-not $proc.WaitForExit(30 * 1000)) {
    $now = Get-Date
    $newest = Get-ChildItem "$transcripts\*.jsonl" -ErrorAction SilentlyContinue |
      Where-Object { $_.LastWriteTime -gt $started.AddSeconds(-60) } |
      Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $idleMin = if ($newest) { ($now - $newest.LastWriteTime).TotalMinutes }
               else { ($now - $started).TotalMinutes }
    $reason = $null
    if ($idleMin -ge $IdleKillMin) { $reason = "silent for $([int]$idleMin) min" }
    elseif (($now - $started).TotalMinutes -ge $IterationCeilingMin) { $reason = "hit the $IterationCeilingMin-min ceiling" }
    if ($reason) {
      Write-Host "[loop] killing the worker - $reason."
      taskkill /PID $proc.Id /T /F 2>$null | Out-Null
      # Drop half-done, uncommitted state so the next iteration starts clean
      # (local commits survive - only unpushed edits of the killed run are lost).
      git rebase --abort 2>$null | Out-Null
      git checkout -- . 2>$null | Out-Null
      $killed = $true
      break
    }
  }
  if ($killed) {
    $code = 124
  } else {
    # PS quirk: after a timed WaitForExit, ExitCode can read null until a
    # blocking WaitForExit() refreshes the handle; a null code then looked
    # like a failure and cost a 15-minute backoff per SUCCESSFUL item.
    $proc.WaitForExit()
    $code = if ($null -ne $proc.ExitCode) { $proc.ExitCode } else { 0 }
  }

  if ($code -eq 124) {
    Start-Sleep -Seconds 30           # the hang already wasted time - go again
  } elseif ($code -ne 0) {
    # Rate limit / transient failure: back off instead of spinning.
    Write-Host "[loop] iteration exited with code $code - waiting 15 minutes."
    Start-Sleep -Seconds 900
  } else {
    Start-Sleep -Seconds 10
  }
}
Write-Host "[loop] done after $i iterations."
