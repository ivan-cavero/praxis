"""Generate praxis app icons (PNG + ICO) using Pillow."""
from PIL import Image, ImageDraw, ImageFont
import os

SIZES = {
    "icon.png": 512,
    "128x128.png": 128,
    "256x256.png": 256,
}


def create_icon(size: int) -> Image.Image:
    """Create a praxis 'P' icon at the given size."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Background: dark rounded square (#09090b)
    margin = int(size * 0.08)
    bg_color = (9, 9, 11, 255)
    radius = int(size * 0.22)
    draw.rounded_rectangle(
        [margin, margin, size - margin, size - margin],
        radius=radius,
        fill=bg_color,
    )

    # Font
    font_size = int(size * 0.6)
    font = None
    font_paths = [
        "C:/Windows/Fonts/arialbd.ttf",
        "C:/Windows/Fonts/segoeuib.ttf",
        "C:/Windows/Fonts/consolab.ttf",
    ]
    for fp in font_paths:
        if os.path.exists(fp):
            font = ImageFont.truetype(fp, font_size)
            break
    if font is None:
        font = ImageFont.load_default()

    # Center text
    text = "P"
    text_color = (34, 197, 94, 255)  # primary green #22c55e
    glow_color = (34, 197, 94, 40)

    bbox = draw.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    x = (size - tw) / 2 - bbox[0]
    y = (size - th) / 2 - bbox[1]

    # Glow
    for dx, dy in [(-1, -1), (-1, 1), (1, -1), (1, 1)]:
        draw.text((x + dx, y + dy), text, fill=glow_color, font=font)
    # Main text
    draw.text((x, y), text, fill=text_color, font=font)

    return img


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))

    for filename, size in SIZES.items():
        img = create_icon(size)
        path = os.path.join(script_dir, filename)
        img.save(path, "PNG")
        print(f"  OK {filename} ({size}x{size}) — {os.path.getsize(path)} bytes")

    ico_img = create_icon(256)
    ico_path = os.path.join(script_dir, "icon.ico")
    ico_img.save(ico_path, "ICO", sizes=[(256, 256)])
    print(f"  OK icon.ico — {os.path.getsize(ico_path)} bytes")

    print("Done! All icons generated.")


if __name__ == "__main__":
    main()
