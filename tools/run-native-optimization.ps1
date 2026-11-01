param(
    [Parameter(Mandatory)][string]$JavaHome,
    [Parameter(Mandatory)][string]$Probe,
    [Parameter(Mandatory)][string]$TrustedArtifact,
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9-]+$')][string]$Tag,
    [ValidateRange(2180,7200)][int]$Frames=2400,
    [switch]$ProfileImports
)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'enter-windows-env.ps1') -JavaHome $JavaHome
Push-Location $root
try {
    $probePath=(Resolve-Path -LiteralPath $Probe).Path
    $artifactPath=(Resolve-Path -LiteralPath $TrustedArtifact).Path
    $output="artifacts/optimization-$Tag"
    if(Test-Path "$output.json"){throw 'Use a new tag to preserve comparison evidence'}
    Copy-Item -LiteralPath ../.tools/references/native-saves.bin -Destination "$output-saves.bin"
    [string[]]$extra=if($ProfileImports){@('--profile-imports')}else{@()}
    # No raw PPM capture. Preserve the original save and quarantined files.
    & $probePath --trusted-artifact $artifactPath --data ../.tools/references/game.data --save-state "$output-saves.bin" --desktop-gles --desktop-keyboard --headless-events --audio-wav "$output.wav" --space-at-frame 1200 --tap '1500,640,320' --tap '1650,380,580' --tap '1720,1185,687' --tap '1850,980,550' --game-input '2090,2,0,1' --game-input '2091,2,0,0' --frames $Frames --startup-fuel 50000000000 --frame-fuel 50000000000 @extra > "$output.json" 2> "$output.log"
    if($LASTEXITCODE -ne 0){throw "Probe process failed: $output.log"}
    $r=Get-Content "$output.json" -Raw | ConvertFrom-Json
    if($r.first_blocker -or $r.main_loop_frames -ne $Frames -or $r.audio.errors.Count -ne 0 -or $r.audio.decode_failures -ne 0 -or $r.audio.loaded_clips -lt 100 -or $r.audio.nonzero_samples -eq 0 -or $r.graphics.pending_gl_error -ne 0 -or $r.graphics.shader_program_failures.Count -ne 0){throw "Validation failed: $output.json"}
    $r.performance.last_300_frames
}finally{Pop-Location}
