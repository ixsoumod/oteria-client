"""Converts a balisong made in Blender (a .glb) into the butterfly knife the game draws
(`game/oteria_knife.rs`), in `private/cosmetics/` next to the repository's root: a model file
and a 512x512 texture (its thin strips stay a few texels wide).

`private/` is not in git. The models this reads are licensed to whoever downloaded them, not
to every reader of this public repository: their files, converted or not, are never committed.

Made for "Balisong - Butterfly knife" by LeoPham on CGTrader (Royalty Free License, no AI). Its
parts, as its file names them: the blade (`Cylinder.004`), two handles (`Cylinder.001` with its
inlays `Cylinder.008` / `Cylinder.017`, and `Cylinder.006`), each on its pin (`Cylinder.003`,
`Cylinder.005`) with a washer (`Cylinder.002`, `Cylinder.007`). The knife lies flat in the file's
x-z plane, its blade toward -z.

The output is in the game knife's model space: pixels at twice the detail, the blade toward -x,
the flat of the blade in the x-y plane, the pins along z, the tang between the pins at the
origin. Each part is a pivot and triangles of (x, y, z, u, v), from its pivot.

Run: python branding/cosmetics/convert_glb_balisong.py "<path to balisong knife.glb>"
"""
import io
import json
import pathlib
import struct
import sys

import numpy as np
from PIL import Image

REPO = pathlib.Path(__file__).resolve().parents[2]
OUT = REPO / "private" / "cosmetics"
NAME = "balisong_leopham"
TEXTURE = 512

PARTS = {
    "blade": ["Cylinder.004"],
    "handle_safe": ["Cylinder.001", "Cylinder.002", "Cylinder.003", "Cylinder.008", "Cylinder.009"],
    "handle_latch": ["Cylinder.006", "Cylinder.007", "Cylinder.005"],
}
# Each handle turns about its own pin (node names).
PINS = {"handle_safe": "Cylinder.003", "handle_latch": "Cylinder.005"}
# The game knife's length in model pixels (blade tip to handle end), which the model is scaled to.
LENGTH = 41.0

COMP = {5126: np.float32, 5123: np.uint16, 5125: np.uint32, 5121: np.uint8}
NCOMP = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}


def main(path):
    data = pathlib.Path(path).read_bytes()
    clen = struct.unpack_from("<I", data, 12)[0]
    doc = json.loads(data[20:20 + clen])
    boff = 20 + clen
    blen = struct.unpack_from("<I", data, boff)[0]
    binary = data[boff + 8: boff + 8 + blen]

    def view(index):
        bv = doc["bufferViews"][index]
        off = bv.get("byteOffset", 0)
        return binary[off: off + bv["byteLength"]]

    def accessor(i):
        a = doc["accessors"][i]
        bv = doc["bufferViews"][a["bufferView"]]
        off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
        n = NCOMP[a["type"]]
        arr = np.frombuffer(binary, COMP[a["componentType"]], a["count"] * n, off)
        return arr.reshape(a["count"], n) if n > 1 else arr

    def local(node):
        if "matrix" in node:
            return np.array(node["matrix"], dtype=np.float64).reshape(4, 4).T
        t = node.get("translation", [0, 0, 0])
        x, y, z, w = node.get("rotation", [0, 0, 0, 1])
        s = node.get("scale", [1, 1, 1])
        rot = np.array([
            [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
        ])
        m = np.eye(4)
        m[:3, :3] = rot @ np.diag(s)
        m[:3, 3] = t
        return m

    nodes = doc["nodes"]
    parent = {c: i for i, n in enumerate(nodes) for c in n.get("children", [])}

    def world(i):
        m = local(nodes[i])
        while i in parent:
            i = parent[i]
            m = local(nodes[i]) @ m
        return m

    by_name = {n.get("name"): i for i, n in enumerate(nodes)}

    def triangles(name):
        """The node's triangles in the file's space: positions (n, 3, 3) and UVs (n, 3, 2)."""
        i = by_name[name]
        m = world(i)
        out_p, out_uv = [], []
        for pr in doc["meshes"][nodes[i]["mesh"]]["primitives"]:
            p = accessor(pr["attributes"]["POSITION"]).astype(np.float64)
            uv = accessor(pr["attributes"]["TEXCOORD_0"]).astype(np.float64)
            idx = accessor(pr["indices"]).astype(np.int64) if "indices" in pr else np.arange(len(p))
            w = (np.c_[p, np.ones(len(p))] @ m.T)[:, :3]
            out_p.append(w[idx].reshape(-1, 3, 3))
            out_uv.append(uv[idx].reshape(-1, 3, 2))
        return np.concatenate(out_p), np.concatenate(out_uv)

    def centre(name):
        p, _ = triangles(name)
        q = p.reshape(-1, 3)
        return (q.min(0) + q.max(0)) / 2

    pins = {part: centre(node) for part, node in PINS.items()}
    tang = (pins["handle_safe"] + pins["handle_latch"]) / 2

    # The whole knife's length along the file's z, for the scale.
    allp = np.concatenate([triangles(n)[0].reshape(-1, 3) for names in PARTS.values() for n in names])
    k = LENGTH / (allp[:, 2].max() - allp[:, 2].min())

    def to_game(p):
        """File space (knife flat in x-z, blade toward -z) to the game knife's: x along the knife,
        y across the blade, z through it (a cyclic swap keeps the handedness), in its pixels."""
        q = (p - tang) * k
        return np.stack([q[..., 2], q[..., 0], q[..., 1]], axis=-1)

    parts = []
    for part, names in PARTS.items():
        pivot = to_game(pins[part]) if part in pins else np.zeros(3)
        tris = []
        for n in names:
            p, uv = triangles(n)
            g = to_game(p) - pivot
            for t in range(len(g)):
                tris.append([[round(float(c), 4) for c in g[t, j]] + [round(float(uv[t, j, 0]), 5), round(float(uv[t, j, 1]), 5)] for j in range(3)])
        parts.append({"name": part, "pivot": [round(float(c), 4) for c in pivot], "triangles": tris})

    # The base colour, small enough for the GUI atlas.
    material = doc["materials"][0]
    tex = material["pbrMetallicRoughness"]["baseColorTexture"]["index"]
    image = doc["images"][doc["textures"][tex]["source"]]
    if "bufferView" in image:
        colour = Image.open(io.BytesIO(view(image["bufferView"])))
    else:
        colour = Image.open(pathlib.Path(path).parent / image["uri"])
    # Opaque: the game cuts out see-through texels, and the metal has none.
    colour = colour.convert("RGB").convert("RGBA").resize((TEXTURE, TEXTURE), Image.LANCZOS)

    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / f"{NAME}.json").write_text(json.dumps({
        "credit": "Balisong - Butterfly knife by LeoPham (CGTrader, Royalty Free License, no AI)",
        "parts": parts,
    }, separators=(",", ":")), encoding="utf-8")
    colour.save(OUT / f"{NAME}.png")
    count = sum(len(p["triangles"]) for p in parts)
    print(f"wrote {OUT.relative_to(REPO)}/{NAME}.json ({count} triangles) and {NAME}.png")


if __name__ == "__main__":
    main(sys.argv[1])
