from PIL import Image
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'submission-assets/rendered/contextsleep-thumbnail.png'
if not SOURCE.exists():
    raise SystemExit(f'Missing source thumbnail: {SOURCE}')

src = Image.open(SOURCE).convert('RGBA')

# Preserve the exact Product Hunt thumbnail artwork everywhere users see the brand.
# The desktop PNG is upscaled slightly for high-DPI rendering; all smaller assets are
# generated with Lanczos from the exact same source pixels.
master = src.resize((512, 512), Image.Resampling.LANCZOS)
master_path = ROOT / 'apps/desktop/src-tauri/icons/icon.png'
master.save(master_path, optimize=True)

# Windows executable icon with standard embedded resolutions.
master.save(
    ROOT / 'apps/desktop/src-tauri/icons/icon.ico',
    format='ICO',
    sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)],
)

outputs = {
    ROOT / 'apps/desktop/public/contextsleep-icon.png': 240,
    ROOT / 'extensions/chrome/public/icon.png': 240,
    ROOT / 'extensions/vscode/icon.png': 240,
    ROOT / 'launch-site/assets/icon.png': 240,
}
for path, size in outputs.items():
    path.parent.mkdir(parents=True, exist_ok=True)
    src.resize((size, size), Image.Resampling.LANCZOS).save(path, optimize=True)

print(f'Icon source: {SOURCE.relative_to(ROOT)} ({src.width}x{src.height})')
for p in [master_path, ROOT / 'apps/desktop/src-tauri/icons/icon.ico', *outputs.keys()]:
    print(f'{p.relative_to(ROOT)}  {p.stat().st_size} bytes')
