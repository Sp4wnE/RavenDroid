param([ValidateSet('host','swiftshader','swangle')][string]$Gpu='host')
$ErrorActionPreference='Stop'
$project=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$workspace=[IO.Path]::GetDirectoryName($project)
$sdk=Join-Path $workspace '.tools/android-sdk'
$avdHome=Join-Path $workspace '.tools/ravendroid-avd'
$device=Join-Path $avdHome 'RavenDroid35.avd'
$image=Join-Path $sdk 'system-images/android-35/default/x86_64'
if(-not(Test-Path -LiteralPath (Join-Path $image 'system.img'))){throw 'Install the API35 AOSP x86_64 image first.'}
$pidFile=Join-Path $project 'artifacts/emulator-035.pid'
if(Test-Path -LiteralPath $pidFile){
    $previous=Get-Process -Id ([int](Get-Content -LiteralPath $pidFile)) -ErrorAction SilentlyContinue
    if($previous){throw 'Previous test-emulator process is still running; inspect it before relaunch.'}
}
New-Item -ItemType Directory -Force -Path $device | Out-Null
@"
avd.ini.encoding=UTF-8
path=$device
target=android-35
"@ | Set-Content -LiteralPath (Join-Path $avdHome 'RavenDroid35.ini') -Encoding utf8
@"
AvdId=RavenDroid35
avd.ini.displayname=RavenDroid API35 Test
abi.type=x86_64
hw.cpu.arch=x86_64
hw.cpu.ncore=4
hw.ramSize=4096
hw.gpu.enabled=yes
hw.gpu.mode=$Gpu
hw.lcd.width=720
hw.lcd.height=1280
hw.lcd.density=320
hw.keyboard=yes
hw.audioInput=no
hw.audioOutput=yes
disk.dataPartition.size=8G
image.sysdir.1=$image
tag.id=default
tag.display=Default
target=android-35
showDeviceFrame=no
fastboot.forceColdBoot=yes
"@ | Set-Content -LiteralPath (Join-Path $device 'config.ini') -Encoding utf8
$env:ANDROID_AVD_HOME=$avdHome
$env:ANDROID_EMULATOR_HOME=Join-Path $workspace '.tools/android-user-home'
$env:ANDROID_HOME=$sdk
$emulator=Join-Path $sdk 'emulator/emulator.exe'
$process=Start-Process -FilePath $emulator -ArgumentList @('-avd','RavenDroid35','-port','5580','-no-window','-no-snapshot','-no-boot-anim','-gpu',$Gpu) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $project 'artifacts/emulator-035-stdout.log') -RedirectStandardError (Join-Path $project 'artifacts/emulator-035-stderr.log')
$process.Id | Set-Content -LiteralPath $pidFile
Write-Output "Started PID=$($process.Id) serial=emulator-5580 gpu=$Gpu"
