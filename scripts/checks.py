#!/usr/bin/env python3
"""Project checks. Every subprocess failure is a failure; no missing-tool skips."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
if not __debug__:
    raise RuntimeError("Run checks without Python -O; assertions are required")

def command(args, **kwargs):
    print('+ ' + ' '.join(map(str, args)), flush=True)
    return subprocess.run(list(map(str, args)), cwd=ROOT, check=True, **kwargs)

def exercise(binary, work, smoke, artifacts):
    records = []
    def run(*args, code=0, error=None):
        result = subprocess.run([str(binary), *map(str, args)], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=30)
        records.append({'argv': list(map(str, args)), 'expectedExit': code, 'exit': result.returncode,
                        'stdout': result.stdout, 'stderr': result.stderr})
        if artifacts:
            (artifacts / 'commands.json').write_text(json.dumps(records, indent=2), encoding='utf8')
        assert result.returncode == code, records[-1]
        bodies = [json.loads(line) for line in result.stdout.splitlines() if line]
        assert bodies, f'CLI returned no structured response: {args}'
        if error:
            assert bodies[-1]['error']['code'] == error, records[-1]
        else:
            assert all(body.get('ok') is True for body in bodies), records[-1]
        return bodies[-1] if bodies else None
    def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
    def size(path):
        data = path.read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n'
        return struct.unpack('>II', data[16:24])

    run('tools')
    assert run('skills', 'list')['count'] == 6
    run('skills', 'read', 'pixel-art-craft/references/action-motion.md', '--json')
    assert 'center-sample' in json.dumps(run('engine', 'list'))
    project, checkpoint = work / 'hero.json', work / 'hero.ckpt.json'
    run('new', '--width', 16, '--height', 16, '--output', project)
    before_revision = run('scene', '--project', project)['revision']
    run('paint', '--project', project, '--checkpoint', checkpoint, '--expected-revision', before_revision,
        '--x', 2, '--y', 3, '--color-index', 1)
    after_revision = run('inspect', '--project', project)['revision']
    assert after_revision != before_revision
    if smoke:
        before = digest(project), digest(checkpoint)
        run('paint', '--project', project, '--checkpoint', checkpoint, '--expected-revision', before_revision,
            '--x', 4, '--y', 3, '--color-index', 1, code=2, error='RevisionConflict')
        assert before == (digest(project), digest(checkpoint))
        run('paint', '--project', project, '--x', 2, '--y', 3, '--color-index', 0, code=1, error='PaletteIndex')
        assert before[0] == digest(project)
        run('preview', '--project', project, '--output', project, code=2, error='OutputConflict')
        assert before[0] == digest(project)
        run('undo', '--project', project, '--checkpoint', checkpoint)
        assert run('scene', '--project', project)['revision'] == before_revision
        run('redo', '--project', project, '--checkpoint', checkpoint)
        assert run('scene', '--project', project)['revision'] == after_revision
        run('frame', 'duplicate', '--project', project, '--index', 0)
        run('tag', 'add', '--project', project, '--name', 'idle_down', '--from', 0, '--to', 1)
        run('preview', '--project', project, '--scale', 2, '--output', work / 'preview.png')
        assert size(work / 'preview.png') == (32, 32)
        run('export', 'png', '--project', project, '--out-dir', work / 'frames')
        assert size(work / 'frames/pixelforge-000.png') == (16, 16)
        run('export', 'sheet', '--project', project, '--cols', 2, '--output', work / 'sheet.png', '--atlas', work / 'sheet.json')
        assert size(work / 'sheet.png') == (32, 16)
        run('validate-atlas', '--atlas', work / 'sheet.json', '--loose')
        run('export', 'apng', '--project', project, '--tag', 'idle_down', '--output', work / 'idle.png')
        assert b'acTL' in (work / 'idle.png').read_bytes()
        run('convert', '--file', work / 'frames/pixelforge-000.png', '--width', 16, '--height', 16,
            '--palette-strategy', 'source', '--output', work / 'converted.json')
        converted = run('inspect', '--project', work / 'converted.json')
        assert converted['result']['pixels'][3 * 16 + 2] != 0
        before = digest(project)
        run('convert', '--file', work / 'frames/pixelforge-000.png', '--project', project,
            '--engine', 'pixelforge-native', code=1, error='EngineUnavailable')
        assert digest(project) == before
    print(f"PASS: {'smoke' if smoke else 'quick'}, {len(records)} CLI invocations", flush=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('check', choices=['lint', 'quick', 'smoke'])
    parser.add_argument('--no-build', action='store_true', help='require an already-built CLI (CI)')
    parser.add_argument('--artifacts', type=Path, help='retain CLI evidence under this directory')
    args = parser.parse_args()
    if args.check == 'lint':
        command(['cargo', 'fmt', '--all', '--', '--check'])
        command(['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings'])
        command(['pnpm', 'typecheck'])
        command(['pnpm', 'typecheck:landing'])
        command([sys.executable, 'scripts/check_docs.py'])
        return
    if not args.no_build:
        command(['cargo', 'build', '-p', 'pixelforge-cli', '--locked'])
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
    if not target.is_absolute(): target = ROOT / target
    binary = Path(os.environ.get('PIXELFORGE_BIN', target / 'debug' / ('pixelforge.exe' if os.name == 'nt' else 'pixelforge'))).resolve()
    assert binary.is_file(), f'CLI missing: {binary}; build it or omit --no-build'
    artifacts = args.artifacts.resolve() if args.artifacts else None
    if artifacts: artifacts.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='pixelforge-check-') as temp:
        exercise(binary, Path(temp), args.check == 'smoke', artifacts)
    command(['node', '--test', 'crates/pixelforge-cli/src/player/playback.test.js'])
    command([sys.executable, 'scripts/check_docs.py'])
    if args.check == 'smoke':
        env = {**os.environ, 'PIXELFORGE': str(binary)}
        for crate, fixture in [('pixelforge-cli', 'base-tiles'), ('pixelforge-cli', 'game-pack-zombie'), ('pixelforge-kernel', 'game-pack-zombie')]:
            command([sys.executable, f'crates/{crate}/tests/fixtures/{fixture}/validate.py'], env=env)

if __name__ == '__main__': main()
