"""The mod menu's icons at 128 px (white, transparent), which the menus resample to the exact
pixels they are drawn at (`ui_theme::Ui::icon`): sharp at any GUI scale.

Renders branding/icons/tabler/*.svg with headless Microsoft Edge (as make_icons.py does) and
writes crates/horizon-client/assets/horizon/textures/gui/icons/hi/<name>.png and the Rust table
crates/horizon-client-render/src/oteria_icon_masters.rs.

Run: python branding/make_icon_masters.py
"""
import pathlib
import shutil
import subprocess
import sys
import tempfile

from PIL import Image

REPO = pathlib.Path(__file__).resolve().parent.parent
TABLER = REPO / "branding" / "icons" / "tabler"
OUT = REPO / "crates/horizon-client/assets/horizon/textures/gui/icons/hi"
SIZE = 128
SHOT = 1024

EDGE = next((p for p in [
    pathlib.Path(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
    pathlib.Path(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
] if p.exists()), None)
if EDGE is None:
    sys.exit("Microsoft Edge not found")

tmp = pathlib.Path(tempfile.mkdtemp(prefix="oteria-icon-masters-"))
OUT.mkdir(parents=True, exist_ok=True)
names = sorted(p.stem for p in TABLER.glob("*.svg"))
for name in names:
    white = tmp / f"{name}.svg"
    white.write_text((TABLER / f"{name}.svg").read_text(encoding="utf-8").replace("currentColor", "#ffffff"),
                     encoding="utf-8")
    page = tmp / f"{name}.html"
    page.write_text(
        '<html><body style="margin:0;background:transparent">'
        f'<img src="{white.as_uri()}" style="display:block;width:{SHOT}px;height:{SHOT}px">'
        "</body></html>", encoding="utf-8")
    shot = tmp / f"{name}.png"
    subprocess.run([str(EDGE), "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--force-device-scale-factor=1", "--allow-file-access-from-files",
                    "--default-background-color=00000000", f"--window-size={SHOT},{SHOT}",
                    f"--screenshot={shot}", page.as_uri()],
                   check=True, capture_output=True, timeout=60)
    img = Image.open(shot).convert("RGBA").resize((SIZE, SIZE), Image.LANCZOS)
    # White everywhere, the shape in the alpha alone.
    alpha = img.getchannel("A")
    out = Image.new("LA", (SIZE, SIZE), 255)
    out.putalpha(alpha)
    out.save(OUT / f"{name}.png", optimize=True)

ident = lambda n: n.upper().replace("-", "_")
rust = [
    "//! The mod menu's icons at 128 texels (white, the shape in the alpha), which the menus",
    "//! resample to the exact pixels they are drawn at: sharp at any GUI scale.",
    "//! Written by `branding/make_icon_masters.py`: edit that, not this.",
    "",
    "/// The 128 px PNG of `icon` (one of `oteria_icons`' names, or its small version's).",
    "pub fn master(icon: &str) -> Option<&'static [u8]> {",
    '    let name = icon.rsplit(\'/\').next()?;',
    "    Some(match name {",
]
for name in names:
    rust.append(f'        "{name}" => include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/hi/{name}.png"),')
rust += ["        _ => return None,", "    })", "}", ""]
(REPO / "crates/horizon-client-render/src/oteria_icon_masters.rs").write_text("\n".join(rust), encoding="utf-8")
shutil.rmtree(tmp, ignore_errors=True)
print(f"wrote {len(names)} icon masters")
