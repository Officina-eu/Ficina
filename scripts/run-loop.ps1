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
  [int]$IterationTimeoutMin = 90     # a hung worker is killed after this long
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

  # One iteration, with a hang guard: a stalled API stream once froze the loop
  # for a whole night, so the worker runs as a child process that gets killed
  # if it exceeds the timeout — the item is simply redone next iteration.
  # --dangerously-skip-permissions is required for unattended runs; the hard
  # safety rails live in LOOP.md and the repo's deny rules.
  if ($claude -like "*.ps1") {
    $file = "powershell"
    $cliArgs = @("-ExecutionPolicy","Bypass","-File",$claude,"-p","`"$prompt`"","--dangerously-skip-permissions")
  } else {
    $file = $claude
    $cliArgs = @("-p","`"$prompt`"","--dangerously-skip-permissions")
  }
  $proc = Start-Process -FilePath $file -ArgumentList $cliArgs -NoNewWindow -PassThru
  if (-not $proc.WaitForExit($IterationTimeoutMin * 60 * 1000)) {
    Write-Host "[loop] iteration exceeded $IterationTimeoutMin min - killing the hung worker."
    taskkill /PID $proc.Id /T /F 2>$null | Out-Null
    # Drop any half-done, uncommitted state so the next iteration starts clean
    # (local commits survive — only unpushed edits of the killed run are lost).
    git rebase --abort 2>$null | Out-Null
    git checkout -- . 2>$null | Out-Null
    $code = 124
  } else {
    $code = $proc.ExitCode
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
