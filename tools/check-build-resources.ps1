# Read-only preflight. Does not alter pagefile, processes or system settings.
param(
    [int]$MinDiskGiB=12,
    [int]$MinAvailableRamGiB=6,
    [int]$MinCommitHeadroomGiB=12,
    [switch]$RequireEmulatorStopped
)
$ErrorActionPreference='Stop'
if(-not ('RavenBuildMemory' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class RavenBuildMemory {
    [StructLayout(LayoutKind.Sequential)] public struct Status {
        public uint length, load;
        public ulong totalPhysical, availablePhysical, totalPageFile, availablePageFile,
            totalVirtual, availableVirtual, availableExtendedVirtual;
    }
    [DllImport("kernel32.dll", SetLastError=true)]
    public static extern bool GlobalMemoryStatusEx(ref Status status);
}
'@
}
$status=New-Object RavenBuildMemory+Status
$status.length=[Runtime.InteropServices.Marshal]::SizeOf($status)
if(-not [RavenBuildMemory]::GlobalMemoryStatusEx([ref]$status)){throw 'Cannot read memory budget'}
$project=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$disk=[IO.DriveInfo]::new([IO.Path]::GetPathRoot($project))
$report=[pscustomobject]@{
    freeDiskGiB=[math]::Round($disk.AvailableFreeSpace/1GB,2)
    availableRamGiB=[math]::Round($status.availablePhysical/1GB,2)
    commitHeadroomGiB=[math]::Round($status.availablePageFile/1GB,2)
    commitLimitGiB=[math]::Round($status.totalPageFile/1GB,2)
}
$report
if($disk.AvailableFreeSpace -lt $MinDiskGiB*1GB -or
    $status.availablePhysical -lt $MinAvailableRamGiB*1GB -or
    $status.availablePageFile -lt $MinCommitHeadroomGiB*1GB){throw 'Insufficient build resource budget; do not start another build'}
if($RequireEmulatorStopped -and (Get-Process -Name 'emulator','qemu-system-x86_64','qemu-system-x86_64-headless' -ErrorAction SilentlyContinue)) {
    throw 'Stop the emulator before a heavy build'
}
