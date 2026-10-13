"""Generate the prototype's geometric book icon using only the standard library."""
from pathlib import Path
import struct
import zlib

size = 128
rows = bytearray()
for y in range(size):
    rows.append(0)
    for x in range(size):
        color = (35, 63, 54, 255)
        if 24 <= x <= 59 and 28 <= y <= 97 or 68 <= x <= 103 and 28 <= y <= 97:
            color = (237, 236, 211, 255)
        if ((31 <= x <= 52) or (75 <= x <= 96)) and y in range(43, 83) and (y - 43) % 12 < 3:
            color = (92, 116, 95, 255)
        rows.extend(color)
def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
image = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b'')
Path(__file__).resolve().parents[1].joinpath('apps/writer/src-tauri/icons/icon.png').write_bytes(image)
