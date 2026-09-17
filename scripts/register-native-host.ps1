param(
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-p]{32}$')][string]$ExtensionId,
  [ValidateSet('Chrome','Chromium')][string]$Browser='Chrome',
  [string]$HostPath='',
  [switch]$Unregister
)
$ErrorActionPreference='Stop'
if(-not $HostPath){$HostPath=Join-Path $PSScriptRoot '..\native\target\debug\hibernate-host.exe'}
$resolvedHost=(Resolve-Path -LiteralPath $HostPath).Path
$manifestPath=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.validation\native-host-$Browser.json"))
$vendor=if($Browser -eq 'Chrome'){'Google\Chrome'}else{'Chromium'}
$key="HKCU:\Software\$vendor\NativeMessagingHosts\local.projecthibernate.bridge"
if($Unregister){
  if((Test-Path -LiteralPath $key) -and (Get-Item -LiteralPath $key).GetValue('') -eq $manifestPath){Remove-Item -LiteralPath $key; Write-Output 'Removed this workspace native host registration.'}
  else {throw 'Registration does not belong to this workspace; no registry key was removed.'}
  exit
}
if((Test-Path -LiteralPath $key) -and (Get-Item -LiteralPath $key).GetValue('') -ne $manifestPath){throw 'An existing host registration belongs to another location. No changes made.'}
New-Item -ItemType Directory -Path (Split-Path -Parent $manifestPath) -Force | Out-Null
$manifest=@{name='local.projecthibernate.bridge';description='ContextSleep local snapshot bridge';path=$resolvedHost;type='stdio';allowed_origins=@("chrome-extension://$ExtensionId/")}
[IO.File]::WriteAllText($manifestPath,($manifest|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
New-Item -Path $key -Force | Out-Null
Set-Item -LiteralPath $key -Value $manifestPath
Write-Output "Registered $Browser native host for extension $ExtensionId at $resolvedHost"
