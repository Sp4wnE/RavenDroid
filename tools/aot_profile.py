"""Read the app's exported engine-profile constant from its little-endian ELF."""
import struct

PROFILES = {'legacy': 0, 'guest-eh-v1': 1, 'guest-eh-inline-eager-v1': 3, 'guest-eh-epoch-v1': 5}
EPOCH_WATCHDOG = {'tickMs': 10, 'startupMs': 60000, 'callbackMs': 30000}


def library_flags(data):
    if data[:6] != b'\x7fELF\x02\x01':
        raise ValueError('Expected little-endian ELF64 JNI')
    offset = struct.unpack_from('<Q', data, 40)[0]
    stride, count = struct.unpack_from('<HH', data, 58)
    if stride != 64 or not 0 < count < 4096 or offset + count * stride > len(data):
        raise ValueError('Invalid ELF sections')
    sections = [struct.unpack_from('<IIQQQQIIQQ', data, offset+i*stride) for i in range(count)]
    for table in sections:
        if table[1] != 11:  # SHT_DYNSYM
            continue
        if table[9] != 24 or table[5] % 24 or table[6] >= count:
            raise ValueError('Invalid dynamic symbols')
        strings = sections[table[6]]
        names = data[strings[4]:strings[4]+strings[5]]
        if len(names) != strings[5] or table[4]+table[5] > len(data):
            raise ValueError('Truncated ELF symbols')
        for position in range(table[4], table[4]+table[5], 24):
            name, info, _, section, address, size = struct.unpack_from('<IBBHQQ', data, position)
            if names[name:].split(b'\0', 1)[0] != b'RAVENDROID_AOT_PROFILE_V1':
                continue
            if not 0 < section < count or size != 4 or info & 15 != 1:
                raise ValueError('Invalid exported AOT profile')
            owner = sections[section]
            start = owner[4] + address - owner[3]
            if address < owner[3] or address+4 > owner[3]+owner[5] or start+4 > len(data):
                raise ValueError('AOT profile outside ELF data')
            return struct.unpack_from('<I', data, start)[0]
    return None  # Legacy 0.17 libraries predate this handshake.


def verify_library(data, manifest):
    profile = manifest.get('engineProfile', 'legacy')
    if profile not in PROFILES:
        raise ValueError('Unknown AOT engine profile')
    if manifest.get('fuel') is not (profile != 'guest-eh-epoch-v1'):
        raise ValueError('AOT execution guard does not match its engine profile')
    if profile == 'guest-eh-epoch-v1' and manifest.get('epochWatchdog') != EPOCH_WATCHDOG:
        raise ValueError('Unknown epoch watchdog profile')
    actual = library_flags(data)
    if actual != PROFILES[profile] and not (actual is None and profile == 'legacy'):
        raise ValueError('JNI features do not match the signed AOT engine profile')
    return profile
