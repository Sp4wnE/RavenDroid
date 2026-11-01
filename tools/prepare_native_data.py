#!/usr/bin/env python3
"""Stream a reviewed local build into UnityWebData and write its import profile."""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import struct
from prepare_assets import verify

ROOT = Path(__file__).resolve().parents[1]


class Parts(io.RawIOBase):
    def __init__(self, paths):
        self.paths = iter(paths)
        self.current = None

    def readable(self):
        return True

    def readinto(self, buffer):
        while True:
            if self.current is None:
                path = next(self.paths, None)
                if path is None:
                    return 0
                self.current = path.open('rb')
            count = self.current.readinto(buffer)
            if count:
                return count
            self.current.close()
            self.current = None

    def close(self):
        if self.current:
            self.current.close()
        super().close()


def prepare(source, output, profile_path):
    lock = json.loads((ROOT / 'tools/source-lock.json').read_text())
    parts = sorted(source.joinpath('Build').glob('*.part.*'))
    if len(parts) != 15:
        raise ValueError('Expected all 15 reviewed data parts')
    entries = {entry['path']: entry for entry in lock['files']}
    for part in parts:
        verify(part, entries[part.relative_to(source).as_posix()])
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + '.partial')
    digest = hashlib.sha256()
    total = 0
    with Parts(parts) as raw, io.BufferedReader(raw, 128*1024) as joined, gzip.GzipFile(fileobj=joined) as data, temporary.open('wb') as out:
        while chunk := data.read(128*1024):
            total += len(chunk)
            if total > 1200*1024*1024:
                raise ValueError('UnityWebData exceeds expected size limit')
            digest.update(chunk)
            out.write(chunk)
    directory = []
    with temporary.open('rb') as stream:
        if stream.read(16) != b'UnityWebData1.0\0':
            raise ValueError('Unexpected Unity data container')
        end = struct.unpack('<I', stream.read(4))[0]
        if not 20 <= end <= 1024*1024:
            raise ValueError('Invalid container directory size')
        while stream.tell() < end:
            offset, size, length = struct.unpack('<III', stream.read(12))
            if not 0 < length <= 4096 or stream.tell()+length > end or offset < end or offset+size > total:
                raise ValueError('Invalid data container entry')
            name = stream.read(length).decode()
            directory.append({'path': name, 'offset': offset, 'bytes': size})
    temporary.replace(output)
    build = json.loads((ROOT / 'native-runtime/build-profile.json').read_text())
    profile = {'format': 1, 'sourceCommit': lock['commit'], 'wasmSha256': build['wasmSha256'],
               'dataBytes': total, 'dataSha256': digest.hexdigest(), 'partCount': len(parts), 'files': directory}
    profile_path.parent.mkdir(parents=True, exist_ok=True)
    profile_path.write_text(json.dumps(profile, indent=2) + '\n')
    print(json.dumps({k: v for k, v in profile.items() if k != 'files'}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path, help='Authorized local Ravenfield repository')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--profile', type=Path, default=ROOT / 'native-android/app/src/main/assets/native/data-profile.json')
    args = parser.parse_args()
    prepare(args.source.resolve(), args.output.resolve(), args.profile.resolve())
