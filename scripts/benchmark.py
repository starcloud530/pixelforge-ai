#!/usr/bin/env python3
"""Repeatable CLI latency/RSS and player refresh measurements, not an art-quality score."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import threading
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]

def invoke(binary, *args, requests=None):
    return subprocess.run([str(binary), *map(str, args)], input=json.dumps(requests) if requests is not None else None,
                          text=True, encoding="utf-8", capture_output=True, check=True, timeout=120)

def measure(binary, args, timeout=60):
    with tempfile.TemporaryFile() as err:
        start = time.perf_counter()
        process = subprocess.Popen([str(binary), *map(str, args)], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=err)
        expired = threading.Event()
        def stop(): expired.set(); process.kill()
        timer = threading.Timer(timeout, stop)
        timer.start()
        rss = None
        try:
            if hasattr(os, 'wait4'):
                _, status, usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)
                rss = usage.ru_maxrss if platform.system() == 'Darwin' else usage.ru_maxrss * 1024
            else:
                process.wait()
        finally:
            timer.cancel()
        elapsed = (time.perf_counter() - start) * 1000
        err.seek(0)
        if expired.is_set() or process.returncode:
            raise RuntimeError(f'{args}: exit={process.returncode}, timeout={expired.is_set()}, {err.read().decode(errors="replace")}')
        return {'ms': round(elapsed, 3), 'peakRssBytes': rss}

def summary(name, samples, **extra):
    times = sorted(s['ms'] for s in samples)
    memory = [s['peakRssBytes'] for s in samples if s['peakRssBytes'] is not None]
    return {'name': name, 'samples': samples, 'medianMs': statistics.median(times),
            'p95Ms': times[math.ceil(len(times) * .95) - 1], 'peakRssBytes': max(memory) if memory else None, **extra}

def make_clip(binary, root, edge, count):
    file = root / f'clip-{edge}-{count}.json'
    invoke(binary, 'new', '--width', edge, '--height', edge, '--output', file)
    commands = [{'tool': 'fillRect', 'params': {'x': i * edge // 8, 'y': 0, 'width': edge // 8, 'height': edge, 'colorIndex': i + 1}} for i in range(8)]
    commands += [{'tool': 'duplicateFrame', 'params': {'index': i}} for i in range(count - 1)]
    commands += [{'tool': 'addTag', 'params': {'name': 'load', 'from': 0, 'to': count - 1, 'loop': 'forward'}}]
    invoke(binary, 'call', '--project', file, requests=commands)
    return file

def player(binary, project, repeats):
    server = subprocess.Popen([str(binary), 'play', '--project', str(project), '--no-open'], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, encoding="utf-8")
    timer = threading.Timer(30, server.kill)
    timer.start()
    try:
        start = time.perf_counter()
        line = server.stdout.readline()
        url = json.loads(line)['result']['url'] + 'snapshot'
        def snapshot():
            with urllib.request.urlopen(url, timeout=5) as response: return json.load(response)
        original = snapshot()
        startup = (time.perf_counter() - start) * 1000
        samples = []
        for i in range(repeats):
            start = time.perf_counter()
            invoke(binary, 'paint', '--project', project, '--x', 0, '--y', 0, '--color-index', 2 if i % 2 == 0 else 1)
            deadline = time.monotonic() + 10
            while snapshot()['revision'] == original['revision']:
                if time.monotonic() > deadline: raise TimeoutError('player did not refresh')
                time.sleep(.01)
            original = snapshot()
            samples.append({'ms': round((time.perf_counter() - start) * 1000, 3), 'peakRssBytes': None})
        return {'startupMs': round(startup, 3), **summary('player-edit-to-snapshot', samples)}
    finally:
        timer.cancel(); server.kill(); server.wait(); server.stdout.close()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--no-build', action='store_true')
    parser.add_argument('--source', type=Path, help='optional real input PNG; never modified')
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--output', type=Path, help='write measurements as JSON')
    args = parser.parse_args()
    if not 1 <= args.repeats <= 20: parser.error('--repeats must be 1..20')
    if not args.no_build: subprocess.run(['cargo', 'build', '--release', '-p', 'pixelforge-cli', '--locked'], cwd=ROOT, check=True)
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
    if not target.is_absolute(): target = ROOT / target
    binary = Path(os.environ.get('PIXELFORGE_BIN', target / 'release' / ('pixelforge.exe' if os.name == 'nt' else 'pixelforge'))).resolve()
    results = {'platform': platform.platform(), 'cliVersion': invoke(binary, '--version').stdout.strip(),
               'repeats': args.repeats, 'rssNote': 'per-child peak RSS; null where wait4 is unavailable; player refresh has no process RSS sample',
               'fixtureNote': 'CLI-generated color bars and duplicated frames measure load, not animation quality', 'complete': False, 'cases': []}
    def persist():
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(results, indent=2), encoding='utf8')
    persist()
    with tempfile.TemporaryDirectory(prefix='pixelforge-bench-') as temp:
        root = Path(temp)
        for edge, count in [(32, 8), (64, 16), (256, 32)]:
            project = make_clip(binary, root, edge, count)
            for name, options in [('scene', ['scene']), ('preview', ['preview', '--output', root / 'preview.png']),
                                  ('sheet', ['export', 'sheet', '--output', root / 'sheet.png', '--cols', 8]),
                                  ('apng', ['export', 'apng', '--tag', 'load', '--output', root / 'clip.png'])]:
                command = [*options, '--project', project]
                measure(binary, command)  # warm-up, excluded
                row = summary(f'{name}-{edge}x{edge}-{count}frames', [measure(binary, command) for _ in range(args.repeats)], projectBytes=project.stat().st_size)
                results['cases'].append(row); persist(); print(json.dumps(row), flush=True)
            if edge == 64:
                results['player'] = player(binary, project, args.repeats)
                persist()
        if args.source:
            source = args.source.resolve()
            results['source'] = {'name': source.name, 'bytes': source.stat().st_size, 'sha256': hashlib.sha256(source.read_bytes()).hexdigest()}
            for engine in ['center-sample', 'dominant', 'kmeans', 'contrast-outline', 'grid-snap']:
                samples = []
                for i in range(args.repeats):
                    samples.append(measure(binary, ['convert', '--file', source, '--output', root / f'{engine}-{i}.json', '--width', 64, '--height', 64, '--engine', engine, '--palette', 'source', '--max-colors', 16]))
                row = summary(f'convert-{engine}-to64x64', samples)
                results['cases'].append(row); persist(); print(json.dumps(row), flush=True)
    results['complete'] = True
    persist()
    print(json.dumps({'player': results.get('player'), 'cases': len(results['cases'])}))

if __name__ == '__main__': main()
