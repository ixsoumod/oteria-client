"""Builds the launcher's Versions page covers from the update artwork the user keeps outside the repo.

Mojang's artwork is never committed: the covers land in `private/launcher-versions/` (ignored by git),
which `oteria-launcher/build.rs` embeds when it is there. Each one is cropped to a 3:4 portrait around
the picture's subject, with the update's logo, when there is one, across its top.

    python tools/oteria/make_version_covers.py ["C:/Users/<you>/Pictures/trailer screen"]
"""

import sys
from pathlib import Path

from PIL import Image

W, H = 360, 480
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "private" / "launcher-versions"

# (cover, picture, the subject's x as a share of the picture's width, rows cut off its top, logo or None)
COVERS = [
    ("26.png", "26.1.jpg", 0.49, 0, "tinytakeover.png"),
    ("1.21.png", "1.21.webp", 0.54, 0, "Tricky_Trials_Logo.webp"),
    # The 1.20 picture has a strip of the old logo along its top.
    ("1.20.png", "1.20.webp", 0.51, 8, "Trails & Tales.webp"),
]


def cover(src: Path, subject_x: float, top: int, logo: Path | None) -> Image.Image:
    im = Image.open(src).convert("RGB")
    im = im.crop((0, top, im.width, im.height))
    crop_w = round(im.height * W / H)
    left = round(im.width * subject_x - crop_w / 2)
    left = max(0, min(im.width - crop_w, left))
    out = im.crop((left, 0, left + crop_w, im.height)).resize((W, H), Image.LANCZOS)
    if logo is not None:
        mark = Image.open(logo).convert("RGBA")
        lw = round(W * 0.84)
        mark = mark.resize((lw, round(mark.height * lw / mark.width)), Image.LANCZOS)
        out.paste(mark, ((W - lw) // 2, round(H * 0.045)), mark)
    return out


def main() -> None:
    source = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / "Pictures" / "trailer screen"
    OUT.mkdir(parents=True, exist_ok=True)
    for name, picture, x, top, logo in COVERS:
        img = cover(source / picture, x, top, source / logo if logo else None)
        img.save(OUT / name, optimize=True)
        print(OUT / name)


if __name__ == "__main__":
    main()
