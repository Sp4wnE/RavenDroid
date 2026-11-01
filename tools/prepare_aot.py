#!/usr/bin/env python3
"""Cross-compile an authorized local WASM; no downloads or phone-side compiler."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import struct

ROOT = Path(__file__).resolve().parents[1]


def verify_aot_artifact(path, machine=183):
    with path.open('rb') as stream:
        header = stream.read(64)
        if header[:6] != b'\x7fELF\x02\x01' or int.from_bytes(header[18:20], 'little') != machine:
            raise ValueError('AOT output does not match the requested little-endian ELF target')
        section_offset = struct.unpack_from('<Q', header, 40)[0]
        entry_size, count, strings_index = struct.unpack_from('<HHH', header, 58)
        if entry_size != 64 or not 0 < count < 4096 or strings_index >= count:
            raise ValueError('Unsupported AOT ELF section directory')
        stream.seek(section_offset)
        sections = [struct.unpack('<IIQQQQIIQQ', stream.read(64)) for _ in range(count)]
        strings_section = sections[strings_index]
        if strings_section[5] > 1024 * 1024:
            raise ValueError('AOT section strings exceed limit')
        stream.seek(strings_section[4])
        strings = stream.read(strings_section[5])
        for section in sections:
            name = strings[section[0]:].split(b'\0', 1)[0]
            if name == b'.eh_frame' and section[5] != 0:
                raise ValueError('Rebuild raven-aot with native_unwind_info(false) before packaging')


def verify_arm64_artifact(path):
    verify_aot_artifact(path, 183)


def digest(path):
    sha = hashlib.sha256()
    with path.open('rb') as stream:
        while chunk := stream.read(1024 * 1024):
            sha.update(chunk)
    return sha.hexdigest()


def prepare(wasm, compiler, output, opt_level='none', stack_global=False, leaf_invokes=False,
            leaf_tool=None, leaf_signatures=None, host_boundaries=False, lower_intrinsics=False):
    if opt_level not in ('none', 'speed'):
        raise ValueError('Unsupported optimization level')
    leaf_invokes = leaf_invokes or bool(leaf_signatures) or host_boundaries or lower_intrinsics
    profile = json.loads((ROOT / 'native-runtime/build-profile.json').read_text())
    if digest(wasm) != profile['wasmSha256']:
        raise ValueError('Local WASM differs from the reviewed build')
    output.mkdir(parents=True, exist_ok=True)
    manifest = {'format': 1, 'runtime': 'wasmtime-49.0.0', 'target': 'aarch64-linux-android',
                'wasmSha256': profile['wasmSha256'], 'fuel': True, 'optLevel': opt_level,
                'nativeUnwindInfo': False, 'artifacts': {}}
    unity_source=wasm
    if stack_global or leaf_invokes:
        from prepare_stack_export import transform
        unity_source=output / 'unity-stack-export.wasm'
        unity_source.write_bytes(transform(wasm.read_bytes()))
        manifest['stackGlobalExport']={'version':1,'transformedWasmSha256':digest(unity_source)}
    if leaf_invokes:
        if leaf_tool is None or not leaf_tool.is_file():
            raise ValueError('Build the local prepare-leaf-invokes example and supply --leaf-tool')
        destination=output / 'unity-leaf-invokes.wasm'
        proof=output / 'leaf-invoke-transform.json'
        command = [str(leaf_tool),str(unity_source),str(destination),str(proof)]
        if leaf_signatures:
            command += ['--signatures', leaf_signatures]
        if host_boundaries:
            command += ['--caught-invokes']
        if lower_intrinsics:
            command += ['--lower-intrinsics']
        subprocess.run(command,check=True)
        reviewed=json.loads(proof.read_text(encoding='utf-8'))
        if reviewed['source_sha256']!=digest(unity_source) or reviewed['transformed_sha256']!=digest(destination):
            raise ValueError('Leaf transform proof does not match local output')
        unity_source=destination
        manifest['stackGlobalExport']['transformedWasmSha256']=digest(unity_source)
        manifest['leafInvokeTransform']={'version':3 if lower_intrinsics else 2 if host_boundaries else 1,**reviewed}
    for label, source in [('smoke', ROOT / 'native-runtime/tests/smoke.wasm'), ('unity', unity_source)]:
        destination = output / f'{label}.cwasm.bin'
        partial = destination.with_suffix('.partial')
        subprocess.run([str(compiler), manifest['target'], str(source), str(partial), '--opt-level', opt_level], check=True)
        verify_arm64_artifact(partial)
        partial.replace(destination)
        manifest['artifacts'][label] = {'file': destination.name, 'bytes': destination.stat().st_size,
                                         'sha256': digest(destination)}
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps(manifest, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('wasm', type=Path)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'native-runtime/target/release/raven-aot')
    parser.add_argument('--output', type=Path, default=ROOT / 'native-android/app/src/main/assets/native')
    parser.add_argument('--opt-level', choices=('none', 'speed'), default='none')
    parser.add_argument('--stack-global', action='store_true', help='Expose the verified pure stackSave global')
    parser.add_argument('--leaf-invokes', action='store_true', help='Keep statically proven import-free targets inside WASM; implies stack-global')
    parser.add_argument('--leaf-tool',type=Path,help='Locally built prepare-leaf-invokes executable')
    parser.add_argument('--leaf-signatures',help='Optional comma-separated invoke signatures; implies leaf-invokes')
    parser.add_argument('--host-boundaries',action='store_true',help='Use reviewed nonthrowing host boundaries; implies leaf-invokes')
    parser.add_argument('--lower-intrinsics',action='store_true',help='Lower reviewed scalar/memcpy imports to guest instructions')
    args = parser.parse_args()
    prepare(args.wasm.resolve(), args.compiler.resolve(), args.output.resolve(), args.opt_level,
            args.stack_global,args.leaf_invokes,args.leaf_tool.resolve() if args.leaf_tool else None,
            args.leaf_signatures,args.host_boundaries,args.lower_intrinsics)
