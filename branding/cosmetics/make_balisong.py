"""Oteria's butterfly knife (balisong), drawn from scratch as voxels: its model and its 64x64
texture, in the cosmetics item format (docs/cosmetics.md), written to
crates/horizon-client/assets/horizon/cosmetics/oteria_balisong.json, and the Cosmetics page's
picture of it.

A cyan blade (dark spine, bright edge, a stepped clip point) and two skeletonised handles in
Oteria's dark steel: open windows and cyan inlays in turn along each, a steel pin at each end,
the latch handle's butt in violet. Both handles turn on the tang (the origin): open, they lie
together behind the blade; shut, each has turned half a turn and the blade lies between them,
seen through the windows.

Model space: one voxel a unit, y down, the blade toward -x, the handles toward +x, the pins
along z. The game scales it (`oteria_knife.rs`).

Run: python branding/cosmetics/make_balisong.py
"""
import base64
import io
import json
import pathlib

from PIL import Image, ImageDraw

REPO = pathlib.Path(__file__).resolve().parents[2]
OUT = REPO / "crates/horizon-client/assets/horizon/cosmetics/oteria_balisong.json"
GUI = REPO / "crates/horizon-client/assets/horizon/textures/gui"

W = H = 64
img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
px = img.load()

SPINE = (22, 110, 138, 255)
BLADE = (43, 191, 233, 255)
BLADE_LIGHT = (120, 222, 246, 255)
EDGE = (214, 246, 252, 255)
TANG = (30, 92, 112, 255)
HANDLE = (20, 27, 38, 255)
HANDLE_LIGHT = (40, 52, 68, 255)
INLAY = (43, 191, 233, 255)
PIN = (196, 204, 214, 255)
VIOLET = (155, 92, 255, 255)


def rect(x, y, w, h, colour):
    for i in range(int(x), int(x + w)):
        for j in range(int(y), int(y + h)):
            if 0 <= i < W and 0 <= j < H:
                px[i, j] = colour


def box_uv(u, v, size):
    """Minecraft's box UV: the faces of a `size` (w, h, d) box laid out from (u, v)."""
    w, h, d = size
    return {
        "top": (u + d, v, w, d),
        "bottom": (u + d + w, v, w, d),
        "east": (u, v + d, d, h),
        "north": (u + d, v + d, w, h),
        "west": (u + d + w, v + d, d, h),
        "south": (u + d + w + d, v + d, w, h),
    }


# A shelf packer for each texture region (2(d + w) by d + h texels).
shelf = {"x": 0, "y": 0, "h": 0}


def region(size):
    w, h, d = size
    rw, rh = int(2 * (d + w)), int(d + h)
    if shelf["x"] + rw > W:
        shelf["x"], shelf["y"], shelf["h"] = 0, shelf["y"] + shelf["h"], 0
    u, v = shelf["x"], shelf["y"]
    assert v + rh <= H, "the texture is full"
    shelf["x"] += rw
    shelf["h"] = max(shelf["h"], rh)
    return [u, v]


def paint(size, base, front=None, sides=None):
    """A region for a box of `size`, every face `base`; `front(face, x, y, w, h)` paints the
    flat faces (north and south, whose columns run in opposite x), `sides` the others."""
    uv = region(size)
    faces = box_uv(uv[0], uv[1], size)
    for face, (x, y, w, h) in faces.items():
        rect(x, y, w, h, base)
        if sides and face not in ("north", "south"):
            sides(face, x, y, w, h)
    if front:
        for face in ("north", "south"):
            front(face, *faces[face])
    return uv


# The blade: a body three voxels across and two steps to its point, one voxel thick. Rows
# down it: the spine, the steel, the edge.
BODY, TIP1, TIP2 = (12, 3, 1), (2, 2, 1), (2, 1, 1)


def body_front(face, x, y, w, h):
    rect(x, y, w, 1, SPINE)
    rect(x, y + 1, w, 1, BLADE)
    rect(x, y + 2, w, 1, EDGE)
    # A light fuller along the steel, and the dark tang by the pins (x 1 to -1: the face's first
    # columns on the north face, its last on the south).
    rect(x + 3, y + 1, w - 6, 1, BLADE_LIGHT)
    tang = x if face == "north" else x + w - 2
    rect(tang, y, 2, h, TANG)


def body_sides(face, x, y, w, h):
    rect(x, y, w, h, {"top": SPINE, "bottom": EDGE}.get(face, BLADE))


body_uv = paint(BODY, BLADE, body_front, body_sides)
tip1_uv = paint(TIP1, BLADE, lambda f, x, y, w, h: (rect(x, y, w, 1, SPINE), rect(x, y + 1, w, 1, EDGE)),
                lambda f, x, y, w, h: rect(x, y, w, h, EDGE if f == "bottom" else SPINE if f == "top" else BLADE))
