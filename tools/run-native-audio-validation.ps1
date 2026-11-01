param([Parameter(Mandatory)][string]$JavaHome)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'enter-windows-env.ps1') -JavaHome $JavaHome
Push-Location $root
try {
    cargo build --release --locked --offline --manifest-path native-runtime/Cargo.toml --no-default-features --bin raven-native-probe -j4
    if($LASTEXITCODE -ne 0){throw 'Probe build failed'}
    Copy-Item -LiteralPath ../.tools/references/native-saves.bin -Destination artifacts/audio-final-saves.bin -Force
    # Deliberately no GPU readback capture. Quarantined images remain untouched.
    [string[]]$probeArgs=@('--trusted-artifact','../.tools/references/unity-windows-stack.cwasm','--data','../.tools/references/game.data','--save-state','artifacts/audio-final-saves.bin','--desktop-gles','--desktop-keyboard','--headless-events','--audio-wav','artifacts/audio-final.wav','--space-at-frame','1200','--tap','1500,640,320','--tap','1650,380,580','--tap','1720,1185,687','--tap','1850,980,550','--game-input','2090,2,0,1','--game-input','2091,2,0,0','--frames','2180','--startup-fuel','50000000000','--frame-fuel','50000000000')
    & ./native-runtime/target/release/raven-native-probe.exe @probeArgs > artifacts/audio-final.json 2> artifacts/audio-final.log
    if($LASTEXITCODE -ne 0){throw 'Probe process failed'}
    $r=Get-Content artifacts/audio-final.json -Raw | ConvertFrom-Json
    if($r.first_blocker -or $r.main_loop_frames -ne 2180 -or $r.audio.errors.Count -ne 0 -or $r.audio.decode_failures -ne 0 -or $r.audio.loaded_clips -lt 100 -or $r.audio.plays -eq 0 -or $r.audio.nonzero_samples -eq 0 -or $r.audio.decoded_resident_bytes -gt 268435456){throw 'Actual-game audio validation failed; inspect artifacts/audio-final.json'}
    $r.audio
}finally{Pop-Location}
