#!/usr/bin/env python3
"""Renders the port's stand-in for the gadgets' "Helvetica" (Arial under
Windows, a TrueType font) as Windows bitmap fonts: Liberation Sans, which
has Arial's metrics, at the cell heights the game asks for (16, 16 bold
and 24), hinted and without smoothing, as GDI drew it. Written as bare
`FNT` 3.0 files into crates/engine/fonts. Needs Pillow and Liberation
Sans 2.x (SIL OFL 1.1, whose Reserved Font Names "Liberation" and
"Arimo" a derivative can't carry: the face is "Widget Sans").

Usage: tools/make-sans-font.py [LIBERATION_DIR]
"""
import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

FACE = "Widget Sans"
OUT = Path(__file__).resolve().parent.parent / "crates/engine/fonts"
# Arial's (and Liberation Sans') usWinAscent and usWinDescent, per 2048.
WIN_ASCENT, WIN_DESCENT, EM = 1854, 434, 2048
FIRST, LAST, DEFAULT = 32, 255, ord("?")
COPYRIGHT = b"Liberation Sans: (c) 2012 Red Hat, (c) 2010 Google; SIL OFL 1.1"


def chars() -> list[str]:
    out = []
    for c in range(FIRST, LAST + 1):
        try:
            out.append(bytes([c]).decode("cp1252"))
        except UnicodeDecodeError:
            out.append(None)
    return out


def render(ttf: Path, height: int, bold: bool) -> bytes:
    # CreateFont with a positive height asks for the cell height: GDI
    # scales the em to fit the Windows ascent and descent into it.
    ppem = round(height * EM / (WIN_ASCENT + WIN_DESCENT))
    font = ImageFont.truetype(str(ttf), ppem)
    ascent = round(WIN_ASCENT * ppem / EM)
    glyphs = []
    for ch in chars():
        if ch is None:
            glyphs.append(None)
            continue
        w = max(round(font.getlength(ch, mode="1")), 1 if ch != " " else 0)
        img = Image.new("1", (max(w, 1), height), 0)
        d = ImageDraw.Draw(img)
        d.fontmode = "1"
        d.text((0, ascent), ch, font=font, fill=1, anchor="ls")
        glyphs.append((w, img))
    glyphs = [g if g is not None else glyphs[DEFAULT - FIRST] for g in glyphs]

    face = FACE.encode() + b"\0"
    count = LAST - FIRST + 2  # and the sentinel
    table_at = 148
    bits_at = table_at + count * 6
    bits = bytearray()
    table = bytearray()
    for w, img in glyphs:
        table += struct.pack("<HI", w, bits_at + len(bits))
        px = img.load()
        for strip in range((w + 7) // 8):
            for y in range(height):
                b = 0
                for bit in range(8):
                    x = strip * 8 + bit
                    if x < w and px[x, y]:
                        b |= 0x80 >> bit
                bits.append(b)
    table += struct.pack("<HI", 0, bits_at + len(bits))
    bits += b"\0" * height
    face_at = bits_at + len(bits)
    size = face_at + len(face)
    widths = [w for w, _ in glyphs]
    head = struct.pack(
        "<HI60sHHHHHHHBBBHBHHBHHBBBBHIIIIBIHHHI16s",
        0x300, size, COPYRIGHT.ljust(60, b"\0"),
        0,  # raster
        round(ppem * 72 / 96), 96, 96,
        ascent, height - ppem, 0,
        0, 0, 0,
        700 if bold else 400,
        0,  # ANSI
        0, height,
        0x21,  # FF_SWISS, variable pitch
        round(sum(widths) / len(widths)), max(widths),
        FIRST, LAST, DEFAULT - FIRST, ord(" ") - FIRST,
        0, 0, face_at, 0, bits_at, 0,
        2,  # DFF_PROPORTIONAL
        0, 0, 0, 0, b"\0" * 16,
    )
    assert len(head) == table_at, len(head)
    return head + table + bits + face


def main() -> None:
    src = Path(sys.argv[1] if len(sys.argv) > 1 else "/usr/share/fonts/liberation")
    OUT.mkdir(parents=True, exist_ok=True)
    for height, bold in [(16, False), (16, True), (24, False)]:
        ttf = src / ("LiberationSans-Bold.ttf" if bold else "LiberationSans-Regular.ttf")
        name = f"WSANS{height}{'B' if bold else ''}.FNT"
        (OUT / name).write_bytes(render(ttf, height, bold))
        print(OUT / name)


if __name__ == "__main__":
    main()
