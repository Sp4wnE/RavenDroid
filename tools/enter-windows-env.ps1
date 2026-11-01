# Dot-source this file to configure only the current PowerShell process.
param(
    [string]$ToolsRoot = (Join-Path $PSScriptRoot '../../.tools'),
    [Parameter(Mandatory)][string]$JavaHome,
    [string]$SdkRoot = (Join-Path $ToolsRoot 'android-sdk')
)
$ErrorActionPreference = 'Stop'
$ToolsRoot = (Resolve-Path -LiteralPath $ToolsRoot).Path
$env:JAVA_HOME = (Resolve-Path -LiteralPath $JavaHome).Path
$env:ANDROID_HOME = (Resolve-Path -LiteralPath $SdkRoot).Path
$env:ANDROID_NDK_HOME = Join-Path $env:ANDROID_HOME 'ndk/28.2.13676358'
$env:CARGO_HOME = Join-Path $ToolsRoot 'cargo'
$env:RUSTUP_HOME = Join-Path $ToolsRoot 'rustup'
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnullvm'
$env:GRADLE_USER_HOME = Join-Path $ToolsRoot 'gradle'
$taskTemp = Join-Path $ToolsRoot 'tmp'
New-Item -ItemType Directory -Force -Path $taskTemp | Out-Null
$env:TEMP = $taskTemp
$env:TMP = $taskTemp
$llvm = Get-ChildItem -LiteralPath $ToolsRoot -Directory -Filter 'llvm-mingw-*-ucrt-x86_64' | Sort-Object Name | Select-Object -Last 1
if (-not $llvm) { throw 'Extract the official LLVM-MinGW UCRT x86_64 toolchain under ToolsRoot first.' }
$llvmBin = Join-Path $llvm.FullName 'bin'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER = Join-Path $llvmBin 'x86_64-w64-mingw32-clang.exe'
$env:CC_x86_64_pc_windows_gnullvm = $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER
$env:AR_x86_64_pc_windows_gnullvm = Join-Path $llvmBin 'llvm-ar.exe'
$env:PATH = (Join-Path $env:CARGO_HOME 'bin') + ';' + $llvmBin + ';' + (Join-Path $env:JAVA_HOME 'bin') + ';' + $env:PATH
