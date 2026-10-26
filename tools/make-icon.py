#!/usr/bin/env python3
"""Draws the port's icon (crates/widget/res/icon.png and icon.ico): a
bubbling flask in front of a gear, with a spark. It's the port's own, not
the game's. Needs Pillow.

Usage: tools/make-icon.py
"""
import math
from pathlib import Path

from PIL import Image, ImageDraw

S = 1024  # drawn large, scaled down smooth
OUT = Path(__file__).resolve().parent.parent / "crates/widget/res"


def gear(d: ImageDraw.ImageDraw, cx: float, cy: float, r: float, teeth: int, fill, hole) -> None:
    pts = []
    for k in range(teeth * 4):
        a = 2 * math.pi * k / (teeth * 4) - math.pi / 2
        rr = r if k % 4 in (1, 2) else r * 0.8
        pts.append((cx + math.cos(a) * rr, cy + math.sin(a) * rr))
    d.polygon(pts, fill=fill)
    d.ellipse((cx - r * 0.34, cy - r * 0.34, cx + r * 0.34, cy + r * 0.34), fill=hole)


def draw() -> Image.Image:
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle((32, 32, S - 32, S - 32), radius=200, fill=255)
    scene = Image.new("RGBA", (S, S))
    d = ImageDraw.Draw(scene)
    # the lab's wall, darker at the bottom
    for y in range(S):
        t = y / S
        d.line(((0, y), (S, y)), fill=(int(52 - 30 * t), int(36 - 20 * t), int(110 - 50 * t)))
    # the gears behind
    gear(d, 330, 330, 230, 10, (240, 176, 40), (43, 29, 92))
    gear(d, 700, 250, 140, 8, (214, 140, 30), (40, 27, 86))
    # the flask: neck, then the round body
    neck = (440, 360, 590, 560)
    body = (250, 470, 780, 960)
    glass = (210, 236, 250)
    d.rectangle(neck, fill=glass)
    d.ellipse(body, fill=glass)
    d.rounded_rectangle((410, 330, 620, 380), radius=20, fill=(235, 245, 252))
    # the liquid, the lower part of the body
    liquid = Image.new("L", (S, S), 0)
    ld = ImageDraw.Draw(liquid)
    ld.ellipse((body[0] + 26, body[1] + 26, body[2] - 26, body[3] - 26), fill=255)
    ld.rectangle((0, 0, S, 660), fill=0)
    green = Image.new("RGBA", (S, S), (80, 220, 90, 255))
    scene.paste(green, (0, 0), liquid)
    d.ellipse((body[0] + 60, 640, body[2] - 60, 690), fill=(140, 245, 140))
    # bubbles rising
    for x, y, r in ((470, 600, 26), (560, 520, 20), (500, 440, 16), (585, 380, 12), (450, 760, 34), (640, 820, 22)):
        d.ellipse((x - r, y - r, x + r, y + r), outline=(250, 255, 250), width=8)
    # a highlight on the glass
    d.arc((300, 520, 730, 910), 200, 250, fill=(255, 255, 255), width=22)
    # the spark
    bolt = [(790, 420), (690, 610), (770, 610), (700, 800), (880, 560), (790, 560), (860, 420)]
    d.polygon(bolt, fill=(255, 236, 80), outline=(255, 255, 255), width=6)
    img.paste(scene, (0, 0), mask)
    return img


def main() -> None:
    big = draw()
    big.resize((512, 512), Image.LANCZOS).save(OUT / "icon.png")
    big.resize((256, 256), Image.LANCZOS).save(
        OUT / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    )


if __name__ == "__main__":
    main()
