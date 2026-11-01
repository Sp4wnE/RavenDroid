param([Parameter(Mandatory)][string]$JavaHome,[switch]$GuestEH)
$ErrorActionPreference='Stop'
$project=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
& (Join-Path $PSScriptRoot 'check-build-resources.ps1') -RequireEmulatorStopped
. (Join-Path $PSScriptRoot 'enter-windows-env.ps1') -JavaHome $JavaHome
$env:CARGO_TARGET_DIR=Join-Path $project 'native-runtime/target-android'
$ndkBin=Join-Path $env:ANDROID_NDK_HOME 'toolchains/llvm/prebuilt/windows-x86_64/bin'
$env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=Join-Path $ndkBin 'x86_64-linux-android29-clang.cmd'
$env:CC_x86_64_linux_android=$env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER
$env:AR_x86_64_linux_android=Join-Path $ndkBin 'llvm-ar.exe'
$env:RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384'
$env:CARGO_BUILD_JOBS='2'
$options=@('build','--locked','--offline','--manifest-path',(Join-Path $project 'native-runtime/Cargo.toml'),'--bin','raven-native-probe','--no-default-features','--release','--target','x86_64-linux-android')
if($GuestEH){$options+=@('--features','guest-eh-prototype')}
& cargo @options
if($LASTEXITCODE -ne 0){throw 'Emulator probe build failed'}
