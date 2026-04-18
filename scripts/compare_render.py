#!/usr/bin/env python3
"""
Render Comparison: Prisimi GPU Rects vs Chrome Screenshot

Reads prisimi_google_rects.csv (dumped by the render pipeline test)
and renders them onto a 1920x1080 canvas using PIL, then places
the result side-by-side with the Chrome screenshot.
"""

import csv
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:
    print("pip install Pillow required")
    sys.exit(1)

PROJ_ROOT = Path(__file__).parent.parent
ARTIFACT_DIR = PROJ_ROOT / ".gemini" / "antigravity" / "brain"
OUTPUT_DIR = PROJ_ROOT / "tests" / "output"
RECTS_CSV = OUTPUT_DIR / "prisimi_google_rects.csv"

W, H = 1920, 1080


def render_rects(csv_path: Path) -> Image.Image:
    """Render GPU primitives from CSV onto a white canvas."""
    img = Image.new("RGBA", (W, H), (255, 255, 255, 255))
    draw = ImageDraw.Draw(img)

    with open(csv_path) as f:
        reader = csv.DictReader(f)
        count = 0
        for row in reader:
            x = float(row["x"])
            y = float(row["y"])
            w = float(row["w"])
            h = float(row["h"])
            r = int(row["r"])
            g = int(row["g"])
            b = int(row["b"])
            a = int(row["a"])
            ptype = int(row["type"])
            opacity = float(row["opacity"])

            if w <= 0 or h <= 0:
                continue

            # Apply opacity
            a = int(a * opacity)

            # Type 0 = Rect, Type 1 = Glyph (skip for now), Type 2 = Image
            if ptype == 0:
                draw.rectangle(
                    [x, y, x + w, y + h], fill=(r, g, b, a)
                )
            elif ptype == 1:
                # Glyph — draw as a small colored rect placeholder
                draw.rectangle(
                    [x, y, x + w, y + h], fill=(r, g, b, a)
                )
            count += 1

    print(f"Rendered {count} primitives")
    return img


def main():
    if not RECTS_CSV.exists():
        print(f"ERROR: {RECTS_CSV} not found. Run the render pipeline test first:")
        print(
            "  zsh scripts/run_test.sh tests/test_e2e_render_pipeline.salt"
        )
        sys.exit(1)

    print(f"Loading rects from {RECTS_CSV}...")
    prisimi_img = render_rects(RECTS_CSV)

    # Save Prisimi render
    prisimi_path = OUTPUT_DIR / "prisimi_google_render.png"
    prisimi_img.save(str(prisimi_path))
    print(f"Saved Prisimi render: {prisimi_path}")

    # Try to find Chrome screenshot
    chrome_path = None
    if ARTIFACT_DIR.exists():
        for conv_dir in ARTIFACT_DIR.iterdir():
            for f in conv_dir.glob("google_homepage_*.png"):
                chrome_path = f
                break

    if chrome_path:
        print(f"Found Chrome screenshot: {chrome_path}")
        chrome_img = Image.open(str(chrome_path)).convert("RGBA")
        # Resize Chrome to match
        chrome_img = chrome_img.resize((W, H), Image.Resampling.LANCZOS)

        # Create side-by-side comparison
        comparison = Image.new("RGBA", (W * 2 + 20, H + 60), (30, 30, 30, 255))
        comparison.paste(prisimi_img, (0, 60))
        comparison.paste(chrome_img, (W + 20, 60))

        # Add labels
        label_draw = ImageDraw.Draw(comparison)
        label_draw.text((W // 2 - 80, 15), "Prisimi Engine", fill=(255, 255, 255))
        label_draw.text((W + 20 + W // 2 - 60, 15), "Chrome", fill=(255, 255, 255))

        comp_path = OUTPUT_DIR / "render_comparison.png"
        comparison.save(str(comp_path))
        print(f"Saved comparison: {comp_path}")
    else:
        print("Chrome screenshot not found, skipping side-by-side")

    print("Done!")


if __name__ == "__main__":
    main()