tip2_uv = paint(TIP2, EDGE)

# A handle: two rails, a cap at each end and a core between them, cut through by windows and
# set with inlays in turn (a pattern the same both ways, so the core's two faces line up).
RAIL, CAP, CORE = (18, 1, 2), (2, 2, 2), (14, 2, 2)
WINDOWS, INLAYS = (0, 6, 12), (3, 9)


def rail_front(face, x, y, w, h):
    rect(x, y, w, h, HANDLE)


def lit(face, x, y, w, h):
    if face in ("top", "bottom"):
        rect(x, y, w, h, HANDLE_LIGHT)


rail_uv = paint(RAIL, HANDLE, rail_front, lit)
pin_cap_uv = paint(CAP, HANDLE, lambda f, x, y, w, h: rect(x, y, 2, 2, PIN))
butt_uv = paint(CAP, HANDLE_LIGHT, lambda f, x, y, w, h: rect(x, y, 2, 2, PIN))
violet_butt_uv = paint(CAP, VIOLET, lambda f, x, y, w, h: (rect(x, y, 2, 2, VIOLET), rect(x + (1 if f == "north" else 0), y, 1, 1, PIN)))


def core_front(face, x, y, w, h):
    for k in WINDOWS:
        rect(x + k, y, 2, h, (0, 0, 0, 0))
    for k in INLAYS:
        rect(x + k, y, 2, h, INLAY)


core_uv = paint(CORE, HANDLE, core_front)


def cube(origin, size, uv):
    return {"origin": list(origin), "size": list(size), "uv": uv, "inflate": 0, "mirror": False}


def handle(z0, latch):
    """A handle from its pin (the origin) toward +x, its thickness from `z0`."""
    cubes = [
        cube([-1, -2, z0], RAIL, rail_uv),
        cube([-1, 1, z0], RAIL, rail_uv),
        cube([-1, -1, z0], CAP, pin_cap_uv),
        cube([15, -1, z0], CAP, violet_butt_uv if latch else butt_uv),
        cube([1, -1, z0], CORE, core_uv),
    ]
    return cubes


buf = io.BytesIO()
img.save(buf, "PNG")
texture = base64.b64encode(buf.getvalue()).decode()

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
                cube([-11, -1.5, -0.5], BODY, body_uv),
                cube([-13, -1.5, -0.5], TIP1, tip1_uv),
                cube([-15, -1.5, -0.5], TIP2, tip2_uv),
            ],
            "children": [],
        },
        {"name": "handle_safe", "pivot": [0, 0, 0], "rotation": [0, 0, 0], "cubes": handle(-2.5, False), "children": []},
        {"name": "handle_latch", "pivot": [0, 0, 0], "rotation": [0, 0, 0], "cubes": handle(0.5, True), "children": []},
    ],
    "animations": [],
}
OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_text(json.dumps(item, indent=1) + "\n", encoding="utf-8")
img.save(REPO / "branding/cosmetics/oteria_balisong.png")
# The same texture in the GUI atlas, where the hand renderer reads it (`oteria_art::KNIFE`).
img.save(GUI / "oteria_balisong.png")


def preview():
    """The Cosmetics page's picture (192x96): the open knife seen flat, its latch handle's
    face, each voxel a block of pixels with a shade on its lower right."""
    S = 5
    pic = Image.new("RGBA", (192, 96), (0, 0, 0, 0))
    draw = ImageDraw.Draw(pic)
    ox, oy = 96 - S, 48

    def face_colour(uv, size, i, j):
        w, h, d = size
        # The north face's texel for voxel column i (from the box's -x end) and row j.
        x = uv[0] + d + (w - 1 - i)
        return px[int(x), int(uv[1] + d + j)]

    def blit(origin, size, uv):
        w, h, _ = size
        for i in range(int(w)):
            for j in range(int(h)):
                c = face_colour(uv, size, i, j)
                if c[3] == 0:
                    continue
                x0, y0 = ox + (origin[0] + i) * S, oy + (origin[1] + j) * S
                shade = tuple(int(k * 0.72) for k in c[:3]) + (255,)
                draw.rectangle([x0 + 1, y0 + 1, x0 + S, y0 + S], fill=shade)
                draw.rectangle([x0, y0, x0 + S - 2, y0 + S - 2], fill=c)

    # The knife tilted a little is not needed: flat, it reads at a glance.
    for c in item["parts"][0]["cubes"] + handle(0.5, True):
        blit(c["origin"], c["size"], c["uv"])
    pic.save(GUI / "oteria_balisong_preview.png")


preview()
print("wrote", OUT.relative_to(REPO))
