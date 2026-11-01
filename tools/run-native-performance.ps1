param(
    [Parameter(Mandatory)][string]$JavaHome,
    [Parameter(Mandatory)][string]$TrustedArtifact,
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9-]+$')][string]$Tag,
    [ValidateRange(1900,7200)][int]$Frames = 3600,
    [switch]$ProfileImports
)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'enter-windows-env.ps1') -JavaHome $JavaHome
$references = (Resolve-Path (Join-Path $projectRoot '../.tools/references')).Path
$output = Join-Path $projectRoot "artifacts/performance-$Tag"
$cache = (Resolve-Path -LiteralPath $TrustedArtifact).Path
$seed = Join-Path $projectRoot 'artifacts/performance-seed.bin'
if (-not (Test-Path -LiteralPath $seed)) { Copy-Item -LiteralPath (Join-Path $references 'native-saves.bin') -Destination $seed }
# Every run gets the same isolated preferences. Never overwrite the user's
# persistent save snapshot or infer a scene from a frame number alone.
Copy-Item -LiteralPath $seed -Destination "$output-saves.bin" -Force
[string[]]$extra = if ($ProfileImports) { @('--profile-imports') } else { @() }
Push-Location $projectRoot
try {
    & ./native-runtime/target/release/raven-native-probe.exe --trusted-artifact $cache --data (Join-Path $references 'game.data') --save-state "$output-saves.bin" --desktop-gles --desktop-keyboard --headless-events --headless-audio --space-at-frame 1200 --tap '1500,640,320' --tap '1650,380,580' --tap '1720,1185,687' --tap '1850,980,550' --frames $Frames --startup-fuel 50000000000 --frame-fuel 50000000000 --capture "$output.ppm" @extra > "$output.json" 2> "$output.log"
    if ($LASTEXITCODE -ne 0) { throw "Probe failed: $output.log" }
    $report = Get-Content -LiteralPath "$output.json" -Raw | ConvertFrom-Json
    if ($report.first_blocker -or $report.main_loop_frames -ne $Frames -or -not $report.rendered_game_frame) { throw "Incomplete probe; inspect $output.json" }
    [pscustomobject]@{Tag=$Tag;StartupMs=$report.startup_ms;WorkMs=$report.main_loop_work_ms;Tail=$report.performance.last_300_frames;Report="$output.json"}
} finally { Pop-Location }
