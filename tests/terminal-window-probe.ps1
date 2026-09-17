# Read-only native observations plus self-exiting, uniquely named fixture windows.
# Window differences are TEST observations, never product close authority.
$ErrorActionPreference='Stop'
$probeRoot=Join-Path ([IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\.validation\terminal-ownership'))) ('run-'+[Guid]::NewGuid())
New-Item -ItemType Directory -Path $probeRoot -Force | Out-Null
Add-Type @'
using System;using System.Collections.Generic;using System.Runtime.InteropServices;using System.Text;
public class TerminalWindowProbe {
 public delegate bool EnumProc(IntPtr h,IntPtr p);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback,IntPtr p);
 [DllImport("user32.dll")] static extern int GetClassName(IntPtr h,StringBuilder b,int n);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 public class Window {public long Handle;public uint Pid;}
 public static Window[] Windows(){var list=new List<Window>();EnumWindows((h,p)=>{var b=new StringBuilder(256);GetClassName(h,b,256);if(b.ToString()=="CASCADIA_HOSTING_WINDOW_CLASS"){uint id;GetWindowThreadProcessId(h,out id);list.Add(new Window{Handle=h.ToInt64(),Pid=id});}return true;},IntPtr.Zero);return list.ToArray();}
}
'@
$helper=Join-Path $probeRoot 'fixture.ps1'
@'
param([string]$OutputPath,[string]$StopPath)
$ErrorActionPreference='Stop'
try {
Add-Type @"
using System;using System.Runtime.InteropServices;using System.Text;
public class OwnConsoleProbe {
 [DllImport("kernel32.dll")]public static extern IntPtr GetConsoleWindow();
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")]public static extern int GetClassName(IntPtr h,StringBuilder b,int n);
}
"@
$console=[OwnConsoleProbe]::GetConsoleWindow()
$class=New-Object Text.StringBuilder 256
$null=[OwnConsoleProbe]::GetClassName($console,$class,256)
[IO.File]::WriteAllText($OutputPath,(@{pid=$PID;started=(Get-Process -Id $PID).StartTime.ToFileTimeUtc().ToString();consoleHandle=$console.ToInt64();consoleVisible=[OwnConsoleProbe]::IsWindowVisible($console);consoleClass=$class.ToString();wtSession=$env:WT_SESSION;hasComProtocol=(-not [string]::IsNullOrEmpty($env:WT_COM_CLSID))}|ConvertTo-Json))
$deadline=(Get-Date).AddSeconds(90)
while(-not (Test-Path -LiteralPath $StopPath) -and (Get-Date) -lt $deadline){Start-Sleep -Milliseconds 100}
} catch { [IO.File]::WriteAllText($OutputPath,(@{error=$_.ToString()}|ConvertTo-Json)) }
exit 0
'@ | Set-Content -LiteralPath $helper -Encoding UTF8
$wt=Join-Path $env:LOCALAPPDATA 'Microsoft\WindowsApps\wt.exe'
$shell=Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
$nameA='Hibernate-fixture-'+[Guid]::NewGuid()
$nameB='Hibernate-fixture-'+[Guid]::NewGuid()
function Launch-Fixture([string]$name,[string]$id){
 $out=Join-Path $probeRoot ($id+'.json');$stop=Join-Path $probeRoot ($id+'.stop')
 & $wt -w $name new-tab $shell -NoLogo -NoProfile -ExecutionPolicy Bypass -File $helper -OutputPath $out -StopPath $stop
 $end=(Get-Date).AddSeconds(45)
 while(-not(Test-Path -LiteralPath $out) -and (Get-Date) -lt $end){Start-Sleep -Milliseconds 100}
 if(-not(Test-Path -LiteralPath $out)){throw "Fixture $id did not report"}
 $report=Get-Content -Raw -LiteralPath $out | ConvertFrom-Json
 if($report.error){throw $report.error}
 $report
}
try {
 $before=@([TerminalWindowProbe]::Windows())
 $a=Launch-Fixture $nameA 'a'
 $afterA=@([TerminalWindowProbe]::Windows())
 $aWindow=@($afterA|Where-Object {$_.Handle -notin $before.Handle})
 if($aWindow.Count -ne 1){throw 'Ambiguous fixture window difference; no window will be targeted.'}
 $b=Launch-Fixture $nameB 'b'
 $afterB=@([TerminalWindowProbe]::Windows())
 $bWindow=@($afterB|Where-Object {$_.Handle -notin $afterA.Handle})
 if($bWindow.Count -ne 1){throw 'Ambiguous second fixture window difference.'}
 $added=Launch-Fixture $nameA 'added'
 $afterAdded=@([TerminalWindowProbe]::Windows())
 if(@($afterAdded|Where-Object {$_.Handle -notin $afterB.Handle}).Count -ne 0){throw 'Named window did not accept the additional fixture tab as expected.'}
 if($a.consoleHandle -eq $aWindow[0].Handle){throw 'Unexpected console/window mapping; review this Terminal version.'}
 New-Item -ItemType File -Path (Join-Path $probeRoot 'a.stop') | Out-Null
 $end=(Get-Date).AddSeconds(10)
 while((Get-Process -Id $a.pid -ErrorAction SilentlyContinue) -and (Get-Date) -lt $end){Start-Sleep -Milliseconds 100}
 if(Get-Process -Id $a.pid -ErrorAction SilentlyContinue){throw 'Fixture shell did not exit itself.'}
 $remaining=@([TerminalWindowProbe]::Windows())
 if($aWindow[0].Handle -notin $remaining.Handle -or $bWindow[0].Handle -notin $remaining.Handle){throw 'Expected fixture window disappeared.'}
 $null=Get-Process -Id $added.pid -ErrorAction Stop
 $null=Get-Process -Id $b.pid -ErrorAction Stop
 $evidence=@{terminalVersion=(Get-AppxPackage Microsoft.WindowsTerminal).Version.ToString();a=$a;b=$b;added=$added;windowA=$aWindow[0];windowB=$bWindow[0];sharedHostObserved=($aWindow[0].Pid -eq $bWindow[0].Pid);checks=@('Named fixture window accepted an additional independent tab','Shell console HWND differed from visible Terminal HWND; visibility flag is not ownership proof','First fixture shell self-exited while its window and extra tab remained','Separate unrelated fixture window/shell remained open');decision='No window close authority inferred; no WM_CLOSE or host termination sent.'}
 $evidence|ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $probeRoot 'results.json') -Encoding UTF8
 $evidence|ConvertTo-Json -Depth 8
 Write-Output "Evidence: $probeRoot"
} finally {
 # Cooperative test-script exit, only through marker files private to this fixture run.
 foreach($id in @('a','b','added')){New-Item -ItemType File -Path (Join-Path $probeRoot ($id+'.stop')) -Force | Out-Null}
}
