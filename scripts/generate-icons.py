#!/usr/bin/env python3
"""Gera os ícones do app desktop do Zuno a partir de `logo.png`.

Uso:
    python scripts/generate-icons.py

O `logo.png` da raiz é quadrado e com transparência. Este script preserva o canal
alfa (não força fundo branco) e gera, em `apps/desktop/src-tauri/icons/`:

- 32x32.png
- 128x128.png
- 128x128@2x.png (256x256)
- icon.png (512x512)
- icon.ico (multi-tamanho: 16, 24, 32, 48, 64, 128, 256)

Requisitos: Python 3 e Pillow (dependência **opcional de desenvolvimento**, usada
apenas para produzir artefatos versionados; nunca é necessária para compilar ou
executar o app). Nada é baixado da rede.

Equivalente manual (caso Pillow não esteja disponível):
    cargo install tauri-cli --version "^2" && cargo tauri icon logo.png
"""

from __future__ import annotations

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - depende do ambiente de dev
    sys.exit(
        "Pillow não encontrado. Instale com 'python -m pip install Pillow' "
        "ou gere os ícones com 'cargo tauri icon logo.png'."
    )

REPO_ROOT = Path(__file__).resolve().parent.parent
SOURCE = REPO_ROOT / "logo.png"
OUTPUT_DIR = REPO_ROOT / "apps" / "desktop" / "src-tauri" / "icons"

PNG_TARGETS = {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "icon.png": 512,
}

ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)


def load_source() -> Image.Image:
    if not SOURCE.is_file():
        sys.exit(f"Arquivo de origem não encontrado: {SOURCE}")

    image = Image.open(SOURCE)
    image.load()

    # O logo é um PNG quadrado com canal alfa. Convertemos para RGBA para manter
    # a transparência previsível em todos os tamanhos gerados.
    if image.mode != "RGBA":
        image = image.convert("RGBA")

    # Garante um canvas quadrado sem distorcer o logo.
    side = max(image.size)
    if image.size != (side, side):
        canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
        offset = ((side - image.width) // 2, (side - image.height) // 2)
        canvas.paste(image, offset)
        image = canvas

    return image


def main() -> int:
    source = load_source()
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    for filename, size in PNG_TARGETS.items():
        resized = source.resize((size, size), Image.Resampling.LANCZOS)
        resized.save(OUTPUT_DIR / filename, format="PNG", optimize=True)
        print(f"gerado {OUTPUT_DIR / filename} ({size}x{size})")

    ico_path = OUTPUT_DIR / "icon.ico"
    source.save(ico_path, format="ICO", sizes=[(s, s) for s in ICO_SIZES])
    print(f"gerado {ico_path} (tamanhos: {', '.join(f'{s}x{s}' for s in ICO_SIZES)})")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
