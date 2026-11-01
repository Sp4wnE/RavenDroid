"""Summarize complete gameplay windows; never treat CLI offscreen FPS as Surface proof."""
import argparse
import json
from pathlib import Path


def summarize(report, first, last):
    windows = [w for w in report['performance']['retained_windows']
               if w['first_frame'] >= first and w['last_frame'] <= last]
    if not windows:
        raise ValueError('No complete windows in the requested gameplay range')
    frames = sum(w['frames'] for w in windows)
    elapsed = sum(w['elapsed_ms'] for w in windows)
    rates = [w['frames']*1000/w['elapsed_ms'] for w in windows]
    work_p95 = max(w['work_p95_ms'] for w in windows)
    total_p99 = [w.get('elapsed_p99_ms') for w in windows]
    p99 = max(total_p99) if all(v is not None for v in total_p99) else None
    errors = bool(report['first_blocker'] or report['graphics']['pending_gl_error']
                  or report['graphics']['shader_program_failures'] or report['audio']['errors']
                  or report['audio']['decode_failures'] or report['audio']['device_error'])
    budget = (not errors and elapsed >= 60000 and min(rates) >= 59.0
              and frames*1000/elapsed >= 59.5 and work_p95 <= 16.667
              and p99 is not None and p99 <= 20.0)
    live = report.get('live') or {}
    surface = live.get('presented_frames', 0) > 0
    return dict(first_frame=windows[0]['first_frame'], last_frame=windows[-1]['last_frame'],
                frames=frames, elapsed_ms=elapsed, fps=frames*1000/elapsed,
                slowest_window_fps=min(rates), fastest_window_fps=max(rates),
                worst_window_work_p95_ms=work_p95, worst_window_elapsed_p99_ms=p99,
                draw_calls=sum(w['draw_calls'] for w in windows),
                fuel_consumed=sum(w['fuel_consumed'] for w in windows),
                errors_present=errors, audio_underruns=report['audio']['underruns'],
                timing_budget_met=budget, real_surface=surface,
                stable_60_surface_candidate=budget and surface,
                caveat='Requires visual gameplay review; AI/time/load can differ. Not proof for every scene/device.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('report', type=Path)
    parser.add_argument('--first', type=int, default=3001)
    parser.add_argument('--last', type=int, default=7200)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = summarize(json.loads(args.report.read_text(encoding='utf-8-sig')), args.first, args.last)
    text = json.dumps(result, indent=2)+'\n'
    if args.output:
        args.output.write_text(text, encoding='utf-8')
    print(text)
