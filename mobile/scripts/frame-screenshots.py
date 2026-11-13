#!/usr/bin/env python3
"""Present actual app captures on store-sized canvases without hardware frames."""

import argparse
import json
import os
import re
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont


def render(source, destination, caption, font_path):
    with Image.open(source) as capture:
        capture = capture.convert("RGB")
    width, height = capture.size
    canvas = Image.new("RGB", capture.size, "#F5F4EC")
    draw = ImageDraw.Draw(canvas)
    eyebrow = caption["keyword"]["text"]
    headline = caption["title"]["text"]
    eyebrow_font = ImageFont.truetype(str(font_path), round(width * 0.029))
    headline_size = round(width * 0.079)
    while True:
        headline_font = ImageFont.truetype(str(font_path), headline_size)
        bounds = draw.multiline_textbbox((0, 0), headline, font=headline_font, spacing=8, align="center")
        if bounds[2] - bounds[0] <= width * 0.9:
            break
        headline_size -= 1
    draw.text((width / 2, height * 0.045), eyebrow, font=eyebrow_font, fill="#367964", anchor="mt")
    draw.multiline_text(
        ((width - (bounds[2] - bounds[0])) / 2 - bounds[0], height * 0.085 - bounds[1]),
        headline, font=headline_font, fill="#183C31", align="center", spacing=8,
    )

    # Preserve the complete capture and its aspect ratio, including app controls.
    scale = min(width * 0.88 / width, height * 0.755 / height)
    panel = capture.resize((round(width * scale), round(height * scale)), Image.Resampling.LANCZOS)
    x, y = (width - panel.width) // 2, round(height * 0.215)
    radius = round(width * 0.025)
    mask = Image.new("L", panel.size)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, panel.width - 1, panel.height - 1), radius, fill=255)
    shadow = Image.new("RGBA", canvas.size)
    ImageDraw.Draw(shadow).rounded_rectangle(
        (x, y + round(width * 0.012), x + panel.width, y + panel.height + round(width * 0.012)),
        radius, fill=(24, 60, 49, 42),
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(width * 0.022))
    canvas = Image.alpha_composite(canvas.convert("RGBA"), shadow).convert("RGB")
    canvas.paste(panel, (x, y), mask)
    ImageDraw.Draw(canvas).rounded_rectangle(
        (x, y, x + panel.width - 1, y + panel.height - 1), radius,
        outline="#D3DCD1", width=max(1, round(width * 0.0015)),
    )
    destination.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=("android", "ios", "ipad"))
    args = parser.parse_args()
    mobile = Path(__file__).resolve().parent.parent
    locale = os.environ.get("LOCALE", "en-US")
    source_dir = mobile / "fastlane/screenshots" / args.platform / locale
    config_platform = "ios" if args.platform == "ipad" else args.platform
    config = json.loads((mobile / "fastlane-config" / config_platform / "Framefile.json").read_text())
    captions = {item["filter"]: item for item in config["data"]}
    sources = {}
    for source in sorted(source_dir.glob("*.png")):
        match = re.fullmatch(r"(?:(.+)-)?(\d{2}_[a-z]+)\.png", source.name)
        if match and match[2] in captions:
            sources.setdefault(match[1] or "", {})[match[2]] = source
    if len(sources) != 1 or set(next(iter(sources.values()), {})) != set(captions):
        parser.error(f"Expected one complete capture set ({', '.join(captions)}) in {source_dir}")
    prefix, captures = next(iter(sources.items()))
    store = "android" if args.platform == "android" else "ios"
    output = mobile / "fastlane/metadata" / store / locale
    output = output / "images/phoneScreenshots" if store == "android" else output / "screenshots"
    font_path = mobile / "fastlane-config/assets/Regular.ttf"
    rendered = []
    for index, (key, caption) in enumerate(captions.items(), 1):
        filename = f"{prefix}-{index}.png" if prefix else f"{index}.png"
        destination = source_dir / f"{prefix + '-' if prefix else ''}{key}_framed.png"
        render(captures[key], destination, caption, font_path)
        rendered.append((destination, output / filename))
    # Render the full set before replacing upload assets. Remove the old device
    # size as well, so an earlier iPhone/iPad set cannot survive a fresh capture.
    output.mkdir(parents=True, exist_ok=True)
    pattern = "*.png" if store == "android" else ("iPad*.png" if args.platform == "ipad" else "iPhone*.png")
    for old in output.glob(pattern):
        old.unlink()
    for source, destination in rendered:
        destination.write_bytes(source.read_bytes())
        print(destination.relative_to(mobile))


if __name__ == "__main__":
    main()
