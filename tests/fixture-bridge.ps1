param([Parameter(Mandatory=$true)][string]$Name,[Parameter(Mandatory=$true)][string]$Manifest,[switch]$Remove)
$ErrorActionPreference='Stop'
if($Name -notmatch '^local\.contextsleep\.fixture\.[a-f0-9]{32}$'){throw 'Not an isolated fixture host name'}
$fixtureRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\.validation'))+[IO.Path]::DirectorySeparatorChar
$fixtureManifest=[IO.Path]::GetFullPath($Manifest)
if(-not $fixtureManifest.StartsWith($fixtureRoot,[StringComparison]::OrdinalIgnoreCase)){throw 'Fixture manifest must stay in this workspace validation directory'}
$fixtureKey="HKCU:\Software\Google\Chrome\NativeMessagingHosts\$Name"
if($Remove){
  if((Test-Path -LiteralPath $fixtureKey) -and (Get-Item -LiteralPath $fixtureKey).GetValue('') -eq $fixtureManifest){Remove-Item -LiteralPath $fixtureKey}
}else{
  if(Test-Path -LiteralPath $fixtureKey){throw 'Fixture key already exists; refusing replacement'}
  New-Item -Path $fixtureKey -Force | Out-Null
  Set-Item -LiteralPath $fixtureKey -Value $fixtureManifest
}
