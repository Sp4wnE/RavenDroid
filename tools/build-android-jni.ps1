param(
    [Parameter(Mandatory)][string]$JavaHome,
    [ValidateSet('x86_64','arm64-v8a')][string]$Abi='arm64-v8a',
    [ValidateSet('legacy','guest-eh-v1','guest-eh-inline-eager-v1','guest-eh-epoch-v1')][string]$EngineProfile='legacy',
    [switch]$AsyncPresent
)
$ErrorActionPreference='Stop'
$project=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
& (Join-Path $PSScriptRoot 'check-build-resources.ps1') -RequireEmulatorStopped
. (Join-Path $PSScriptRoot 'enter-windows-env.ps1') -JavaHome $JavaHome
$target=if($Abi -eq 'x86_64'){'x86_64-linux-android'}else{'aarch64-linux-android'}
$env:CARGO_TARGET_DIR=Join-Path $project 'native-runtime/target-android'
$ndkBin=Join-Path $env:ANDROID_NDK_HOME 'toolchains/llvm/prebuilt/windows-x86_64/bin'
$linker=Join-Path $ndkBin "$($target)29-clang.cmd"
Set-Item "env:CARGO_TARGET_$($target.ToUpperInvariant().Replace('-','_'))_LINKER" $linker
Set-Item "env:CC_$($target.Replace('-','_'))" $linker
Set-Item "env:AR_$($target.Replace('-','_'))" (Join-Path $ndkBin 'llvm-ar.exe')
$workspace=Split-Path $project -Parent
# Keep builder-specific absolute paths out of distributed native binaries.
$env:CARGO_ENCODED_RUSTFLAGS=@('-C','link-arg=-Wl,-z,max-page-size=16384',"--remap-path-prefix=$workspace=/source",("--remap-path-prefix="+$workspace.Replace('\','/')+'=/source')) -join [char]0x1f
$env:CARGO_BUILD_JOBS='2'
$options=@('rustc','--locked','--offline','--manifest-path',(Join-Path $project 'native-runtime/Cargo.toml'),'--lib','--no-default-features','--release','--target',$target,'--crate-type','cdylib')
if($EngineProfile -ne 'legacy'){$options+=@('--features',$(switch($EngineProfile){'guest-eh-v1'{'android-app-eh'} 'guest-eh-epoch-v1'{'android-app-epoch'} default{'android-app-inline-eager'}}))}
if($AsyncPresent){$options+=@('--features','android-async-present')}
& cargo @options
if($LASTEXITCODE -ne 0){throw 'Android JNI build failed'}
Write-Output "JNI target=$target profile=$EngineProfile"
