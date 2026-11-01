#!/usr/bin/env python3
"""Verify local ARM64 APK payloads and alignment; signature is a separate SDK check."""
import argparse
import hashlib
import gzip
import io
import json
from pathlib import Path
import struct
import zipfile
from aot_profile import verify_library

ROOT = Path(__file__).resolve().parents[1]


class ArchiveParts(io.RawIOBase):
    def __init__(self, archive, names):
        self.archive, self.names, self.index, self.stream = archive, names, 0, None

    def readable(self):
        return True

    def readinto(self, buffer):
        if not len(buffer):
            return 0
        while self.index < len(self.names):
            if self.stream is None:
                self.stream = self.archive.open(self.names[self.index])
            count = self.stream.readinto(buffer)
            if count:
                return count
            self.stream.close()
            self.stream = None
            self.index += 1
        return 0

    def close(self):
        if self.stream is not None:
            self.stream.close()
            self.stream = None
        super().close()


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify(apk, jni, bundled=False):
    assets = ROOT / 'native-android/app/src/main/assets/native'
    expected_manifest = (assets / 'manifest.json').read_bytes()
    manifest = json.loads(expected_manifest)
    if (manifest['target'] != 'aarch64-linux-android'
            or manifest['runtime'] != 'wasmtime-49.0.0'
            or manifest['nativeUnwindInfo'] is not False):
        raise ValueError('Unexpected local AOT runtime profile')
    with zipfile.ZipFile(apk) as archive, apk.open('rb') as raw:
        names = archive.namelist()
        if len(set(names)) != len(names):
            raise ValueError('Duplicate ZIP entry')
        if archive.read('assets/native/manifest.json') != expected_manifest:
            raise ValueError('Packaged AOT manifest differs from local build inputs')
        if archive.read('assets/native/data-profile.json') != (assets / 'data-profile.json').read_bytes():
            raise ValueError('Packaged data profile differs from local build inputs')
        bundled_report = None
        bundle_entries = sorted(name for name in names if name.startswith('assets/bundled/'))
        if bundled:
            data_profile = json.loads(archive.read('assets/native/data-profile.json'))
            if archive.getinfo('assets/bundled/manifest.json').file_size > 65536:
                raise ValueError('Bundled manifest exceeds bounds')
            info = json.loads(archive.read('assets/bundled/manifest.json'))
            parts = info['parts']
            if (info['format'] != 1 or not 1 <= len(parts) <= 64
                    or info['dataBytes'] != data_profile['dataBytes']
                    or info['dataSha256'] != data_profile['dataSha256']):
                raise ValueError('Bundled data metadata/compression mismatch')
            part_names = ['assets/bundled/game.data.%03d.chunk' % i for i in range(len(parts))]
            if bundle_entries != sorted(part_names + ['assets/bundled/manifest.json']):
                raise ValueError('Missing or unexpected bundled game assets')
            compressed_bytes, compressed_hash = 0, hashlib.sha256()
            for index, part in enumerate(parts):
                payload = archive.getinfo(part_names[index])
                if (part['file'] != 'game.data.%03d.chunk' % index or payload.file_size != part['bytes']
                        or not 1 <= part['bytes'] <= 64 * 1024 * 1024 or payload.compress_type != zipfile.ZIP_STORED):
                    raise ValueError('Bundled part metadata/compression mismatch')
                checksum = hashlib.sha256()
                with archive.open(payload) as stream:
                    while chunk := stream.read(1024 * 1024):
                        checksum.update(chunk);compressed_hash.update(chunk);compressed_bytes += len(chunk)
                if checksum.hexdigest() != part['sha256']:
                    raise ValueError('Bundled part SHA-256 mismatch')
            if compressed_bytes != info['compressedBytes'] or compressed_hash.hexdigest() != info['compressedSha256']:
                raise ValueError('Bundled compressed stream mismatch')
            checksum, length = hashlib.sha256(), 0
            with io.BufferedReader(ArchiveParts(archive, part_names)) as stream, gzip.GzipFile(fileobj=stream) as decoded:
                while chunk := decoded.read(1024 * 1024):
                    length += len(chunk)
                    if length > info['dataBytes']:
                        raise ValueError('Bundled data exceeds reviewed size')
                    checksum.update(chunk)
            if length != info['dataBytes'] or checksum.hexdigest() != info['dataSha256']:
                raise ValueError('Decoded bundled game data mismatch')
            bundled_report = dict(data_bytes=length, data_sha256=info['dataSha256'],
                                  compressed_bytes=compressed_bytes, part_count=len(parts), compressed_asset_stored=True,
                                  requires_external_game_file=False)
        elif bundle_entries:
            raise ValueError('Use --bundled to verify the full game APK')
        for label in ('smoke', 'unity'):
            item = manifest['artifacts'][label]
            if item['file'] != label + '.cwasm.bin':
                raise ValueError('Unexpected AOT filename')
            name = 'assets/native/' + item['file']
            with archive.open(name) as stream:
                actual = hashlib.file_digest(stream, 'sha256').hexdigest()
            if actual != item['sha256'] or archive.getinfo(name).file_size != item['bytes']:
                raise ValueError('Packaged AOT size/checksum mismatch: ' + label)
        library = 'lib/arm64-v8a/libravenfield_native_host.so'
        if sorted(name for name in names if name.startswith('lib/')) != [library]:
            raise ValueError('Unexpected native ABI/library set')
        payload = archive.read(library)
        engine_profile = verify_library(payload, manifest)
        jni_hash = hashlib.sha256(payload).hexdigest()
        if jni_hash != digest(jni):
            raise ValueError('Packaged JNI differs from the locally built library')
        if (payload[:6] != b'\x7fELF\x02\x01'
                or struct.unpack_from('<HH', payload, 16) != (3, 183)):
            raise ValueError('Expected an ARM64 little-endian ELF shared library')
        phoff = struct.unpack_from('<Q', payload, 32)[0]
        stride, count = struct.unpack_from('<HH', payload, 54)
        if stride < 56 or phoff + stride * count > len(payload):
            raise ValueError('Invalid ELF program header range')
        loads = []
        for index in range(count):
            entry = phoff + index * stride
            if struct.unpack_from('<I', payload, entry)[0] == 1:
                offset, address = struct.unpack_from('<QQ', payload, entry + 8)
                alignment = struct.unpack_from('<Q', payload, entry + 48)[0]
                if alignment < 16384 or alignment & (alignment - 1) or (address - offset) % 16384:
                    raise ValueError('ELF LOAD segment does not support 16 KiB pages')
                loads.append(alignment)
        if not loads:
            raise ValueError('ELF has no LOAD segments')
        info = archive.getinfo(library)
        raw.seek(info.header_offset)
        header = raw.read(30)
        if header[:4] != b'PK\x03\x04':
            raise ValueError('Invalid ZIP local header')
        name_size, extra_size = struct.unpack_from('<HH', header, 26)
        data_offset = info.header_offset + 30 + name_size + extra_size
        if info.compress_type != zipfile.ZIP_STORED or data_offset % 16384:
            raise ValueError('JNI must be uncompressed and ZIP-aligned to 16 KiB')
    return {'apk': str(apk.resolve()), 'bytes': apk.stat().st_size,
            'engine_profile': engine_profile, 'jni_profile_matches_aot': True,
            'sha256': digest(apk), 'jni_sha256': jni_hash,
            'aot_hashes_verified': True, 'jni_matches_local_build': True,
            'elf_load_alignments': loads, 'jni_zip_data_offset': data_offset,
            'signature_checked_by_this_tool': False,
            'physical_device_tested': False, 'rendered_game_frame': False,
            'bundled_game_data': bundled_report}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('apk', type=Path)
    parser.add_argument('--jni', type=Path, default=ROOT / 'native-android/app/src/main/jniLibs/arm64-v8a/libravenfield_native_host.so')
    parser.add_argument('--report', type=Path)
    parser.add_argument('--bundled', action='store_true', help='Also stream-verify every bundled game byte')
    args = parser.parse_args()
    report = json.dumps(verify(args.apk, args.jni, args.bundled), indent=2) + '\n'
    if args.report:
        args.report.write_text(report, encoding='utf-8')
    print(report)
