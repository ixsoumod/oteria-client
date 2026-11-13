"""Oteria's butterfly knife (balisong), drawn from scratch: its model and its 64x64 texture, in
the cosmetics item format (docs/cosmetics.md), written to
crates/horizon-client/assets/horizon/cosmetics/oteria_balisong.json.

Built at twice the pixel size and scaled by half (`attach.scale`), so the knife is fine-grained:
a blade with a clip point, its edge bevel and a spine notch; two skeletonised handles (Oteria's
dark steel, a cyan line, see-through cut-outs) that turn on their own pins at the tang; the
latch, violet, on the end of the latch handle. Open, the handles lie together behind the blade;
closed, each has turned half a turn about its pin and lies on one side of the blade.

Run: python branding/cosmetics/make_balisong.py
"""
import base64
import io
import json
import pathlib

from PIL import Image

REPO = pathlib.Path(__file__).resolve().parents[2]
OUT = REPO / "crates/horizon-client/assets/horizon/cosmetics/oteria_balisong.json"

W = H = 64
img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
px = img.load()


def rect(x, y, w, h, colour):
    for i in range(x, x + w):
        for j in range(y, y + h):
            if 0 <= i < W and 0 <= j < H:
                px[i, j] = colour


def box_uv(u, v, size):
    """Minecraft's box UV: the faces of a `size` (w, h, d) cube laid out from (u, v)."""
    w, h, d = size
    return {
        "top": (u + d, v, w, d),
        "bottom": (u + d + w, v, w, d),
        "east": (u, v + d, d, h),
        "north": (u + d, v + d, w, h),
        "west": (u + d + w, v + d, d, h),
        "south": (u + d + w + d, v + d, w, h),
    }


STEEL = (196, 204, 214, 255)
STEEL_DARK = (128, 137, 150, 255)
EDGE = (236, 242, 248, 255)
HANDLE = (30, 36, 48, 255)
HANDLE_LIGHT = (52, 60, 76, 255)
CYAN = (43, 191, 233, 255)
VIOLET = (155, 92, 255, 255)
PIN = (220, 226, 232, 255)

# The blade: 18 long, 4 tall, 1 thick (x along it, y down).
BLADE = (18, 4, 1)
uv = box_uv(0, 0, BLADE)
for face, (x, y, w, h) in uv.items():
    rect(x, y, w, h, STEEL)
for face in ("north", "south"):
    x, y, w, h = uv[face]
    # The spine along the top, the bevel and its bright edge along the bottom.
    rect(x, y, w, 1, STEEL_DARK)
    rect(x, y + h - 2, w, 1, STEEL_DARK)
    rect(x, y + h - 1, w, 1, EDGE)
    # A notch in the spine near the tang, and a fuller line along the blade.
    rect(x + (2 if face == "north" else w - 4), y, 2, 1, (0, 0, 0, 0))
    rect(x + 4, y + 1, w - 7, 1, (176, 184, 196, 255))

# The clip point: a shorter piece at the end, set low, so the tip comes to a point.
TIP = (3, 2, 1)
for face, (x, y, w, h) in box_uv(40, 0, TIP).items():
    rect(x, y, w, h, STEEL)
    if face in ("north", "south"):
        rect(x, y + h - 1, w, 1, EDGE)

# The handles: 20 long, 5 tall (the blade's 4 fits inside), 2 thick, skeletonised.
HANDLE_SIZE = (20, 5, 2)


def handle(u, v, latch_side):
    faces = box_uv(u, v, HANDLE_SIZE)
    for face, (x, y, w, h) in faces.items():
        rect(x, y, w, h, HANDLE)
    for face in ("north", "south"):
        x, y, w, h = faces[face]
        rect(x, y, w, 1, HANDLE_LIGHT)
        rect(x, y + h - 1, w, 1, HANDLE_LIGHT)
        # Oteria's cyan line along the middle, and three see-through cut-outs above and below it.
        rect(x + 3, y + 2, w - 6, 1, CYAN)
        for k in (5, 10, 15):
            rect(x + k, y + 1, 2, 1, (0, 0, 0, 0))
            rect(x + k + 1, y + 3, 2, 1, (0, 0, 0, 0))
        # The pin's end at the tang.
        pin_x = x + (w - 2 if face == "north" else 1)
        rect(pin_x, y + 2, 1, 1, PIN)
    for face in ("top", "bottom"):
        x, y, w, h = faces[face]
        rect(x, y, w, h, HANDLE_LIGHT)
    if latch_side:
        # The butt end in violet: where the latch sits.
        x, y, w, h = faces["east"]
        rect(x, y, w, h, VIOLET)


handle(0, 8, False)
handle(0, 16, True)

# The latch: a small violet bar that swings at the end of the latch handle.
LATCH = (4, 1, 2)
for face, (x, y, w, h) in box_uv(48, 8, LATCH).items():
    rect(x, y, w, h, VIOLET)

buf = io.BytesIO()
img.save(buf, "PNG")
texture = base64.b64encode(buf.getvalue()).decode()


def cube(origin, size, uv):
    return {"origin": origin, "size": list(size), "uv": uv, "inflate": 0, "mirror": False}


# Model pixels at twice the detail, y down; a part's pivot is from its parent's, its cubes from
# its own pivot (as vanilla's model parts). The tang (the pins) at the origin; the blade runs
# toward -x (forward in the hand), the handles toward +x when open.
item = {
    "format": 1,
    "id": "oteria_balisong",
    "slot": "back",
    "bone": "right_arm",
    "attach": {"offset": [-1, 10, -1], "rotation": [0, 0, 0], "scale": 0.5},
    "texture": {"png": texture},
    "parts": [
        {
            "name": "blade", "pivot": [0, 0, 0], "rotation": [0, 0, 0],
            "cubes": [
                cube([-18, -2, -0.5], BLADE, [0, 0]),
                cube([-21, -1, -0.5], TIP, [40, 0]),
            ],
            "children": [],
        },
        {
            # The safe handle: on one side of the blade, turning on its pin at the tang.
            "name": "handle_safe", "pivot": [0, 0, 0], "rotation": [0, 0, 0],
            "cubes": [cube([0, -2.5, -2.5], HANDLE_SIZE, [0, 8])],
            "children": [],
        },
        {
            # The latch handle: on the other side, the latch at its end.
            "name": "handle_latch", "pivot": [0, 0, 0], "rotation": [0, 0, 0],
            "cubes": [cube([0, -2.5, 0.5], HANDLE_SIZE, [0, 16])],
            "children": [
                {
                    "name": "latch", "pivot": [20, 0, 1.5], "rotation": [0, 0, 0],
                    "cubes": [cube([-1, 1.5, -1], LATCH, [48, 8])],
                    "children": [],
                }
            ],
        },
    ],
    "animations": [],
}
OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_text(json.dumps(item, indent=1) + "\n", encoding="utf-8")
img.save(REPO / "branding/cosmetics/oteria_balisong.png")
# The same texture in the GUI atlas, where the hand renderer reads it (`oteria_art::KNIFE`).
img.save(REPO / "crates/horizon-client/assets/horizon/textures/gui/oteria_balisong.png")
print("wrote", OUT.relative_to(REPO))
