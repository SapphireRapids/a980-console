$env:PATH = "C:\Users\eijdd\.cargo\bin;C:\Program Files (x86)\NSIS\Bin;" + $env:PATH
Set-Location "C:\Users\eijdd\Documents\zcodework\a980-app"
npx tauri build 2>&1 | Select-Object -Last 50
