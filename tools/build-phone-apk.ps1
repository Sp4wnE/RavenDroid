param(
    [Parameter(Mandatory)][string]$JavaHome,
    [switch]$BundledData,
    [string]$Gradle='gradle'
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'enter-windows-env.ps1') -JavaHome $JavaHome
& (Join-Path $PSScriptRoot 'check-build-resources.ps1') -RequireEmulatorStopped
# JNI and reviewed ARM64 AOT must already be staged; never rebuild a legacy JNI here.
$bundle=if($BundledData){'true'}else{'false'}
$workers=if($BundledData){1}else{2}
& $Gradle --offline --no-daemon "--max-workers=$workers" :nativeApp:assembleDebug :nativeApp:testDebugUnitTest :nativeApp:lintDebug -PravenEmulatorTest=false "-PravenBundleData=$bundle"
if($LASTEXITCODE -ne 0){throw 'Phone APK build/test/lint failed'}
