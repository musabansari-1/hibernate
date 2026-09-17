param([Parameter(Mandatory=$true)][string]$FixturePids)
$ErrorActionPreference='Stop'
$ids=@($FixturePids.Split(',')|ForEach-Object {[uint32]$_})
# Only the fixture PID set and native bridges parented by it. Never export raw command lines.
$items=Get-CimInstance Win32_Process -Filter "Name='Code.exe' OR Name='hibernate-host.exe'" | Where-Object {
 $_.ProcessId -in $ids -or ($_.Name -eq 'hibernate-host.exe' -and $_.ParentProcessId -in $ids)
} | ForEach-Object {
 $kind='main-or-cli'
 if($_.Name -eq 'hibernate-host.exe'){$kind='hibernate-bridge'}
 elseif($_.CommandLine -match '--type=renderer'){$kind='renderer'}
 elseif($_.CommandLine -match '--type=gpu-process'){$kind='gpu'}
 elseif($_.CommandLine -match '--utility-sub-type=node.mojom.NodeService'){$kind='node-utility'}
 elseif($_.CommandLine -match '--type=utility'){$kind='other-utility'}
 [pscustomobject]@{pid=[uint32]$_.ProcessId;parent=[uint32]$_.ParentProcessId;role=$kind;created=$_.CreationDate.ToFileTimeUtc().ToString()}
}
ConvertTo-Json -InputObject @($items) -Compress
