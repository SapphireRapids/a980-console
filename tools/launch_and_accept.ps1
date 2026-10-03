param([string]$Exe, [string]$NodeScript = "tools\accept_test.mjs")
$ErrorActionPreference = "Continue"
Get-Process a980-console -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
Start-Process $Exe
Start-Sleep -Seconds 6
$p = Get-Process a980-console -ErrorAction SilentlyContinue
if ($p) { "shell-alive pid=" + ($p.Id -join ",") } else { "shell-DEAD (webview failed?)"; exit 1 }
$c = Get-NetTCPConnection -LocalPort 9222 -State Listen -ErrorAction SilentlyContinue
if (-not $c) { "port 9222 not listening"; exit 1 }
"port 9222 listening ok"
$log = Join-Path $env:TEMP "a980_accept_out.txt"
$rc = 0
try { & node $NodeScript 2>&1 | Tee-Object -FilePath $log; "accept rc=" + $LASTEXITCODE }
catch { "accept threw: " + $_.Exception.Message; exit 1 }
"--- log tail ---"
Get-Content $log -Tail 25
