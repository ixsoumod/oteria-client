"""Oteria's own cape, in Minecraft's cape layout at twice its size (128x64): the cape (its
outside at (1,1)-(11,17), its inside at (12,1)-(22,17), its edges) and its elytra
((22,0)-(46,22)), so the elytra worn with it matches. Purple falling to deep, a light trim, a
diamond in a diamond. Writes crates/horizon-client/assets/horizon/textures/gui/oteria_cape.png.

Run: python branding/cosmetics/make_oteria_cape.py
"""
import math
import pathlib

from PIL import Image

REPO = pathlib.Path(__file__).resolve().parents[2]
OUT = REPO / "crates/horizon-client/assets/horizon/textures/gui/oteria_cape.png"
K = 2  # texels per cape pixel
MAIN = (0x5B, 0x2B, 0xB5)
LIGHT = (0xC9, 0xA2, 0xFF)


def scale(c, k):
    return tuple(max(0, min(255, round(v * k))) for v in c)


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def grain(x, y):
    h = (x * 374761393 ^ y * 668265263) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return ((h >> 8) & 0xFFFF) / 32768.0 - 1.0


def face(img, x0, y0, w, h, emblem_at):
    """A face of the cloth: the fall of colour, the trim, the emblem `emblem_at` of the way down."""
    W, H = w * K, h * K
    for y in range(H):
        for x in range(W):
            fy = y / (H - 1)
            g = grain(x0 * K + x, y0 * K + y) * 0.05
            fold = math.sin(x * 0.55 * 32 / W) * 0.05
            c = scale(MAIN, 1.08 - 0.38 * fy + fold + g)
            if x < K or x >= W - K or y >= H - K:
                c = scale(LIGHT, 0.92 + g)
            elif x < 2 * K or x >= W - 2 * K or y >= H - 2 * K:
                c = scale(MAIN, 0.6)
            if emblem_at is not None:
                cx, cy = (W - 1) / 2, H * emblem_at
                d = (abs(x - cx) + abs(y - cy)) / (W / 32 * 2)
                if 7.0 <= d < 8.6 or d < 3.2:
                    c = mix(c, LIGHT, 0.95)
                elif d < 7.0:
                    c = mix(c, scale(MAIN, 0.7), 0.6)
            img.putpixel((x0 * K + x, y0 * K + y), c + (255,))


def fill(img, x0, y0, w, h, colour):
    for y in range(y0 * K, (y0 + h) * K):
        for x in range(x0 * K, (x0 + w) * K):
            g = grain(x, y) * 0.05
            img.putpixel((x, y), scale(colour, 1 + g) + (255,))


def main():
    img = Image.new("RGBA", (64 * K, 32 * K), (0, 0, 0, 0))
    # The cape: 10x16x1, box UV at (0, 0).
    fill(img, 1, 0, 10, 1, LIGHT)  # top
    fill(img, 11, 0, 10, 1, scale(LIGHT, 0.8))  # bottom
    fill(img, 0, 1, 1, 16, LIGHT)  # side
    face(img, 1, 1, 10, 16, 0.32)  # outside
    fill(img, 11, 1, 1, 16, LIGHT)  # side
    fill(img, 12, 1, 10, 16, scale(MAIN, 0.42))  # inside
    # The elytra's wing: 10x20x2, box UV at (22, 0).
    fill(img, 24, 0, 10, 2, LIGHT)
    fill(img, 34, 0, 10, 2, scale(LIGHT, 0.8))
    fill(img, 22, 2, 2, 20, LIGHT)
    face(img, 24, 2, 10, 20, 0.3)
    fill(img, 34, 2, 2, 20, LIGHT)
    face(img, 36, 2, 10, 20, None)
    img.save(OUT, optimize=True)
    print(OUT)


main()
