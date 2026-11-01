param(
    [Parameter(Mandatory)][string]$JavaHome,
    [ValidateRange(0,7200)][int]$Frames = 300,
    [UInt64]$StartupFuel = 50000000000,
    [UInt64]$FrameFuel = 50000000000,
    [string]$Python,
    [switch]$DesktopKeyboard,
    [ValidateRange(0,7198)][int]$SpaceAtFrame = 0,
    [ValidateRange(0,7198)][int]$TapAtFrame = 0,
    [ValidateRange(0,1279)][int]$TapX = 640,
    [ValidateRange(0,719)][int]$TapY = 360,
    [switch]$DebugHost
)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'enter-windows-env.ps1') -JavaHome $JavaHome
$references = (Resolve-Path (Join-Path $projectRoot '../.tools/references')).Path
$artifacts = Join-Path $projectRoot 'artifacts'
New-Item -ItemType Directory -Force -Path $artifacts | Out-Null
Push-Location $projectRoot
try {
    # This cache must have been compiled locally from the reviewed source.
    # Never substitute an untrusted downloaded serialized Wasmtime artifact.
    $cache = Join-Path $references 'unity-windows-stack.cwasm'
    $data = Join-Path $references 'game.data'
    foreach ($path in @($cache,$data)) { if (-not (Test-Path -LiteralPath $path)) { throw "Missing local input: $path" } }
    [string[]]$profileArguments = if ($DebugHost) { @() } else { @('--release') }
    cargo build @profileArguments --locked --offline --manifest-path native-runtime/Cargo.toml --no-default-features --bin raven-native-probe -j 4 *> (Join-Path $artifacts 'gles-build-progress.log')
    if ($LASTEXITCODE -ne 0) { throw 'Native desktop host build failed.' }
    $buildProfile = if ($DebugHost) { 'debug' } else { 'release' }
    $exe = Join-Path $projectRoot "native-runtime/target/$buildProfile/raven-native-probe.exe"
    $capture = Join-Path $artifacts 'native-game-frame.ppm'
    $reportPath = Join-Path $artifacts 'windows-startup-gles-capture.json'
    [string[]]$inputArguments = @()
    if ($DesktopKeyboard) { $inputArguments += '--desktop-keyboard' }
    if ($SpaceAtFrame -gt 0) {
        if (-not $DesktopKeyboard -or $SpaceAtFrame+1 -ge $Frames) { throw 'SpaceAtFrame requires DesktopKeyboard and two later frames.' }
        $inputArguments += @('--space-at-frame',"$SpaceAtFrame")
    }
    if ($TapAtFrame -gt 0) {
        if ($TapAtFrame+1 -ge $Frames) { throw 'TapAtFrame requires two later frames.' }
        $inputArguments += @('--tap-at-frame',"$TapAtFrame",'--tap-x',"$TapX",'--tap-y',"$TapY")
    }
    & $exe --trusted-artifact $cache --data $data --save-state (Join-Path $references 'native-saves.bin') --desktop-gles --headless-events --headless-audio @inputArguments --frames $Frames --startup-fuel $StartupFuel --frame-fuel $FrameFuel --capture $capture > $reportPath 2> (Join-Path $artifacts 'windows-startup-gles-capture.log')
    if ($LASTEXITCODE -ne 0) { throw 'Native probe failed; inspect its log.' }
    $report = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
    if ($report.graphics.capture.sha256) {
        if ((Get-FileHash -LiteralPath $capture -Algorithm SHA256).Hash.ToLowerInvariant() -ne $report.graphics.capture.sha256) { throw 'Capture hash does not match this run.' }
        if ($Python) {
            & $Python -c 'from PIL import Image; import sys; Image.open(sys.argv[1]).save(sys.argv[2])' $capture (Join-Path $artifacts 'native-game-frame.png')
            if ($LASTEXITCODE -ne 0) { throw 'PNG conversion failed.' }
        }
    }
    [pscustomobject]@{ Stage=$report.stage; Frames=$report.main_loop_frames; DrawCalls=$report.graphics.draw_calls; RenderedFrame=$report.rendered_game_frame; Playable=$report.playable; Blocker=$report.first_blocker; Report=$reportPath }
} finally { Pop-Location }
