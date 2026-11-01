param(
    [Parameter(Mandatory)][string]$JavaHome,
    [string]$Gradle='gradle'
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'enter-windows-env.ps1') -JavaHome $JavaHome
& (Join-Path $PSScriptRoot 'check-build-resources.ps1') -RequireEmulatorStopped
& $Gradle --offline --no-daemon --max-workers=2 :nativeApp:assembleDebug :nativeApp:testDebugUnitTest :nativeApp:lintDebug -PravenEmulatorTest=true
if($LASTEXITCODE -ne 0){throw 'Emulator APK build/test/lint failed'}
