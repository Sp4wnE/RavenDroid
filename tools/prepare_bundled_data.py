"""Stream verified local Unity data into the optional offline APK asset."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def split_payload(payload, info, manifest):
    parts = []
    with payload.open('rb') as stream:
        while chunk := stream.read(64 * 1024 * 1024):
            name = 'game.data.%03d.chunk' % len(parts)
            target = payload.parent / name
            temporary = target.with_suffix('.chunk.partial')
            temporary.write_bytes(chunk)
            temporary.replace(target)
            parts.append(dict(file=name, bytes=len(chunk), sha256=hashlib.sha256(chunk).hexdigest()))
    info.pop('file', None)
    info['parts'] = parts
    temporary = manifest.with_suffix('.json.partial')
    temporary.write_text(json.dumps(info, indent=2) + '\n', encoding='utf-8')
    temporary.replace(manifest)
    payload.unlink()
    print(json.dumps(info), flush=True)


def prepare(source, output):
    profile = json.loads((ROOT / 'native-android/app/src/main/assets/native/data-profile.json').read_text())
    expected_size, expected_hash = profile['dataBytes'], profile['dataSha256']
    if source.stat().st_size != expected_size:
        raise ValueError('Local game data size differs from the reviewed profile')
    output.mkdir(parents=True, exist_ok=True)
    payload, manifest = output / 'game.data.gz', output / 'manifest.json'
    if manifest.exists():
        old = json.loads(manifest.read_text())
        parts = old.get('parts', [])
        if (old.get('dataBytes') == expected_size and old.get('dataSha256') == expected_hash
                and 1 <= len(parts) <= 64
                and all(part['file'] == 'game.data.%03d.chunk' % i
                        and (output / part['file']).is_file()
                        and 1 <= part['bytes'] <= 64 * 1024 * 1024
                        and (output / part['file']).stat().st_size == part['bytes']
                        and digest(output / part['file']) == part['sha256'] for i, part in enumerate(parts))):
            print(json.dumps(dict(reused=True, **old)), flush=True)
            return
    if payload.exists() and manifest.exists():
        old = json.loads(manifest.read_text())
        if (old.get('dataBytes') == expected_size and old.get('dataSha256') == expected_hash
                and payload.stat().st_size == old.get('compressedBytes')
                and digest(payload) == old.get('compressedSha256')):
            split_payload(payload, old, manifest)
            return
    temporary = output / 'game.data.gz.partial'
    checksum, count, next_report = hashlib.sha256(), 0, 0
    try:
        with source.open('rb') as incoming, temporary.open('wb') as raw:
            with gzip.GzipFile(filename='', mode='wb', fileobj=raw, compresslevel=6, mtime=0) as compressed:
                while chunk := incoming.read(1024 * 1024):
                    count += len(chunk)
                    if count > expected_size:
                        raise ValueError('Game data exceeds reviewed size')
                    checksum.update(chunk)
                    compressed.write(chunk)
                    if count >= next_report:
                        print(json.dumps(dict(preparing_bytes=count, total=expected_size)), flush=True)
                        next_report = count + 128 * 1024 * 1024
        if count != expected_size or checksum.hexdigest() != expected_hash:
            raise ValueError('Game data SHA-256 mismatch; bundle not accepted')
        info = dict(format=1, file='game.data.gz', dataBytes=count, dataSha256=expected_hash,
                    compressedBytes=temporary.stat().st_size, compressedSha256=digest(temporary))
        temporary.replace(payload)
        split_payload(payload, info, manifest)
    finally:
        temporary.unlink(missing_ok=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=ROOT.parent / '.tools/references/game.data')
    parser.add_argument('--output', type=Path, default=ROOT.parent / '.tools/bundled-game/assets/bundled')
    args = parser.parse_args()
    prepare(args.source, args.output)
