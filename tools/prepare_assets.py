#!/usr/bin/env python3
"""Package a locally supplied, authorized build. Never downloads game files."""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import shutil
import struct

ROOT = Path(__file__).resolve().parents[1]


def verify(path, entry):
    digest = hashlib.sha1(f"blob {entry['size']}\0".encode())
    with path.open('rb') as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    if path.stat().st_size != entry['size'] or digest.hexdigest() != entry['sha']:
        raise ValueError(f"Source differs from the reviewed build: {entry['path']}")


def replace_once(text, old, new):
    if text.count(old) != 1:
        raise ValueError(f"Framework patch anchor changed: {old[:80]}")
    return text.replace(old, new, 1)


def prepare(source, dest):
    lock = json.loads((ROOT / 'tools/source-lock.json').read_text())
    entries = [x for x in lock['files'] if x['path'].startswith(('Build/', 'StreamingAssets/')) or x['path'] == 'LICENSE']
    for entry in entries:
        verify(source / entry['path'], entry)
    dest.mkdir(parents=True, exist_ok=True)
    parts = sorted((source / 'Build').glob('*.part.*'))
    if len(parts) != 15:
        raise ValueError('Expected all 15 data parts')
    with gzip.open(parts[0], 'rb') as data:
        if data.read(16) != b'UnityWebData1.0\0':
            raise ValueError('Unsupported data container')
        end = struct.unpack('<I', data.read(4))[0]
        directory = []
        while data.tell() < end:
            offset, size, n = struct.unpack('<III', data.read(12))
            directory.append({'path': data.read(n).decode(), 'offset': offset, 'size': size})
    part_names = []
    for i, part in enumerate(parts):
        name = f'data.{i:03}.bin'
        shutil.copyfile(part, dest / name)
        part_names.append(name)
    wasm = next((source / 'Build').glob('*.wasm.unityweb'))
    with gzip.open(wasm, 'rb') as stream:
        if stream.read(8) != b'\0asm\1\0\0\0':
            raise ValueError('Invalid WASM header')
        wasm_size = 8
        while chunk := stream.read(1024 * 1024):
            wasm_size += len(chunk)
    # AAPT strips .gz and decompresses it, even with noCompress: use .bin.
    (dest / 'game.wasm.gz').unlink(missing_ok=True)
    shutil.copyfile(wasm, dest / 'game.wasm.bin')
    framework = gzip.decompress(next((source / 'Build').glob('*.js.unityweb')).read_bytes()).decode()
    # Release the source bytecode after instantiation. This does not change WASM code.
    framework = replace_once(framework, 'Module["usingWasm"]=true;removeRunDependency("wasm-instantiate")',
        'Module["usingWasm"]=true;Module["wasmBinary"]=null;removeRunDependency("wasm-instantiate")')
    # A touch-only virtual lock needs no browser user-gesture permission. Unity
    # requests it from its game loop after Deploy; deferring until another input
    # would leave the touch HUD hidden on spawn.
    framework = replace_once(framework,
        'function _emscripten_request_pointerlock(target,deferUntilInEventHandler){',
        'function _emscripten_request_pointerlock(target,deferUntilInEventHandler){if(Module.ravenTouch){Browser.pointerLock=true;Module.canvas.requestPointerLock();return 0;}')
    framework = replace_once(framework, 'function _emscripten_exit_pointerlock(){',
        'function _emscripten_exit_pointerlock(){if(Module.ravenTouch)Browser.pointerLock=false;')
    bridge = (ROOT / 'tools/framework-hooks.js').read_text()
    framework = replace_once(framework, 'Module["noExitRuntime"]=true;run()', bridge + '\nModule["noExitRuntime"]=true;run()')
    (dest / 'framework.js').write_text(framework)
    shutil.copyfile(next((source / 'Build').glob('*.js')), dest / 'loader.js')
    shutil.copytree(source / 'StreamingAssets', dest / 'StreamingAssets', dirs_exist_ok=True)
    shutil.copyfile(source / 'LICENSE', dest / 'UPSTREAM-LICENSE')
    manifest = {
        'repository': lock['repository'], 'commit': lock['commit'],
        'dataParts': part_names, 'dataSize': max(x['offset'] + x['size'] for x in directory),
        'wasmSize': wasm_size, 'directory': directory,
        'frameworkPatches': ['release-wasm-source', 'touch-pointer-lock', 'lifecycle-audio-diagnostics-hooks'],
    }
    (dest / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(f"Prepared {dest}: data={manifest['dataSize']} bytes, wasm={wasm_size} bytes")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path, help='Authorized local Ravenfield repository/build directory')
    parser.add_argument('--output', type=Path, default=ROOT / 'app/src/main/assets/game')
    args = parser.parse_args()
    prepare(args.source.resolve(), args.output.resolve())
