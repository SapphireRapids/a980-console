param()
Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
  Where-Object { $_.CommandLine -like "*A980*" -or $_.CommandLine -like "*a980*" } |
  Select-Object ProcessId, Name, CreationDate, @{n = "path"; e = { ($_.CommandLine -split " ")[0] -replace '"', "" } } |
  Sort-Object CreationDate |
  Format-Table -AutoSize | Out-String -Width 200
