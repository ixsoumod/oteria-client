"""Windows stand-in for `cargo run -p xtask -- icons`, which needs rsvg-convert (librsvg).

Renders docs/media/brand/icon.svg, icon-macos.svg and avatar.svg with headless Microsoft Edge
(transparent background) and writes the same files xtask does, in the same sizes and formats:
packaging/icons (hicolor PNGs, horizon.ico, horizon.icns), the window icon
crates/horizon-theme/icons/horizon-64.png, and the avatars and web icons in docs/media/brand.

Run: python branding/make_icons.py   (after branding/make_logo.py)
"""
import io
import pathlib
import shutil
import struct
import subprocess
import sys
import tempfile

from PIL import Image

REPO = pathlib.Path(__file__).resolve().parent.parent
BRAND = REPO / "docs" / "media" / "brand"
PACKAGING = REPO / "packaging" / "icons"
HICOLOR = [16, 24, 32, 48, 64, 128, 256, 512]
ICO = [16, 24, 32, 48, 64, 128, 256]
ICNS = [(b"icp4", 16), (b"icp5", 32), (b"icp6", 64), (b"ic07", 128), (b"ic08", 256),
        (b"ic09", 512), (b"ic10", 1024), (b"ic11", 32), (b"ic12", 64), (b"ic13", 256),
        (b"ic14", 512)]

EDGE = next((p for p in [
    pathlib.Path(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
    pathlib.Path(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
] if p.exists()), None)
if EDGE is None:
    sys.exit("Microsoft Edge not found")

TMP = pathlib.Path(tempfile.mkdtemp(prefix="oteria-icons-"))
MASTER = 1024
_cache: dict = {}


def master(svg: pathlib.Path) -> Image.Image:
    """`svg` rendered once at 1024 px on a transparent background."""
    if svg not in _cache:
        page = TMP / f"{svg.stem}.html"
        page.write_text(
            '<html><body style="margin:0;background:transparent">'
            f'<img src="{svg.as_uri()}" style="display:block;width:{MASTER}px;height:{MASTER}px">'
            "</body></html>", encoding="utf-8")
        shot = TMP / f"{svg.stem}.png"
        subprocess.run([str(EDGE), "--headless=new", "--disable-gpu", "--hide-scrollbars",
                        "--force-device-scale-factor=1", "--allow-file-access-from-files",
                        "--default-background-color=00000000", f"--window-size={MASTER},{MASTER}",
                        f"--screenshot={shot}", page.as_uri()],
                       check=True, capture_output=True, timeout=60)
        _cache[svg] = Image.open(shot).convert("RGBA")
    return _cache[svg]


def render(svg: pathlib.Path, size: int) -> bytes:
    """PNG bytes of `svg` at `size` x `size`."""
    out = io.BytesIO()
    master(svg).resize((size, size), Image.LANCZOS).save(out, "PNG", optimize=True)
    return out.getvalue()


def write(path: pathlib.Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def ico(images):
    """An .ico of PNG entries, as xtask writes it."""
    out = bytearray(struct.pack("<HHH", 0, 1, len(images)))
    offset = 6 + 16 * len(images)
    for size, png in images:
        side = 0 if size >= 256 else size
        out += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(png), offset)
        offset += len(png)
    for _, png in images:
        out += png
    return bytes(out)


def icns(entries):
    """An .icns of PNG entries, as xtask writes it."""
    body = b"".join(kind + struct.pack(">I", 8 + len(png)) + png for kind, png in entries)
    return b"icns" + struct.pack(">I", 8 + len(body)) + body


square, macos, avatar = BRAND / "icon.svg", BRAND / "icon-macos.svg", BRAND / "avatar.svg"
for size in HICOLOR:
    write(PACKAGING / f"hicolor/{size}x{size}/apps/horizon.png", render(square, size))
write(PACKAGING / "hicolor/scalable/apps/horizon.svg", square.read_bytes())
write(PACKAGING / "horizon.ico", ico([(s, render(square, s)) for s in ICO]))
write(PACKAGING / "horizon.icns", icns([(k, render(macos, s)) for k, s in ICNS]))
write(REPO / "crates/horizon-theme/icons/horizon-64.png", render(square, 64))
for svg, size, out in [
    (avatar, 500, "docs/media/brand/social/github-avatar-500.png"),
    (avatar, 180, "docs/media/brand/web/apple-touch-icon.png"),
    (square, 32, "docs/media/brand/web/favicon-32.png"),
    (square, 1024, "docs/media/brand/icon-1024.png"),
    (macos, 1024, "docs/media/brand/icon-macos-1024.png"),
]:
    write(REPO / out, render(svg, size))
write(BRAND / "web/favicon.svg", square.read_bytes())
# The badge before an Oteria player's name (horizon_client::name_badge): 32 px, drawn 8 text
# pixels square, so 1:1 at GUI scale 4.
badge = REPO / "branding" / "oteria-badge.svg"
write(REPO / "crates/horizon-client/assets/horizon/textures/gui/oteria_badge.png", render(badge, 32))
shutil.rmtree(TMP, ignore_errors=True)
print("wrote packaging/icons, the window icon, the avatars and the website's icons")
