import cairosvg
import os
from PIL import Image


def process_and_whiten_png(png_path):
  """Turns dark/background pixels transparent and forces all visible icon pixels to solid opaque white (255, 255, 255, 255)."""
  img = Image.open(png_path).convert("RGBA")
  width, height = img.size
  pixels = img.load()

  for y in range(height):
    for x in range(width):
      r, g, b, a = pixels[x, y]

      # Calculate brightness/average to determine if it's the background or the icon
      avg = (int(r) + int(g) + int(b)) / 3.0

      # If it's dark or part of the background, make it fully transparent
      if avg < 50 or a < 50:
        pixels[x, y] = (0, 0, 0, 0)
      else:
        # Otherwise, force it to completely solid opaque white
        pixels[x, y] = (255, 255, 255, 255)

  img.save(png_path)


def generate_pngs_from_svg(svg_path, tray_output_path, vscode_output_path):
  if not os.path.exists(svg_path):
    print(f"❌ Error: SVG file not found at '{svg_path}'")
    return

  # Ensure parent directories exist
  os.makedirs(os.path.dirname(tray_output_path), exist_ok=True)
  os.makedirs(os.path.dirname(vscode_output_path), exist_ok=True)

  # 1. Generate 32x32 for the system tray
  cairosvg.svg2png(
    url=svg_path,
    write_to=tray_output_path,
    output_width=32,
    output_height=32
  )
  process_and_whiten_png(tray_output_path)
  print(f"✅ Generated solid white 32x32 tray icon -> {tray_output_path}")

  # 2. Generate 128x128 for the VS Code extension
  cairosvg.svg2png(
    url=svg_path,
    write_to=vscode_output_path,
    output_width=128,
    output_height=128
  )
  process_and_whiten_png(vscode_output_path)
  print(f"✅ Generated solid white 128x128 VS Code icon -> {vscode_output_path}")


# Run the exporter pointing to your SVG inside the icons folder
generate_pngs_from_svg(
  svg_path='icons/img.svg',
  tray_output_path='pcbfapi/res/icon.png',
  vscode_output_path='extension/images/icon.png'
)
