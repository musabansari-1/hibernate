param(
  [Parameter(Mandatory=$true)][string]$BridgePath,
  [Parameter(Mandatory=$true)][string]$SessionId,
  [Parameter(Mandatory=$true)][string]$ProjectDirectory,
  [Parameter(Mandatory=$true)][string]$DataDirectory
)
$ErrorActionPreference='Stop'
Set-Location -LiteralPath $ProjectDirectory
$env:HIBERNATE_DATA_DIR=$DataDirectory
$global:HibernateBridgePath=$BridgePath
$global:HibernateTerminalSession=$SessionId
function global:prompt {
  # Report only the current directory. No command text, environment or history is collected.
  if($executionContext.SessionState.Path.CurrentLocation.Provider.Name -eq 'FileSystem') {
    & $global:HibernateBridgePath --terminal-report $global:HibernateTerminalSession $PID $executionContext.SessionState.Path.CurrentLocation.ProviderPath | Out-Null
  }
  "PS $($executionContext.SessionState.Path.CurrentLocation)> "
}
Write-Host 'ContextSleep terminal: working-directory reporting is active. No commands are replayed.'
