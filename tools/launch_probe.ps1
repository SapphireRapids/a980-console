param([string]$Exe, [switch]$KillFirst)
$ErrorActionPreference = "Continue"
if ($KillFirst) {
  Get-Process msedgewebview2,a980-console -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 2
  "killed leftovers; remaining msedgewebview2=" + (@(Get-Process msedgewebview2 -ErrorAction SilentlyContinue).Count)
}
Start-Process $Exe
Start-Sleep -Seconds 6
$p = Get-Process a980-console -ErrorAction SilentlyContinue
if ($p) { "proc-alive pid=" + ($p.Id -join ",") } else { "proc-NOT-alive" }
$w = Get-Process msedgewebview2 -ErrorAction SilentlyContinue
"webview-children=" + (@($w).Count)
