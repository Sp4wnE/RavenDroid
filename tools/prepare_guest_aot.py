"""Package the reviewed guest-EH/atomic source and an already locally built AOT."""
import argparse
import json
from pathlib import Path
import subprocess
from prepare_aot import digest, verify_aot_artifact
from aot_profile import EPOCH_WATCHDOG

ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = ROOT.parent
SOURCE_SHA = '235a040613b5bcbc47a8a770a8bb9ee497c56631caad8007ff9ca5c8aa298914'
BASE_SHA = '97fb7db1805bb557c88bbd5d5e4d2f1e61982aad7b1c1e4b160bf4aa133ac71b'
REVIEWED_ARTIFACTS = {
    ('aarch64-linux-android', False, True): '3ca58bfa409ed181a3b17ccb90278b8c2a21488b5a85ce8f9abee649df3d6fee',
    ('x86_64-linux-android', False, False): '9bbc4e8fadae776d003dbea8e79d6492d116f2d8858e8ffe6ef16fbd949d306d',
    ('x86_64-linux-android', False, True): 'e4dcd134916ca348f0f30be76a9b3d586be32efaca5496514c430ec81b9644ee',
}


def install(source, destination, replace=False):
    if destination.exists():
        if digest(source) == digest(destination):
            return
        if not replace:
            raise ValueError('Existing generated payload differs: ' + str(destination))
    temporary = destination.with_suffix(destination.suffix + '.staging')
    if temporary.exists():
        temporary.unlink()
    temporary.hardlink_to(source.resolve())
    temporary.replace(destination)


def prepare(artifact, target, output, compiler, inline=False, epoch=False):
    expected = REVIEWED_ARTIFACTS.get((target, inline, epoch))
    if expected is None or digest(artifact) != expected:
        raise ValueError('No reviewed successful AOT for this artifact/target/codegen profile')
    source = WORKSPACE / '.tools/references/game-guest-atomics-018.wasm'
    if digest(source) != SOURCE_SHA:
        raise ValueError('Unexpected reviewed guest source')
    base = WORKSPACE / '.tools/references/aot-android-intrinsics-017/manifest.json'
    manifest = json.loads(base.read_text(encoding='utf-8'))
    if manifest['stackGlobalExport']['transformedWasmSha256'] != BASE_SHA:
        raise ValueError('Unexpected baseline transform chain')
    eh = json.loads((ROOT / 'artifacts/guest-eh-018-transform.json').read_text(encoding='utf-8'))
    atomic = json.loads((ROOT / 'artifacts/guest-atomics-018-transform.json').read_text(encoding='utf-8'))
    if (eh['source_sha256'] != BASE_SHA or atomic['source_sha256'] != eh['transformed_sha256']
            or atomic['transformed_sha256'] != SOURCE_SHA or eh['wrappers'] != 404
            or atomic['rewritten_calls'] != 1):
        raise ValueError('Guest transform proof chain mismatch')
    machine = 183 if target == 'aarch64-linux-android' else 62
    verify_aot_artifact(artifact, machine)
    output.mkdir(parents=True, exist_ok=True)
    profile = 'guest-eh-epoch-v1' if epoch else ('guest-eh-inline-eager-v1' if inline else 'guest-eh-v1')
    if (output / 'manifest.json').exists():
        previous = json.loads((output / 'manifest.json').read_text(encoding='utf-8'))
        if previous.get('engineProfile') != profile or previous.get('target') != target:
            raise ValueError('Use a separate cache for a different engine profile/target')
    install(artifact, output / 'unity.cwasm.bin')
    smoke = output / 'smoke.cwasm.bin'
    if not smoke.exists():
        partial = smoke.with_suffix('.partial')
        command = [str(compiler.resolve()), target, str(ROOT / 'native-runtime/tests/smoke.wasm'),
                   str(partial), '--opt-level', 'speed', '--guest-eh']
        if inline:
            command += ['--inline-eager-codegen']
        if epoch:
            command += ['--epoch-guard']
        subprocess.run(command, check=True)
        verify_aot_artifact(partial, machine)
        partial.replace(smoke)
    manifest.update(target=target, engineProfile=profile, optLevel='speed',
                    guestExceptionTransform=eh, guestAtomicTransform=atomic)
    manifest['fuel'] = not epoch
    if epoch:
        manifest['epochWatchdog'] = EPOCH_WATCHDOG
    manifest['stackGlobalExport']['transformedWasmSha256'] = SOURCE_SHA
    for label in ('unity', 'smoke'):
        path = output / (label + '.cwasm.bin')
        manifest['artifacts'][label] = dict(file=path.name, bytes=path.stat().st_size, sha256=digest(path))
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(dict(target=target, profile=profile, unity=manifest['artifacts']['unity']), indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('artifact', type=Path)
    parser.add_argument('--target', choices=('aarch64-linux-android', 'x86_64-linux-android'), required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'native-runtime/target/release/raven-aot.exe')
    parser.add_argument('--inline-eager-codegen', action='store_true')
    parser.add_argument('--epoch-guard', action='store_true')
    args = parser.parse_args()
    prepare(args.artifact.resolve(), args.target, args.output.resolve(), args.compiler.resolve(), args.inline_eager_codegen, args.epoch_guard)
