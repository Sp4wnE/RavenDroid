param(
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9-]+$')][string]$Tag,
    [Parameter(Mandatory)][string]$TrustedArtifact,
    [string]$Probe,
    [ValidateRange(3120,7200)][int]$Frames=3600,
    [switch]$ProfileImports,
    [switch]$GuestEH,
    [switch]$InlineEagerCodegen,
    [switch]$Traverse,
    [switch]$EpochGuard
)
$ErrorActionPreference='Stop'
if($Traverse -and $Frames -lt 6000){throw 'Traversal profile needs at least 6000 frames'}
$project=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$workspace=[IO.Path]::GetDirectoryName($project)
$adb=Join-Path $workspace '.tools/android-sdk/platform-tools/adb.exe'
$artifact=(Resolve-Path -LiteralPath $TrustedArtifact).Path
$probe=if($Probe){(Resolve-Path -LiteralPath $Probe).Path}else{Join-Path $project 'native-runtime/target-android/x86_64-linux-android/release/raven-native-probe'}
$output=Join-Path $project "artifacts/emulator-$Tag"
if(Test-Path -LiteralPath "$output.json"){throw 'Use a fresh tag; preserve the previous evidence.'}
if((& $adb -s emulator-5580 shell getprop sys.boot_completed).Trim() -ne '1'){throw 'Project emulator is not booted.'}
$remote="/data/local/tmp/ravendroid/run-$Tag"
& $adb -s emulator-5580 shell mkdir -p $remote
$localHash=(Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
$remoteHash=(& $adb -s emulator-5580 shell sha256sum /data/local/tmp/ravendroid/unity.cwasm 2>$null) -join ''
if(-not $remoteHash.StartsWith($localHash)){
    & $adb -s emulator-5580 push -Z $artifact /data/local/tmp/ravendroid/unity.cwasm
    if($LASTEXITCODE -ne 0){throw 'AOT transfer failed'}
    $remoteHash=(& $adb -s emulator-5580 shell sha256sum /data/local/tmp/ravendroid/unity.cwasm) -join ''
    if(-not $remoteHash.StartsWith($localHash)){throw 'Transferred AOT hash mismatch'}
}
& $adb -s emulator-5580 push $probe /data/local/tmp/ravendroid/probe
if($LASTEXITCODE -ne 0){throw 'Probe transfer failed'}
& $adb -s emulator-5580 shell chmod 700 /data/local/tmp/ravendroid/probe
& $adb -s emulator-5580 shell cp /data/local/tmp/ravendroid/test-saves.bin "$remote/native-saves.bin"
$arguments=@('--trusted-artifact','/data/local/tmp/ravendroid/unity.cwasm','--data','/data/local/tmp/ravendroid/game.data','--save-state',"$remote/native-saves.bin",'--android-gles','--headless-events','--native-audio','--frames',"$Frames",'--startup-fuel','50000000000','--frame-fuel','50000000000',
    '--game-input','1200,0,32,1','--game-input','1201,0,32,0','--tap','1500,640,320','--tap','1650,380,580','--tap','1720,1185,687','--tap','1850,980,550',
    '--game-input','2700,2,0,1','--game-input','2710,2,0,0')
$captures=@(2000,3000)
if($Frames -ge 6000){$captures+=6000}
if($Frames -gt 6000){$captures+=$Frames}
foreach($frame in $captures){$arguments+=@('--capture-at',"$frame,$remote/frame-$frame.png")}
if($Traverse){
    # Same scripted input for every comparison; actual AI/time can still differ.
    foreach($event in @('3100,1,-300,0','3150,0,87,1','3390,0,87,0','3500,1,180,0','3600,0,87,1','3840,0,87,0','4400,1,-180,0','4500,0,87,1','4740,0,87,0')){
        $arguments+=@('--game-input',$event)
    }
    # Sweep through open/heavy views too; one stationary wall must not qualify.
    for($frame=3900;$frame -le $Frames-120;$frame+=120){
        $arguments+=@('--game-input',"$frame,1,120,0")
    }
}
if($ProfileImports){$arguments+='--profile-imports'}
if($GuestEH){$arguments+='--guest-eh'}
if($EpochGuard){$arguments+='--epoch-guard'}
if($InlineEagerCodegen){$arguments+='--inline-eager-codegen'}
$command='LD_PRELOAD=/system/lib64/libEGL_angle.so:/system/lib64/libGLESv2_angle.so /data/local/tmp/ravendroid/probe '+($arguments -join ' ')+" >$remote/report.json 2>$remote/progress.log"
& $adb -s emulator-5580 shell $command
$runExit=$LASTEXITCODE
& $adb -s emulator-5580 pull "$remote/report.json" "$output.json"
& $adb -s emulator-5580 pull "$remote/progress.log" "$output.log"
foreach($frame in $captures){& $adb -s emulator-5580 pull "$remote/frame-$frame.png" "$output-frame-$frame.png"}
if($runExit -ne 0){throw "Probe failed ($runExit); inspect $output.log"}
$report=Get-Content -LiteralPath "$output.json" -Raw | ConvertFrom-Json
if($report.first_blocker -or $report.main_loop_frames -ne $Frames -or $report.graphics.pending_gl_error -ne 0 -or $report.graphics.shader_program_failures.Count -ne 0){throw "Game validation failed; inspect $output.json"}
$report.performance.last_300_frames
