"""Stage a strictly separate x86_64 debug APK from locally produced 0.17 AOT."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
from aot_profile import verify_library
from prepare_guest_aot import install

ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = ROOT.parent


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def prepare(artifact, compiler, reviewed=None, replace=False, library=None):
    original = ROOT / 'native-android/app/src/main/assets'
    if reviewed:
        manifest = json.loads((reviewed / 'manifest.json').read_text(encoding='utf-8'))
        if (manifest['target'] != 'x86_64-linux-android'
                or manifest['artifacts']['unity']['sha256'] != digest(artifact)):
            raise ValueError('Reviewed emulator cache/target mismatch')
    else:
        manifest = json.loads((original / 'native/manifest.json').read_text(encoding='utf-8'))
        source = WORKSPACE / '.tools/references/game-intrinsics-017.wasm'
        if digest(source) != manifest['stackGlobalExport']['transformedWasmSha256']:
            raise ValueError('Emulator source does not match the phone build')
    library = library or ROOT / 'native-runtime/target-android/x86_64-linux-android/release/libravenfield_native_host.so'
    verify_library(library.read_bytes(), manifest)
    with artifact.open('rb') as stream:
        header = stream.read(64)
    if header[:6] != b'\x7fELF\x02\x01' or struct.unpack_from('<H', header, 18)[0] != 62:
        raise ValueError('Expected locally built x86_64 AOT ELF')
    target = WORKSPACE / '.tools/emulator-app'
    assets = target / 'assets/native'
    assets.mkdir(parents=True, exist_ok=True)
    unity = assets / 'unity.cwasm.bin'
    install(artifact, unity, replace)
    smoke = assets / 'smoke.cwasm.bin'
    if reviewed:
        source = reviewed / 'smoke.cwasm.bin'
        if digest(source) != manifest['artifacts']['smoke']['sha256']:
            raise ValueError('Reviewed smoke cache checksum mismatch')
        install(source, smoke, replace)
    elif not smoke.exists():
        subprocess.run([str(compiler.resolve()), 'x86_64-linux-android',
                        str(ROOT / 'native-runtime/tests/smoke.wasm'), str(smoke),
                        '--opt-level', 'speed'], check=True)
    manifest['target'] = 'x86_64-linux-android'
    for label in ('unity', 'smoke'):
        path = assets / (label + '.cwasm.bin')
        manifest['artifacts'][label] = {'file': path.name, 'bytes': path.stat().st_size, 'sha256': digest(path)}
    (assets / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    shutil.copy2(original / 'native/data-profile.json', assets / 'data-profile.json')
    for name in ('THIRD-PARTY-NOTICES.txt', 'third-party-audio.txt'):
        shutil.copy2(original / name, target / 'assets' / name)
    jni = target / 'jniLibs/x86_64'
    jni.mkdir(parents=True, exist_ok=True)
    shutil.copy2(library, jni / 'libravenfield_native_host.so')
    resources = target / 'resources/lib/x86_64'
    resources.mkdir(parents=True, exist_ok=True)
    # Debug test process only; no global Android driver/security setting changes.
    (resources / 'wrap.sh').write_text(
        '#!/system/bin/sh\nexport LD_PRELOAD=/system/lib64/libEGL_angle.so:/system/lib64/libGLESv2_angle.so\nexec "$@"\n', encoding='utf-8', newline='\n')
    print(json.dumps({'target': manifest['target'], 'unity': manifest['artifacts']['unity'],
                      'jni_sha256': digest(library), 'root': str(target)}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('artifact', type=Path)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'native-runtime/target/release/raven-aot.exe')
    parser.add_argument('--reviewed-cache', type=Path)
    parser.add_argument('--replace-generated', action='store_true')
    parser.add_argument('--library', type=Path, help='Reuse an already built JNI library')
    args = parser.parse_args()
    prepare(args.artifact, args.compiler, args.reviewed_cache, args.replace_generated, args.library)
