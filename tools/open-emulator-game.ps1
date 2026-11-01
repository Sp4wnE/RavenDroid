# Start only RavenDroid's existing launcher; never navigate another app.
param([Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9-]+$')][string]$Tag)
$ErrorActionPreference='Stop'
$project=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$adb=Join-Path $project '../.tools/android-sdk/platform-tools/adb.exe'
$uiFile=Join-Path $project "artifacts/emulator-launcher-$Tag.xml"
if(Test-Path -LiteralPath $uiFile){throw 'Use a fresh UI evidence tag'}
$remoteUi="/sdcard/ravendroid-launcher-$Tag.xml"
$dumped=$false
for($attempt=0;$attempt -lt 5;$attempt++){
    $dump=(& $adb -s emulator-5580 shell uiautomator dump $remoteUi) -join ''
    if($LASTEXITCODE -eq 0 -and $dump -match 'dumped to:'){$dumped=$true;break}
    Start-Sleep -Seconds 1
}
if(-not $dumped){throw 'Unable to read a fresh launcher UI; do not reuse an old dump'}
& $adb -s emulator-5580 pull $remoteUi $uiFile
if($LASTEXITCODE -ne 0){throw 'Unable to capture launcher UI'}
[xml]$ui=Get-Content -LiteralPath $uiFile
$play=$ui.SelectNodes('//node')|Where-Object {
    $_.package -eq 'dev.ravenfield.nativehost.emutest' -and $_.enabled -eq 'true' -and
    $_.clickable -eq 'true' -and $_.text -like '*LAUNCH GAME*'
}|Select-Object -First 1
if(-not $play){throw 'Enabled RavenDroid launcher button missing; inspect the UI before continuing'}
$bounds=[regex]::Matches($play.bounds,'\d+')|ForEach-Object {[int]$_.Value}
if($bounds.Count -ne 4){throw 'Invalid launcher button bounds'}
$x=[int](($bounds[0]+$bounds[2])/2);$y=[int](($bounds[1]+$bounds[3])/2)
$begun=[long]((& $adb -s emulator-5580 shell date +%s).Trim())*1000
& $adb -s emulator-5580 shell input touchscreen swipe $x $y $x $y 160
for($i=0;$i -lt 45;$i++){
    $line=(& $adb -s emulator-5580 shell run-as dev.ravenfield.nativehost.emutest tail -n 1 files/native-progress.log) -join ''
    if($line -match '^(\d+) main-loop-completed=(\d+)' -and [long]$Matches[1] -ge $begun -and [int]$Matches[2] -ge 900){
        Write-Output "New RavenDroid game session ready; frame=$($Matches[2]); ui=$uiFile"
        return
    }
    Start-Sleep -Seconds 1
}
throw 'The new session did not reach its title menu; do not send blind game taps'
