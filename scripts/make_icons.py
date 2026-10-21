#!/usr/bin/env python3
"""Draw Tunelito's app icon as PNG, ICO, and ICNS. No third-party packages."""

import struct
import zlib
from pathlib import Path

INK = (28, 25, 21, 255)
CREAM = (243, 236, 223, 255)
GREEN = (20, 108, 67, 255)
AMBER = (226, 163, 26, 255)
CLEAR = (0, 0, 0, 0)


def rounded(nx, ny):
    ax, ay = abs(nx), abs(ny)
    radius = 0.10
    limit = 0.5 - radius
    if ax > 0.5 or ay > 0.5:
        return False
    if ax <= limit or ay <= limit:
        return True
    dx, dy = ax - limit, ay - limit
    return dx * dx + dy * dy <= radius * radius


def pixel(x, y, w, h):
    nx = (x + 0.5) / w - 0.5
    ny = (y + 0.5) / h - 0.5
    if not rounded(nx, ny):
        return CLEAR
    # Tunnel arch, opening downward. Coordinates are centered.
    cx, cy, outer, inner = 0.0, 0.06, 0.30, 0.17
    dx, dy = nx - cx, ny - cy
    dist = (dx * dx + dy * dy) ** 0.5
    if dy <= 0.02 and inner <= dist <= outer:
        return CREAM
    if abs(nx) <= 0.30 and -0.02 <= ny - cy <= 0.16 and abs(nx) >= inner:
        return CREAM
    # Floor of the tunnel.
    if abs(nx) <= 0.30 and 0.18 <= ny <= 0.24:
        return GREEN
    # Amber mark, the switch light.
    if (nx + 0.0) ** 2 + (ny - 0.34) ** 2 <= 0.012:
        return AMBER
    return INK


def png_bytes(size):
    raw = bytearray()
    for y in range(size):
        raw.append(0)
        for x in range(size):
            raw.extend(pixel(x, y, size, size))

    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")


def ico_bytes(images):
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = b""
    blobs = b""
    for size, blob in images:
        entries += struct.pack(
            "<BBBBHHII",
            size if size < 256 else 0,
            size if size < 256 else 0,
            0,
            0,
            1,
            32,
            len(blob),
            offset,
        )
        offset += len(blob)
        blobs += blob
    return header + entries + blobs


def icns_bytes(entries):
    body = b""
    for kind, blob in entries:
        body += kind + struct.pack(">I", 8 + len(blob)) + blob
    return b"icns" + struct.pack(">I", 8 + len(body)) + body


def main():
    out = Path(__file__).resolve().parents[1] / "app" / "src-tauri" / "icons"
    out.mkdir(parents=True, exist_ok=True)
    sizes = {32: png_bytes(32), 128: png_bytes(128), 256: png_bytes(256), 512: png_bytes(512), 64: png_bytes(64)}
    (out / "32x32.png").write_bytes(sizes[32])
    (out / "128x128.png").write_bytes(sizes[128])
    (out / "128x128@2x.png").write_bytes(sizes[256])
    (out / "icon.png").write_bytes(sizes[512])
    (out / "icon.ico").write_bytes(ico_bytes([(32, sizes[32]), (128, sizes[128]), (256, sizes[256])]))
    (out / "icon.icns").write_bytes(
        icns_bytes(
            [
                (b"icp5", sizes[32]),
                (b"ic11", sizes[64]),
                (b"ic07", sizes[128]),
                (b"ic08", sizes[256]),
                (b"ic09", sizes[512]),
            ]
        )
    )
    print(f"wrote icons in {out}")


if __name__ == "__main__":
    main()
