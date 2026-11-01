"""Read actual SurfaceFlinger presentation timestamps for RavenDroid's test layer only.

Column definition: AOSP services/surfaceflinger/FrameTracker.cpp::dumpStats.
No global settings/statistics are changed. Unsupported/empty data fails closed.
"""
import argparse
import json
import math
from pathlib import Path
import shlex
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
PACKAGE = 'dev.ravenfield.nativehost.emutest'


def parse_latency(text):
    times = set()
    for line in text.splitlines():
        values = line.split()
        if len(values) == 3 and all(v.isdigit() for v in values):
            desired, actual, ready = map(int, values)
            if desired > 0 and ready > 0 and 0 < actual < 2**63-1:
                times.add(actual)
    return times


def measure(adb, seconds):
    def shell(*args):
        return subprocess.run([str(adb), '-s', 'emulator-5580', 'shell', shlex.join(args)],
                              check=True, capture_output=True, text=True, timeout=10).stdout
    layers = [line for line in shell('dumpsys', 'SurfaceFlinger', '--list').splitlines()
              if PACKAGE in line and 'GameActivity' in line and 'SurfaceView' in line]
    # Android 15 lists RequestedLayerState{NAME parentId=...}; --latency
    # still expects NAME, including its #id, not the surrounding debug text.
    layers = [line.removeprefix('RequestedLayerState{').split(' parentId=',1)[0].rstrip('}')
              for line in layers]
    layers = sorted((line for line in layers if not line.startswith('Background ')),
                    key=lambda line: '(BLAST)' not in line)
    layer = None
    initial = set()
    for candidate in layers:
        rows = parse_latency(shell('dumpsys', 'SurfaceFlinger', '--latency', candidate))
        if rows:
            layer, initial = candidate, rows
            break
    if not layer:
        return dict(supported=False, reason='No RavenDroid SurfaceView with valid actual-present timestamps',
                    matching_layers=layers, stable_60_candidate=False)
    cutoff = max(initial)
    samples = set()
    start = time.monotonic()
    heartbeat = -10
    while time.monotonic()-start < seconds:
        rows = parse_latency(shell('dumpsys', 'SurfaceFlinger', '--latency', layer))
        samples.update(t for t in rows if t > cutoff)
        elapsed = time.monotonic()-start
        if elapsed-heartbeat >= 10:
            heartbeat = elapsed
            print(json.dumps(dict(elapsed_seconds=round(elapsed), unique_presentations=len(samples))), flush=True)
        time.sleep(0.8)
    ordered = sorted(samples)
    if len(ordered) < 2:
        return dict(supported=False, reason='Surface timestamps did not advance', layer=layer,
                    stable_60_candidate=False)
    intervals = [b-a for a,b in zip(ordered,ordered[1:])]
    ranked = sorted(intervals)
    percentile = lambda p: ranked[math.ceil(len(ranked)*p)-1]/1e6
    fps = len(intervals)*1e9/sum(intervals)
    windows = [120*1e9/sum(intervals[i:i+120]) for i in range(0,len(intervals)-119,120)]
    duration = (ordered[-1]-ordered[0])/1e9
    minimum = min(windows) if windows else 0
    return dict(supported=True, layer=layer, source='SurfaceFlinger actualPresentTime (column 2)',
                frames=len(ordered), measured_seconds=duration, fps=fps,
                slowest_120_interval_fps=minimum, interval_p95_ms=percentile(.95),
                interval_p99_ms=percentile(.99), interval_max_ms=max(intervals)/1e6,
                stable_60_candidate=duration>=60 and fps>=59.5 and minimum>=59 and percentile(.99)<=20,
                timestamps_ns=ordered, gameplay_visual_review_required=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--adb', type=Path, default=ROOT/'.tools/android-sdk/platform-tools/adb.exe')
    parser.add_argument('--seconds', type=int, default=65)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    if not 5 <= args.seconds <= 300:
        parser.error('Use 5..300 seconds (stable target requires at least 60 measured seconds)')
    result = measure(args.adb, args.seconds)
    args.report.write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print(json.dumps({k:v for k,v in result.items() if k != 'timestamps_ns'}, indent=2))
