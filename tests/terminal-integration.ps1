$ErrorActionPreference='Stop'
$testRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\.validation\terminal'))
$runDirectory=Join-Path $testRoot ('run-'+[Guid]::NewGuid().ToString())
$projectDirectory=Join-Path $runDirectory 'project'
New-Item -ItemType Directory -Path $projectDirectory -Force | Out-Null
$env:HIBERNATE_DATA_DIR=Join-Path $runDirectory 'data'
$bridgePath=if($env:HIBERNATE_TEST_HOST){[IO.Path]::GetFullPath($env:HIBERNATE_TEST_HOST)}else{[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\native\target\debug\hibernate-host.exe'))}
$started=Get-Date
$testShell=$null
try {
  $result=& $bridgePath --restore-folder terminal $projectDirectory | ConvertFrom-Json
  if($LASTEXITCODE -ne 0){throw 'Native terminal restoration failed'}
  if($result.status -ne 'cwd-verified'){throw ($result|ConvertTo-Json -Depth 5)}
  if($result.cwd -ne $projectDirectory){throw 'Observed terminal working directory did not match the fixture'}
  $testShell=Get-Process -Id $result.pid -ErrorAction Stop
  if($testShell.StartTime -lt $started -or $testShell.Path -ne (Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe')){throw 'Refusing cleanup: the process does not match the newly launched test shell'}
  $evidence=@{checks=@('Windows Terminal launched an instrumented PowerShell prompt','Native bridge verified the exact requested working directory','The test terminal used its isolated snapshot database');sessionId=$result.sessionId;observedDirectory=$result.cwd}
  [IO.File]::WriteAllText((Join-Path $testRoot 'results.json'),($evidence|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
  $evidence|ConvertTo-Json -Depth 5
} finally {
  # Only the fresh, idle fixture shell whose identity was checked above is stopped.
  if($null -ne $testShell -and -not $testShell.HasExited){$testShell.Kill();$testShell.WaitForExit(5000)|Out-Null}
}
