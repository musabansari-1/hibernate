param(
  [switch]$InstallVSCode,
  [switch]$RegisterChrome,
  [switch]$UnregisterChrome,
  [ValidatePattern('^[a-p]{32}$')][string]$ChromeExtensionId='',
  [ValidateSet('Chrome','Chromium')][string]$Browser='Chrome',
  [switch]$Launch
)
$ErrorActionPreference='Stop'
$packageRoot=Split-Path -Parent $MyInvocation.MyCommand.Path
if((Split-Path -Leaf $packageRoot) -eq 'scripts'){$packageRoot=Split-Path -Parent $packageRoot}
$nativeHost=Join-Path $packageRoot 'hibernate-host.exe'
$desktop=Join-Path $packageRoot 'ContextSleep.exe'
$registration=Join-Path $packageRoot 'scripts\register-native-host.ps1'
$vsix=Get-ChildItem -LiteralPath $packageRoot -Filter 'contextsleep-*.vsix' -File | Select-Object -First 1
$stableRoot=Join-Path $env:LOCALAPPDATA 'ContextSleep\integrations'
$stableHost=Join-Path $stableRoot 'hibernate-host.exe'

if(-not ($InstallVSCode -or $RegisterChrome -or $UnregisterChrome -or $Launch)){
  Write-Host 'ContextSleep package setup helper'
  Write-Host ''
  Write-Host 'Nothing was changed. Choose explicit actions:'
  Write-Host '  .\SETUP.ps1 -InstallVSCode'
  Write-Host '  .\SETUP.ps1 -RegisterChrome -ChromeExtensionId YOUR_32_CHARACTER_ID'
  Write-Host '  .\SETUP.ps1 -InstallVSCode -RegisterChrome -ChromeExtensionId YOUR_32_CHARACTER_ID -Launch'
  Write-Host '  .\SETUP.ps1 -UnregisterChrome -ChromeExtensionId YOUR_32_CHARACTER_ID'
  Write-Host ''
  Write-Host 'Load the chrome-extension folder manually in chrome://extensions first, then copy its ID.'
  Write-Host 'VS Code setup now prepares a stable local bridge automatically. The hibernate.nativeHostPath setting remains available as an advanced override.'
  exit 0
}

if(-not (Test-Path -LiteralPath $nativeHost -PathType Leaf)){throw 'hibernate-host.exe is missing from this package.'}
if(-not (Test-Path -LiteralPath $desktop -PathType Leaf)){throw 'ContextSleep.exe is missing from this package.'}

if($InstallVSCode){
  if(-not $vsix){throw 'The packaged VS Code extension (.vsix) is missing.'}
  New-Item -ItemType Directory -Path $stableRoot -Force | Out-Null
  Copy-Item -LiteralPath $nativeHost -Destination $stableHost -Force
  $code=Get-Command code.cmd -ErrorAction SilentlyContinue
  if(-not $code){$code=Get-Command code -ErrorAction SilentlyContinue}
  if(-not $code){throw 'VS Code command-line tool `code` was not found on PATH. Install the VSIX manually from VS Code instead.'}
  & $code.Source --install-extension $vsix.FullName --force
  if($LASTEXITCODE -ne 0){throw "VS Code extension installation failed with exit code $LASTEXITCODE."}
  Write-Host "Installed VS Code extension: $($vsix.Name)"
  Write-Host "Prepared stable ContextSleep bridge: $stableHost"
  Write-Host 'Open or reload a trusted local project. The extension will auto-connect to this bridge.'
}

if($RegisterChrome -or $UnregisterChrome){
  if(-not $ChromeExtensionId){throw 'Provide -ChromeExtensionId with the 32-character ID shown by chrome://extensions.'}
  if(-not (Test-Path -LiteralPath $registration -PathType Leaf)){throw 'The packaged native-host registration script is missing.'}
  New-Item -ItemType Directory -Path $stableRoot -Force | Out-Null
  Copy-Item -LiteralPath $nativeHost -Destination $stableHost -Force
  if($UnregisterChrome){
    & $registration -ExtensionId $ChromeExtensionId -Browser $Browser -HostPath $stableHost -Unregister
  } else {
    & $registration -ExtensionId $ChromeExtensionId -Browser $Browser -HostPath $stableHost
  }
  if($LASTEXITCODE -ne 0){throw "Native-host registration failed with exit code $LASTEXITCODE."}
}

if($Launch){
  Start-Process -FilePath $desktop -WorkingDirectory $packageRoot
  Write-Host 'ContextSleep launched.'
}
