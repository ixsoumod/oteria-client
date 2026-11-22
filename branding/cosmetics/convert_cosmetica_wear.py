"""Copies the capes, hats and shoulder pets downloaded from Cosmetica into private/cosmetics/wear/
(not in git: they belong to whoever made them), as the game reads them (`oteria_art::WEAR`):

- `<id>.png`: the texture, its frames stacked top to bottom when it is animated. A cape's frame
  is twice as wide as tall (Minecraft's 64x32 cape layout, its elytra included); a cape wider
  than 256 is scaled down to 256 (an HD cape's frames would fill the atlas).
- `<id>.json`: a hat's or pet's model, a Java block model (Blockbench), as Cosmetica serves it.

Run: python branding/cosmetics/convert_cosmetica_wear.py "C:/Users/<you>/Pictures/textures & model"
It prints each one's frames, for `oteria_art::WEAR`.
"""
import json
import pathlib
import shutil
import sys

from PIL import Image

REPO = pathlib.Path(__file__).resolve().parents[2]
OUT = REPO / "private" / "cosmetics" / "wear"
CAPE_WIDTH = 256

# (folder, file's start, id)
CAPES = [
    ("FlameFrags Hellstorm", "hellstorm"),
    ("HellStorm Purple", "hellstorm_purple"),
    ("HellStorm Toxic", "hellstorm_toxic"),
    ("Pizza", "pizza"),
    ("Red Moon", "red_moon"),
    ("Sharpness Enchant", "sharpness"),
    ("Spring Droplets", "spring_droplets"),
    ("cicada 3301", "cicada"),
]
HATS = [
    ("Anthaeres_'s Red Halo", "red_halo"),
    ("Black Cat hears", "black_cat_ears"),
    ("CAT EARS OK", "cat_ears"),
    ("Cigar", "cigar"),
    ("Demon Horns", "demon_horns"),
    ("Doug Dimmaldome Cowboy Hat", "tall_hat"),
    ("Purple star demon horns", "star_horns"),
    ("TLF Axolotl", "axolotl_hat"),
]
PETS = [
    ("black cat", "black_cat"),
    ("Even cooler Axolotl", "axolotl"),
    ("Opossum", "opossum"),
    ("Sir Shrimp, my cat", "ginger_cat"),
    ("Snow Fox Kit", "snow_fox"),
]


def find(folder: pathlib.Path, start: str, ext: str) -> pathlib.Path:
    hits = [p for p in folder.iterdir() if p.name.startswith(start) and p.suffix == ext]
    if len(hits) != 1:
        sys.exit(f"{folder.name}/{start}*{ext}: {len(hits)} files")
    return hits[0]


def main() -> None:
    src = pathlib.Path(sys.argv[1])
    OUT.mkdir(parents=True, exist_ok=True)
    for start, cid in CAPES:
        img = Image.open(find(src / "capes & elytra", start, ".png")).convert("RGBA")
        w, h = img.size
        frame_h = w // 2
        frames = max(1, h // frame_h)
        img = img.crop((0, 0, w, frame_h * frames))
        if w > CAPE_WIDTH:
            # Each frame on its own, so no frame bleeds into the next.
            out = Image.new("RGBA", (CAPE_WIDTH, CAPE_WIDTH // 2 * frames))
            for f in range(frames):
                frame = img.crop((0, f * frame_h, w, (f + 1) * frame_h))
                frame = frame.resize((CAPE_WIDTH, CAPE_WIDTH // 2), Image.BOX)
                out.paste(frame, (0, f * CAPE_WIDTH // 2))
            img = out
        img.save(OUT / f"{cid}.png", optimize=True)
        print(f"cape {cid}: {img.size[0]}x{img.size[1]}, {frames} frame(s)")
    for folder, items, kind in (("hat", HATS, "hat"), ("pets", PETS, "pet")):
        for start, cid in items:
            model = find(src / folder, start, ".json")
            model_json = json.loads(model.read_text(encoding="utf-8"))
            tw, th = model_json.get("texture_size", [16, 16])
            img = Image.open(find(src / folder, start, ".png")).convert("RGBA")
            w, h = img.size
            frame_h = round(w * th / tw)
            frames = max(1, h // frame_h)
            img.save(OUT / f"{cid}.png", optimize=True)
            shutil.copyfile(model, OUT / f"{cid}.json")
            print(f"{kind} {cid}: {w}x{h}, {frames} frame(s), {len(model_json['elements'])} cubes")


main()
