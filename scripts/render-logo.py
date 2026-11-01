# /// script
# requires-python = ">=3.10"
# dependencies = ["pillow>=10"]
# ///
"""Renders assets/logo.txt into the images the README and the repository use.

    uv run scripts/render-logo.py            # writes assets/logo-light.png, logo-dark.png, social-preview.png

The text file is the source of truth. It has three blocks separated by blank lines: the diff hunk
(its `+` line and `<` marker take the accent colour), the wordmark and the tagline. Edit the text,
run the script, commit both.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"

FONT_CANDIDATES = [
    "~/Library/Fonts/DejaVuSansMNerdFontMono-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
]

ACCENT = (245, 158, 11)
THEMES = {
    "light": {"ink": (31, 35, 40), "soft": (107, 114, 128), "background": None},
    "dark": {"ink": (240, 243, 246), "soft": (139, 148, 158), "background": None},
}
SOCIAL_BACKGROUND = (11, 15, 23)
SOCIAL_TAGLINE = "A terminal code reviewer for AI coding agents"


def find_font(explicit: str | None) -> str:
    for candidate in [explicit, *FONT_CANDIDATES]:
        if candidate and Path(candidate).expanduser().exists():
            return str(Path(candidate).expanduser())
    raise SystemExit("no monospace font found; pass one with --font")


def blocks(text: str) -> list[list[str]]:
    result: list[list[str]] = [[]]
    for line in text.rstrip("\n").splitlines():
        if line.strip():
            result[-1].append(line.rstrip())
        elif result[-1]:
            result.append([])
    return [block for block in result if block]


def layout(text: str, tagline: str | None):
    hunk, word, tag = blocks(text)
    if tagline is not None:
        tag = [tagline]
    # The wordmark's left edge is the reference: drop the margin the text file carries.
    margin = min(len(line) - len(line.lstrip()) for block in (hunk, word, tag) for line in block)
    rows: list[tuple[str, str]] = []
    for kind, block in (("hunk", hunk), ("word", word), ("tag", tag)):
        for line in block:
            rows.append((kind, line[margin:] if kind != "tag" else line.strip()))
        rows.append(("gap", ""))
    return rows[:-1]


def render(rows, size, font_path, theme, background, center_tag):
    width, height = size
    text_width = max(len(line) for kind, line in rows if kind in ("hunk", "word"))
    # Fit the widest line into 70% of the canvas width, then check the height.
    probe = ImageFont.truetype(font_path, 100)
    cell_w = probe.getlength("M")
    px = int(100 * (width * 0.7) / (cell_w * text_width))
    font = ImageFont.truetype(font_path, px)
    ascent, descent = font.getmetrics()
    line_h = int((ascent + descent) * 1.08)
    cell_w = font.getlength("M")
    total = line_h * len(rows)
    while total > height * 0.86 and px > 8:
        px -= 2
        font = ImageFont.truetype(font_path, px)
        ascent, descent = font.getmetrics()
        line_h = int((ascent + descent) * 1.08)
        cell_w = font.getlength("M")
        total = line_h * len(rows)

    image = Image.new("RGBA", size, background + (255,) if background else (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    block_w = cell_w * text_width
    left = (width - block_w) / 2
    top = (height - total) / 2
    for index, (kind, line) in enumerate(rows):
        y = top + index * line_h
        if kind == "tag":
            tag_x = (width - cell_w * len(line)) / 2 if center_tag else left
            draw.text((tag_x, y), line, font=font, fill=theme["ink"])
            continue
        for column, char in enumerate(line):
            if char == " ":
                continue
            if kind == "hunk":
                colour = ACCENT if char in "+<" else theme["soft"]
            else:
                colour = theme["ink"]
            draw.text((left + column * cell_w, y), char, font=font, fill=colour)
    return image


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--font", help="path of a monospace font")
    args = parser.parse_args()
    font_path = find_font(args.font)
    text = (ASSETS / "logo.txt").read_text()

    for name, theme in THEMES.items():
        image = render(layout(text, None), (1000, 1000), font_path, theme, None, True)
        image.save(ASSETS / f"logo-{name}.png", optimize=True)

    social = render(
        layout(text, SOCIAL_TAGLINE),
        (1280, 640),
        font_path,
        THEMES["dark"],
        SOCIAL_BACKGROUND,
        True,
    )
    social.convert("RGB").save(ASSETS / "social-preview.png", optimize=True)


if __name__ == "__main__":
    main()
