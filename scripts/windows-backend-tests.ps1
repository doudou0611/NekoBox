param(
  [Parameter(Mandatory = $true)][string]$TestExecutable,
  [Parameter(Mandatory = $true)][string]$ReportDirectory
)
$ErrorActionPreference = 'Stop'
$taskSource = (Resolve-Path -LiteralPath $TestExecutable).Path
$taskReport = New-Item -ItemType Directory -Path $ReportDirectory -Force
$taskRoot = Join-Path $env:TEMP ('gm-acceptance-' + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $taskRoot | Out-Null
$taskExe = Join-Path $taskRoot 'backend-tests.exe'
Copy-Item -LiteralPath $taskSource -Destination $taskExe
$taskOut = Join-Path $taskReport.FullName 'stdout.txt'
$taskErr = Join-Path $taskReport.FullName 'stderr.txt'
$taskProc = Start-Process -FilePath $taskExe -ArgumentList @('--test-threads=1') -PassThru -RedirectStandardOutput $taskOut -RedirectStandardError $taskErr
$taskHandle = $taskProc.Handle
$taskCompleted = $taskProc.WaitForExit(120000)
if (!$taskCompleted) { Stop-Process -Id $taskProc.Id }
$taskProc.Refresh()
$taskExitCode = $(if ($taskCompleted) { $taskProc.ExitCode } else { $null })
$taskOs = Get-CimInstance Win32_OperatingSystem
$taskResult = [ordered]@{
  platform = $taskOs.Caption
  build = $taskOs.Version
  architecture = $taskOs.OSArchitecture
  binary_sha256 = (Get-FileHash -LiteralPath $taskSource -Algorithm SHA256).Hash
  completed = $taskCompleted
  exit_code = $taskExitCode
  gui_tested = $false
}
$taskResult | ConvertTo-Json | Out-File -LiteralPath (Join-Path $taskReport.FullName 'result.json') -Encoding utf8
Get-Content -LiteralPath $taskOut
if (!$taskCompleted -or $null -eq $taskExitCode -or $taskExitCode -ne 0) { exit 1 }
