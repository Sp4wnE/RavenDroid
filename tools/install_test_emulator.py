"""Install only Google's stable Windows emulator and API35 AOSP x86_64 image.

Uses repository checksums, workspace-only paths and checked archive members.
SDK Manager's Java downloader can stall behind a proxy; urllib uses host proxy.
"""
import hashlib
import argparse
import json
from pathlib import Path
import time
import urllib.request
import xml.etree.ElementTree as ET
import zipfile
import zlib

WORKSPACE = Path(__file__).resolve().parents[2]
SDK = WORKSPACE / '.tools/android-sdk'
DOWNLOADS = WORKSPACE / '.tools/downloads'
PACKAGES = [
    ('https://dl.google.com/android/repository/', 'repository2-1.xml', 'emulator', SDK, 'emulator'),
    ('https://dl.google.com/android/repository/sys-img/android/', 'sys-img2-1.xml',
     'system-images;android-35;default;x86_64', SDK / 'system-images/android-35/default', 'x86_64'),
]


def install(image_only=False):
    DOWNLOADS.mkdir(exist_ok=True)
    reports = []
    for base, index, package, destination, prefix in PACKAGES:
        if image_only and package == 'emulator':
            continue
        with urllib.request.urlopen(base + index, timeout=45) as response:
            repository = ET.fromstring(response.read())
        packages = [p for p in repository.findall('remotePackage')
                    if p.get('path') == package and p.find('channelRef').get('ref') == 'channel-0']
        if len(packages) != 1:
            raise ValueError('Expected one stable package: ' + package)
        node = packages[0]
        archives = [a for a in node.findall('archives/archive') if a.findtext('host-os') in (None, 'windows')]
        if len(archives) != 1:
            raise ValueError('Ambiguous Windows archive')
        complete = archives[0].find('complete')
        url = base + complete.findtext('url')
        size = int(complete.findtext('size'))
        checksum = complete.find('checksum')
        algorithm = checksum.get('type', 'sha1')
        expected = checksum.text.strip()
        path = DOWNLOADS / ('avd-' + url.rsplit('/', 1)[-1])
        started = time.monotonic()
        print(f'download {package} bytes={size}', flush=True)
        cached = False
        if path.is_file() and path.stat().st_size == size:
            with path.open('rb') as stream:
                cached = hashlib.file_digest(stream, algorithm).hexdigest() == expected
        if not cached:
            downloaded = 0
            digest = hashlib.new(algorithm)
            with urllib.request.urlopen(url, timeout=60) as source, path.open('wb') as output:
                while chunk := source.read(1024 * 1024):
                    output.write(chunk)
                    digest.update(chunk)
                    downloaded += len(chunk)
                    if downloaded % (32 * 1024 * 1024) == 0:
                        print(f'{package} {downloaded}/{size}', flush=True)
            cached = downloaded == size and digest.hexdigest() == expected
        if not cached:
            raise ValueError('Official archive checksum/size mismatch: ' + str(path))
        destination = destination.resolve()
        destination.relative_to(WORKSPACE)
        with zipfile.ZipFile(path) as archive:
            for member in archive.infolist():
                target = (destination / member.filename).resolve()
                target.relative_to(destination / prefix)
                if (member.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError('Unexpected archive symlink')
            existing = destination / prefix
            scaffold = existing.is_dir() and all(p.name == '.installer' for p in existing.iterdir())
            if existing.exists() and not scaffold:
                # SDK Manager may have completed before its parent was stopped.
                # Reuse only when every archive file matches; never overwrite it.
                for member in archive.infolist():
                    if member.is_dir():
                        continue
                    installed = destination / member.filename
                    if not installed.is_file() or installed.stat().st_size != member.file_size:
                        raise ValueError('Existing package differs: ' + member.filename)
                    crc = 0
                    with installed.open('rb') as stream:
                        while chunk := stream.read(1024 * 1024):
                            crc = zlib.crc32(chunk, crc)
                    if crc != member.CRC:
                        raise ValueError('Existing package CRC mismatch: ' + member.filename)
            else:
                archive.extractall(destination)
        path.unlink()  # verified installation is retained; avoid duplicate ZIP storage
        reports.append({'package': package, 'url': url, 'bytes': size,
                        'checksum_algorithm': algorithm, 'checksum': expected,
                        'installed': str(destination / prefix),
                        'elapsed_seconds': round(time.monotonic() - started, 2)})
        print('installed ' + package, flush=True)
    report = WORKSPACE / 'native/artifacts/emulator-install.json'
    report.write_text(json.dumps(reports, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image-only', action='store_true')
    install(parser.parse_args().image_only)
