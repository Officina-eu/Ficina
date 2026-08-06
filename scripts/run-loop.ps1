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
  [string]$Track = "business",   # business | sites (LOOP.md Tracks table)
  [int]$MaxIterations = 500      # hard backstop against runaway loops
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

  # One iteration. --dangerously-skip-permissions is required for unattended
  # runs; the hard safety rails live in LOOP.md and the repo's deny rules.
  & $claude -p $prompt --dangerously-skip-permissions
  $code = $LASTEXITCODE

  if ($code -ne 0) {
    # Rate limit / transient failure: back off instead of spinning.
    Write-Host "[loop] iteration exited with code $code - waiting 15 minutes."
    Start-Sleep -Seconds 900
  } else {
    Start-Sleep -Seconds 10
  }
}
Write-Host "[loop] done after $i iterations."
