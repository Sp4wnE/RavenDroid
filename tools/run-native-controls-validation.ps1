param([Parameter(Mandatory)][string]$JavaHome,[string]$Python)
$ErrorActionPreference='Stop'
$projectRoot=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'enter-windows-env.ps1') -JavaHome $JavaHome
Push-Location $projectRoot
try {
    cargo build --release --locked --offline --manifest-path native-runtime/Cargo.toml --no-default-features --bin raven-native-probe -j4 *> artifacts/controls-probe-build.log
    if($LASTEXITCODE -ne 0){throw 'Probe build failed'}
    Copy-Item -LiteralPath ../.tools/references/native-saves.bin -Destination artifacts/controls-final-saves.bin -Force
    [string[]]$probeArguments=@('--trusted-artifact','../.tools/references/unity-windows-stack.cwasm','--data','../.tools/references/game.data','--save-state','artifacts/controls-final-saves.bin','--desktop-gles','--desktop-keyboard','--headless-events','--headless-audio','--space-at-frame','1200','--tap','1500,640,320','--tap','1650,380,580','--tap','1720,1185,687','--tap','1850,980,550')
    # Wait until the deployment/spawn animation has completed before checking
    # mouse look. Early world-only captures do not establish player control.
    $events=@('2050,1,160,0','2052,1,160,0','2054,1,47,0','2057,0,87,1','2087,0,87,0','2090,2,0,1','2091,2,0,0','2100,2,2,1','2110,2,2,0','2118,0,87,1','2118,2,0,1','2119,1,8,0','2124,0,87,0','2124,2,0,0','2130,3,0,0','2140,0,27,1','2141,0,27,0','2150,0,27,1','2151,0,27,0')
    foreach($event in $events){$probeArguments+=@('--game-input',$event)}
    $captures=@('2049,before','2051,look-one','2053,look-edge','2055,look-beyond','2088,walk','2095,fire','2105,aim','2126,combined','2145,pause')
    foreach($capture in $captures){$parts=$capture.Split(',');$probeArguments+=@('--capture-at',"$($parts[0]),artifacts/controls-final-$($parts[1]).ppm")}
    $probeArguments+=@('--frames','2180','--startup-fuel','50000000000','--frame-fuel','50000000000','--capture','artifacts/controls-final.ppm')
    & ./native-runtime/target/release/raven-native-probe.exe @probeArguments > artifacts/controls-final.json 2> artifacts/controls-final.log
    if($LASTEXITCODE -ne 0){throw 'Probe process failed'}
    $report=Get-Content artifacts/controls-final.json -Raw | ConvertFrom-Json
    if($report.first_blocker -or $report.main_loop_frames -ne 2180 -or $report.pointer.buttons -ne 0 -or $report.input.virtual_controls.held_keys.Count -ne 0){throw 'Input did not finish cleanly; inspect controls-final.json'}
    if($Python){
        & $Python -X utf8 -c 'from pathlib import Path; from PIL import Image; [Image.open(p).save(p.with_suffix(".png")) for p in Path("artifacts").glob("controls-final*.ppm")]'
        if($LASTEXITCODE -ne 0){throw 'PNG conversion failed'}
    }
    [pscustomobject]@{Stage=$report.stage;Frames=$report.main_loop_frames;Blocker=$report.first_blocker;Keyboard=$report.input;Pointer=$report.pointer;Report='artifacts/controls-final.json'}
} finally {Pop-Location}
