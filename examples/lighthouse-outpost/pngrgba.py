"""Decode an 8-bit RGBA PNG with the Python standard library."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path


def decode(path: Path) -> tuple[int, int, bytes]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")
    pos = 8
    width = height = None
    bit_depth = color_type = None
    idat: list[bytes] = []
    while pos + 8 <= len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, bit_depth, color_type = struct.unpack(">IIBB", chunk[:10])
        elif kind == b"IDAT":
            idat.append(chunk)
        elif kind == b"IEND":
            break
    if width is None or height is None or bit_depth is None or color_type is None:
        raise SystemExit(f"{path} has no IHDR")
    if bit_depth != 8 or color_type != 6:
        raise SystemExit(f"{path} is not 8-bit RGBA (depth {bit_depth}, type {color_type})")
    raw = zlib.decompress(b"".join(idat))
    stride = width * 4
    rows: list[bytes] = []
    index = 0
    prev = bytearray(stride)
    for _y in range(height):
        if index >= len(raw):
            raise SystemExit(f"{path} is truncated")
        filter_type = raw[index]
        index += 1
        row = bytearray(raw[index : index + stride])
        index += stride
        if len(row) != stride:
            raise SystemExit(f"{path} row is short")
        recon = _unfilter(filter_type, row, prev)
        rows.append(bytes(recon))
        prev = recon
    return width, height, b"".join(rows)


def _unfilter(filter_type: int, row: bytearray, prev: bytearray) -> bytearray:
    stride = len(row)
    out = bytearray(stride)
    bpp = 4
    for i in range(stride):
        left = out[i - bpp] if i >= bpp else 0
        up = prev[i]
        ul = prev[i - bpp] if i >= bpp else 0
        sample = row[i]
        if filter_type == 0:
            mixed = sample
        elif filter_type == 1:
            mixed = sample + left
        elif filter_type == 2:
            mixed = sample + up
        elif filter_type == 3:
            mixed = sample + ((left + up) // 2)
        elif filter_type == 4:
            mixed = sample + _paeth(left, up, ul)
        else:
            raise SystemExit(f"PNG filter {filter_type} is not supported")
        out[i] = mixed & 255
    return out


def _paeth(left: int, up: int, ul: int) -> int:
    estimate = left + up - ul
    dl = abs(estimate - left)
    du = abs(estimate - up)
    dul = abs(estimate - ul)
    if dl <= du and dl <= dul:
        return left
    if du <= dul:
        return up
    return ul
