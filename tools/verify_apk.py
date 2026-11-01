#!/usr/bin/env python3
"""Check packaged asset names/bytes and gzip WASM after AAPT processing."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def digest(stream):
    value = hashlib.sha256()
    while chunk := stream.read(1024 * 1024):
        value.update(chunk)
    return value.hexdigest()


def verify(apk):
    assets = ROOT / 'app/src/main/assets'
    verified = []
    with zipfile.ZipFile(apk) as package:
        for source in sorted(assets.rglob('*')):
            if not source.is_file():
                continue
            name = 'assets/' + str(source.relative_to(assets))
            with source.open('rb') as original, package.open(name) as packed:
                if digest(original) != digest(packed):
                    raise ValueError(f'Packaged asset differs: {name}')
            verified.append(name)
        manifest = json.loads(package.read('assets/game/manifest.json'))
        with package.open('assets/game/game.wasm.bin') as packed:
            with gzip.GzipFile(fileobj=packed) as wasm:
                if wasm.read(8) != b'\0asm\1\0\0\0':
                    raise ValueError('Packaged WASM header is invalid')
                length = 8
                while chunk := wasm.read(1024 * 1024):
                    length += len(chunk)
                if length != manifest['wasmSize']:
                    raise ValueError('Packaged WASM size differs')
        native = [name for name in package.namelist() if name.startswith('lib/')]
    with apk.open('rb') as stream:
        checksum = digest(stream)
    return {'apk': apk.name, 'bytes': apk.stat().st_size, 'sha256': checksum,
            'verifiedAssets': verified, 'decompressedWasmBytes': length,
            'nativeLibraries': native}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('apk', type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.apk), indent=2))
