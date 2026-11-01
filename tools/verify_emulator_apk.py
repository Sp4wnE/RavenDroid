"""Verify the isolated x86_64 debug APK; not a replacement for ARM64 release checks."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import zipfile
from aot_profile import verify_library

ROOT = Path(__file__).resolve().parents[2] / '.tools/emulator-app'


def verify(apk):
    with zipfile.ZipFile(apk) as archive:
        names = archive.namelist()
        assert len(names) == len(set(names)), 'Duplicate ZIP entry'
        assert sorted(n for n in names if n.startswith('lib/')) == [
            'lib/x86_64/libravenfield_native_host.so', 'lib/x86_64/wrap.sh'], 'Unexpected ABI/library'
        for name in ('manifest.json', 'data-profile.json'):
            assert archive.read('assets/native/' + name) == (ROOT / 'assets/native' / name).read_bytes()
        manifest = json.loads(archive.read('assets/native/manifest.json'))
        assert manifest['target'] == 'x86_64-linux-android' and manifest['runtime'] == 'wasmtime-49.0.0'
        assert manifest['nativeUnwindInfo'] is False
        for label in ('unity', 'smoke'):
            entry = manifest['artifacts'][label]
            assert entry['file'] == label + '.cwasm.bin'
            name = 'assets/native/' + entry['file']
            assert archive.getinfo(name).file_size == entry['bytes']
            with archive.open(name) as stream:
                assert hashlib.file_digest(stream, 'sha256').hexdigest() == entry['sha256']
        library = archive.read('lib/x86_64/libravenfield_native_host.so')
        engine_profile = verify_library(library, manifest)
        assert library[:6] == b'\x7fELF\x02\x01' and struct.unpack_from('<HH', library, 16) == (3, 62)
        assert library == (ROOT / 'jniLibs/x86_64/libravenfield_native_host.so').read_bytes()
        wrapper = archive.read('lib/x86_64/wrap.sh')
        assert wrapper == (ROOT / 'resources/lib/x86_64/wrap.sh').read_bytes() and b'\r' not in wrapper
    with apk.open('rb') as stream:
        sha256 = hashlib.file_digest(stream, 'sha256').hexdigest()
    return {'apk': str(apk.resolve()), 'bytes': apk.stat().st_size, 'sha256': sha256,
            'engine_profile': engine_profile, 'jni_profile_matches_aot': True,
            'target': manifest['target'], 'aot_sha256': manifest['artifacts']['unity']['sha256'],
            'jni_sha256': hashlib.sha256(library).hexdigest(), 'jni_matches_local': True,
            'driver_wrapper_test_only': True, 'signature_checked_by_this_tool': False,
            'physical_device_tested': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('apk', type=Path)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    result = json.dumps(verify(args.apk), indent=2) + '\n'
    if args.report:
        args.report.write_text(result, encoding='utf-8')
    print(result)
