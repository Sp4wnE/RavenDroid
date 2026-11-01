#!/usr/bin/env python3
"""Restore only pinned native build inputs from the supplied, hash-verified Lab APK.

This does not install an APK, download files, accept arbitrary native artifacts,
or import the separate 1 GB Unity game data container.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
APK_NAME = 'ravenfield-native-lab-v0.4-debug.apk'
APK_SHA256 = '4660368bd87b6f88fa71c23499414cd7dddc58f4e264cb71c609d2f41fe3c68e'
WASM_SHA256 = '03c6c9516d803086adf7e57c2122b21988421cb1302d1736609cd4e44324cc0a'
PINNED = {
    'smoke': (198064, '24e47c81254c0236bf9c76b4ca86cb73547451481621e8f80c50e436557d3fb2'),
    'unity': (486597328, '37300a98c0f7f0765ddd634e83fe2e97b960ce2bb453134f1895b5dfd4f548d1'),
}


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as source:
        while block := source.read(1024 * 1024):
            result.update(block)
    return result.hexdigest()


def restore_entry(archive, entry, destination, expected_hash, expected_size):
    if archive.getinfo(entry).file_size != expected_size:
        raise ValueError(f'Unexpected packaged size: {entry}')
    if destination.is_file() and destination.stat().st_size == expected_size and digest(destination) == expected_hash:
        print(f'Already verified: {destination}')
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(prefix='handoff-', suffix='.partial', dir=destination.parent, delete=False) as output:
            temporary = Path(output.name)
            result = hashlib.sha256()
            size = 0
            with archive.open(entry) as source:
                while block := source.read(1024 * 1024):
                    size += len(block)
                    if size > expected_size:
                        raise ValueError(f'Payload exceeds size: {entry}')
                    output.write(block)
                    result.update(block)
            output.flush()
            os.fsync(output.fileno())
        if size != expected_size or result.hexdigest() != expected_hash:
            raise ValueError(f'Payload checksum mismatch: {entry}')
        temporary.replace(destination)
        print(f'Restored: {destination} ({size} bytes)')
    finally:
        if temporary is not None and temporary.exists():
            temporary.unlink()


def restore(apk, output_root):
    if digest(apk) != APK_SHA256:
        raise ValueError('APK SHA-256 differs from the pinned Lab 0.4 build; refusing native code extraction')
    with zipfile.ZipFile(apk) as archive:
        manifest_bytes = archive.read('assets/native/manifest.json')
        data_bytes = archive.read('assets/native/data-profile.json')
        manifest = json.loads(manifest_bytes)
        data = json.loads(data_bytes)
        if (manifest['runtime'] != 'wasmtime-49.0.0' or manifest['target'] != 'aarch64-linux-android'
                or manifest['wasmSha256'] != WASM_SHA256 or manifest['nativeUnwindInfo'] is not False
                or data['wasmSha256'] != WASM_SHA256):
            raise ValueError('Build profile does not match this native host')
        base = Path('native-android/app/src/main')
        for label, (size, sha) in PINNED.items():
            item = manifest['artifacts'][label]
            name = label + '.cwasm.bin'
            if item != {'file': name, 'bytes': size, 'sha256': sha}:
                raise ValueError(f'Unrecognized AOT artifact: {label}')
            restore_entry(archive, 'assets/native/' + name, output_root / base / 'assets/native' / name, sha, size)
        for name, content in [('manifest.json', manifest_bytes), ('data-profile.json', data_bytes)]:
            restore_entry(archive, 'assets/native/' + name, output_root / base / 'assets/native' / name,
                          hashlib.sha256(content).hexdigest(), len(content))
        name = 'lib/arm64-v8a/libravenfield_native_host.so'
        restore_entry(archive, name, output_root / base / 'jniLibs/arm64-v8a/libravenfield_native_host.so',
                      '1c5b5c893bfb0255423a68d69b986a6b865346246259a93005b22480030f0409', archive.getinfo(name).file_size)
    print('ARM64 assets restored. Rebuild the JNI library after Rust source changes.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apk', type=Path, default=ROOT.parent / 'binaries' / APK_NAME)
    parser.add_argument('--output-root', type=Path, default=ROOT)
    args = parser.parse_args()
    restore(args.apk.resolve(), args.output_root.resolve())
