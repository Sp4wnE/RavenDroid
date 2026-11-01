"""Run a local build in a Windows Job with a hard commit limit (no system changes)."""
import argparse
import ctypes as C
from ctypes import wintypes as W
import json
from pathlib import Path
import shutil
import subprocess
import time


class Basic(C.Structure):
    _fields_ = [('process_time', C.c_int64), ('job_time', C.c_int64), ('flags', W.DWORD),
                ('min_ws', C.c_size_t), ('max_ws', C.c_size_t), ('processes', W.DWORD),
                ('affinity', C.c_size_t), ('priority', W.DWORD), ('scheduling', W.DWORD)]


class Limits(C.Structure):
    _fields_ = [('basic', Basic), ('io', C.c_uint64 * 6), ('process_memory', C.c_size_t),
                ('job_memory', C.c_size_t), ('peak_process', C.c_size_t), ('peak_job', C.c_size_t)]


class Memory(C.Structure):
    _fields_ = [('length', W.DWORD), ('load', W.DWORD)] + [
        (n, C.c_uint64) for n in ('total_ram', 'free_ram', 'total_commit', 'free_commit',
                                'total_virtual', 'free_virtual', 'extended_virtual')]


def run(command, limit_mib, log, report):
    kernel = C.WinDLL('kernel32', use_last_error=True)
    kernel.CreateJobObjectW.argtypes = [C.c_void_p, W.LPCWSTR]
    kernel.CreateJobObjectW.restype = W.HANDLE
    kernel.SetInformationJobObject.argtypes = [W.HANDLE, C.c_int, C.c_void_p, W.DWORD]
    kernel.AssignProcessToJobObject.argtypes = [W.HANDLE, W.HANDLE]
    kernel.QueryInformationJobObject.argtypes = [W.HANDLE, C.c_int, C.c_void_p, W.DWORD, C.c_void_p]
    kernel.TerminateJobObject.argtypes = [W.HANDLE, W.UINT]
    kernel.CloseHandle.argtypes = [W.HANDLE]
    kernel.GlobalMemoryStatusEx.argtypes = [C.POINTER(Memory)]
    memory = Memory()
    memory.length = C.sizeof(memory)

    def budget():
        if not kernel.GlobalMemoryStatusEx(C.byref(memory)):
            raise C.WinError(C.get_last_error())
        return dict(free_ram=memory.free_ram, free_commit=memory.free_commit,
                    commit_limit=memory.total_commit, free_disk=shutil.disk_usage(Path.cwd()).free)

    initial = budget()
    # The child job must fit in current physical/commit headroom, with a reserve.
    cap = limit_mib * 1024**2
    if min(initial['free_ram'], initial['free_commit']) < cap + 4 * 1024**3 or initial['free_disk'] < 12 * 1024**3:
        raise RuntimeError('Insufficient resources for the requested bounded build')
    job = kernel.CreateJobObjectW(None, None)
    if not job:
        raise C.WinError(C.get_last_error())
    child = None
    samples = []
    reason = None
    limits = Limits()
    limits.basic.flags = 0x2000 | 0x200  # KILL_ON_JOB_CLOSE | JOB_MEMORY
    limits.job_memory = cap
    try:
        if not kernel.SetInformationJobObject(job, 9, C.byref(limits), C.sizeof(limits)):
            raise C.WinError(C.get_last_error())
        with log.open('wb') as output:
            child = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT)
            if not kernel.AssignProcessToJobObject(job, W.HANDLE(int(child._handle))):
                error = C.get_last_error()
                child.kill()
                child.wait()
                raise C.WinError(error)
            started = time.monotonic()
            heartbeat = started - 30
            while child.poll() is None:
                current = budget()
                samples.append(current)
                if time.monotonic() - heartbeat >= 30:
                    heartbeat = time.monotonic()
                    snapshot = dict(event='bounded-build-running', elapsed_seconds=round(heartbeat-started),
                                    limit_mib=limit_mib, **current)
                    report.with_suffix('.running.json').write_text(json.dumps(snapshot, indent=2)+'\n', encoding='utf-8')
                    print(json.dumps(snapshot), flush=True)
                if (current['free_ram'] < 4 * 1024**3 or current['free_commit'] < 8 * 1024**3
                        or current['free_disk'] < 12 * 1024**3
                        or initial['free_disk'] - current['free_disk'] > 8 * 1024**3):
                    reason = 'Resource reserve or 8 GiB disk growth budget reached'
                    kernel.TerminateJobObject(job, 125)
                    break
                time.sleep(2)
            code = child.wait()
        if not kernel.QueryInformationJobObject(job, 9, C.byref(limits), C.sizeof(limits), None):
            raise C.WinError(C.get_last_error())
        result = dict(exit_code=code, reason=reason, limit_mib=limit_mib,
                      enforced_job_limit_bytes=limits.job_memory, job_flags=limits.basic.flags,
                      peak_job_accounting_bytes=limits.peak_job,
                      accounting_note='Windows job peak accounting may include a rejected allocation request',
                      before=initial, after=budget(), samples=samples)
        report.write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
        print(json.dumps({k:v for k,v in result.items() if k != 'samples'}, indent=2))
        return code
    finally:
        # Closing kills only processes assigned to this fresh unnamed job.
        kernel.CloseHandle(job)
        if child is not None and child.poll() is None:
            child.wait()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--limit-mib', type=int, default=8192)
    parser.add_argument('--log', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not command or args.limit_mib < 32:
        parser.error('A command and limit of at least 32 MiB are required')
    raise SystemExit(run(command, args.limit_mib, args.log, args.report))
