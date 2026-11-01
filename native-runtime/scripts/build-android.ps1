param(
    [string]$SdkRoot = $env:ANDROID_HOME,
    [string]$NdkRoot = $env:ANDROID_NDK_HOME,
    [string]$JavaHome = $env:JAVA_HOME,
    [string]$Cargo = 'cargo',
    [string]$GradleCommand,
    [switch]$SkipNativeBuild,
    [switch]$Clean
)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (-not $GradleCommand) { $GradleCommand = Join-Path $projectRoot 'gradlew.bat' }
if (-not $SdkRoot -or -not (Test-Path -LiteralPath $SdkRoot)) { throw 'Set -SdkRoot or ANDROID_HOME to the Android SDK.' }
if (-not $JavaHome -or -not (Test-Path -LiteralPath (Join-Path $JavaHome 'bin/java.exe'))) { throw 'Set -JavaHome or JAVA_HOME to JDK 17/21.' }
$saved = @{}
$variables = @('ANDROID_HOME','JAVA_HOME','CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER','CC_aarch64_linux_android','AR_aarch64_linux_android','RUSTFLAGS','CARGO_BUILD_JOBS')
foreach ($name in $variables) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    $env:ANDROID_HOME = (Resolve-Path -LiteralPath $SdkRoot).Path
    $env:JAVA_HOME = (Resolve-Path -LiteralPath $JavaHome).Path
    if (-not $SkipNativeBuild) {
        if (-not $NdkRoot) { throw 'Set -NdkRoot or ANDROID_NDK_HOME to NDK r28c.' }
        $ndkTools = Join-Path $NdkRoot 'toolchains/llvm/prebuilt/windows-x86_64/bin'
        $linker = Join-Path $ndkTools 'aarch64-linux-android29-clang.cmd'
        if (-not (Test-Path -LiteralPath $linker)) { throw "Android linker missing: $linker" }
        $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $linker
        $env:CC_aarch64_linux_android = $linker
        $env:AR_aarch64_linux_android = Join-Path $ndkTools 'llvm-ar.exe'
        $env:RUSTFLAGS = (($saved['RUSTFLAGS'] + ' -C link-arg=-Wl,-z,max-page-size=16384').Trim())
        if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '2' }
        & $Cargo rustc --locked --manifest-path (Join-Path $projectRoot 'native-runtime/Cargo.toml') --crate-type cdylib --lib --no-default-features --release --target aarch64-linux-android
        if ($LASTEXITCODE -ne 0) { throw 'Android Rust build failed; existing packaged JNI was preserved.' }
        $targetRoot = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $projectRoot 'native-runtime/target' }
        $library = Join-Path $targetRoot 'aarch64-linux-android/release/libravenfield_native_host.so'
        $destination = Join-Path $projectRoot 'native-android/app/src/main/jniLibs/arm64-v8a'
        New-Item -ItemType Directory -Force -Path $destination | Out-Null
        Copy-Item -LiteralPath $library -Destination $destination -Force
    }
    $gradleTasks = @(':nativeApp:assembleDebug',':nativeApp:testDebugUnitTest',':nativeApp:lintDebug')
    if ($Clean) { $gradleTasks = @(':nativeApp:clean') + $gradleTasks }
    & $GradleCommand -p $projectRoot @gradleTasks --no-daemon --console=plain
    if ($LASTEXITCODE -ne 0) { throw 'Android packaging, tests or lint failed.' }
} finally {
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
