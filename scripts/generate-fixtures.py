#!/usr/bin/env python3
"""Gera as fixtures de áudio usadas pelos testes do servidor Zuno.

As fixtures são sintéticas (tons gerados por ffmpeg), pequenas e versionadas em
``fixtures/audio/``. O script é idempotente: rodar de novo reescreve os mesmos
arquivos. Não usa rede e nunca toca em arquivos de música do usuário.

Requisitos: Python 3.10+, Pillow e ffmpeg (com libopus e libvorbis).

Uso:
    python scripts/generate-fixtures.py
"""

from __future__ import annotations

import base64
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import NoReturn

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "fixtures" / "audio"

MAX_TOTAL_BYTES = 500 * 1024

FFMPEG_TIMEOUT_SECONDS = 120


def fail(message: str) -> NoReturn:
    print(f"erro: {message}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# Ogg / Opus: injeção de capa embutida (METADATA_BLOCK_PICTURE)
# ---------------------------------------------------------------------------
#
# O muxer Ogg do ffmpeg recusa streams de imagem (attached_pic), então a capa
# é inserida no pacote OpusTags reescrevendo os primeiros páginas Ogg. Todo o
# resto do fluxo Ogg é preservado byte a byte (só o número de sequência e o CRC
# das páginas seguintes são recalculados, porque a contagem de páginas muda).

_CRC_TABLE: list[int] = []


def _crc_table() -> list[int]:
    if _CRC_TABLE:
        return _CRC_TABLE

    for index in range(256):
        remainder = index << 24
        for _ in range(8):
            if remainder & 0x8000_0000:
                remainder = ((remainder << 1) ^ 0x04C1_1DB7) & 0xFFFF_FFFF
            else:
                remainder = (remainder << 1) & 0xFFFF_FFFF
        _CRC_TABLE.append(remainder)

    return _CRC_TABLE


def ogg_crc(data: bytes) -> int:
    table = _crc_table()
    crc = 0
    for byte in data:
        crc = ((crc << 8) & 0xFFFF_FFFF) ^ table[((crc >> 24) & 0xFF) ^ byte]
    return crc


class OggPage:
    __slots__ = ("header_type", "granule", "serial", "sequence", "segments", "payload")

    def __init__(
        self,
        header_type: int,
        granule: int,
        serial: int,
        sequence: int,
        segments: bytes,
        payload: bytes,
    ) -> None:
        self.header_type = header_type
        self.granule = granule
        self.serial = serial
        self.sequence = sequence
        self.segments = segments
        self.payload = payload

    @classmethod
    def parse_all(cls, data: bytes) -> list["OggPage"]:
        pages: list[OggPage] = []
        position = 0

        while position < len(data):
            if data[position : position + 4] != b"OggS":
                fail(f"página Ogg inválida na posição {position}")

            header_type = data[position + 5]
            granule = struct.unpack_from("<q", data, position + 6)[0]
            serial = struct.unpack_from("<I", data, position + 14)[0]
            sequence = struct.unpack_from("<I", data, position + 18)[0]
            stored_crc = struct.unpack_from("<I", data, position + 22)[0]
            segment_count = data[position + 26]
            segments = data[position + 27 : position + 27 + segment_count]
            payload_start = position + 27 + segment_count
            payload_end = payload_start + sum(segments)
            payload = data[payload_start:payload_end]

            raw = data[position:payload_end]
            patched = raw[:22] + b"\x00\x00\x00\x00" + raw[26:]
            if ogg_crc(patched) != stored_crc:
                fail(f"CRC Ogg inválido na posição {position}")

            pages.append(
                cls(header_type, granule, serial, sequence, segments, payload)
            )
            position = payload_end

        return pages

    def to_bytes(self, sequence: int | None = None) -> bytes:
        sequence = self.sequence if sequence is None else sequence
        header = (
            b"OggS"
            + bytes([0, self.header_type])
            + struct.pack("<q", self.granule)
            + struct.pack("<I", self.serial)
            + struct.pack("<I", sequence)
            + b"\x00\x00\x00\x00"
            + bytes([len(self.segments)])
            + self.segments
        )
        page = header + self.payload
        crc = ogg_crc(page)
        return page[:22] + struct.pack("<I", crc) + page[26:]


def flac_picture_block(
    image: bytes, mime_type: str, width: int, height: int, depth: int = 24
) -> bytes:
    """Monta bloco FLAC PICTURE (tipo 3 = capa frontal) para METADATA_BLOCK_PICTURE."""
    mime = mime_type.encode("ascii")
    block = struct.pack(">I", 3)
    block += struct.pack(">I", len(mime)) + mime
    block += struct.pack(">I", 0)  # descrição vazia
    block += struct.pack(">IIII", width, height, depth, 0)
    block += struct.pack(">I", len(image)) + image
    return block


def append_opus_tags_comment(packet: bytes, comment: bytes) -> bytes:
    """Adiciona um comentário ao pacote OpusTags, preservando os existentes."""
    if not packet.startswith(b"OpusTags"):
        fail("pacote OpusTags ausente no arquivo gerado pelo ffmpeg")

    offset = len(b"OpusTags")
    vendor_length = struct.unpack_from("<I", packet, offset)[0]
    offset += 4 + vendor_length
    user_comment_count = struct.unpack_from("<I", packet, offset)[0]
    comments_start = offset + 4

    existing: list[bytes] = []
    cursor = comments_start
    for _ in range(user_comment_count):
        length = struct.unpack_from("<I", packet, cursor)[0]
        cursor += 4
        existing.append(packet[cursor : cursor + length])
        cursor += length

    body = packet[:offset] + struct.pack("<I", user_comment_count + 1)
    for entry in existing:
        body += struct.pack("<I", len(entry)) + entry
    body += struct.pack("<I", len(comment)) + comment

    return body


def paginate_packet(packet: bytes, serial: int, first_sequence: int) -> tuple[list[bytes], int]:
    """Divide um pacote em páginas Ogg (sem intercalar outros pacotes)."""
    lacing: list[int] = []
    remaining = packet
    while len(remaining) >= 255:
        lacing.append(255)
        remaining = remaining[255:]
    lacing.append(len(remaining))

    pages: list[bytes] = []
    sequence = first_sequence
    data_offset = 0
    segment_offset = 0

    while segment_offset < len(lacing):
        chunk = lacing[segment_offset : segment_offset + 255]
        payload_length = sum(chunk)
        payload = packet[data_offset : data_offset + payload_length]
        data_offset += payload_length

        header_type = 0x01 if segment_offset > 0 else 0x00
        page = OggPage(
            header_type=header_type,
            granule=0,
            serial=serial,
            sequence=sequence,
            segments=bytes(chunk),
            payload=payload,
        )
        pages.append(page.to_bytes())
        sequence += 1
        segment_offset += len(chunk)

    return pages, sequence


def rewrite_serial(path: Path, serial: int) -> None:
    """Fixa o número de série do fluxo Ogg (o ffmpeg sorteia um a cada execução).

    Sem isso o script não seria idempotente: o mesmo áudio geraria bytes
    diferentes a cada rodada.
    """
    pages = OggPage.parse_all(path.read_bytes())
    output = bytearray()
    for page in pages:
        page.serial = serial
        output += page.to_bytes()
    path.write_bytes(bytes(output))


def inject_cover(input_path: Path, picture_block: bytes) -> None:
    data = input_path.read_bytes()
    pages = OggPage.parse_all(data)
    if len(pages) < 2:
        fail(f"arquivo Opus inesperado (poucas páginas): {input_path.name}")

    head = pages[0]
    if not head.payload.startswith(b"OpusHead"):
        fail(f"primeira página não é OpusHead: {input_path.name}")

    # Coleta as páginas que contêm o pacote OpusTags (headers ficam em páginas
    # separadas das páginas de áudio no muxer do ffmpeg).
    tags_pages: list[OggPage] = []
    index = 1
    while index < len(pages):
        page = pages[index]
        tags_pages.append(page)
        index += 1
        if page.segments and page.segments[-1] < 255:
            break
    else:
        fail(f"pacote OpusTags incompleto em {input_path.name}")

    tags_packet = b"".join(page.payload for page in tags_pages)
    if not tags_packet.startswith(b"OpusTags"):
        fail(f"segundo pacote não é OpusTags: {input_path.name}")

    comment = b"METADATA_BLOCK_PICTURE=" + base64.b64encode(picture_block)
    new_packet = append_opus_tags_comment(tags_packet, comment)

    serial = head.serial
    new_tags_pages, next_sequence = paginate_packet(new_packet, serial, head.sequence + 1)

    output = bytearray(head.to_bytes())
    for page in new_tags_pages:
        output += page

    for page in pages[index:]:
        output += page.to_bytes(sequence=next_sequence)
        next_sequence += 1

    input_path.write_bytes(bytes(output))


# ---------------------------------------------------------------------------
# Geração via ffmpeg
# ---------------------------------------------------------------------------


def require_ffmpeg() -> str:
    ffmpeg = shutil.which("ffmpeg")
    if ffmpeg is None:
        fail(
            "ffmpeg não encontrado no PATH. Instale o ffmpeg (com libopus e libvorbis) "
            "e rode novamente: https://ffmpeg.org/download.html"
        )
    return ffmpeg


def run_ffmpeg(ffmpeg: str, args: list[str]) -> None:
    command = [ffmpeg, "-hide_banner", "-loglevel", "error", "-y", *args]
    try:
        subprocess.run(
            command,
            check=True,
            timeout=FFMPEG_TIMEOUT_SECONDS,
            capture_output=True,
            text=True,
        )
    except subprocess.TimeoutExpired:
        fail(f"ffmpeg excedeu {FFMPEG_TIMEOUT_SECONDS}s: {' '.join(command)}")
    except subprocess.CalledProcessError as error:
        fail(
            "ffmpeg falhou\n"
            f"  comando: {' '.join(command)}\n"
            f"  stderr: {error.stderr.strip()}"
        )


def opus_tone(
    ffmpeg: str,
    target: Path,
    frequency: int,
    duration: float,
    metadata: dict[str, str],
) -> None:
    args = [
        "-f",
        "lavfi",
        "-i",
        f"sine=frequency={frequency}:duration={duration}:sample_rate=48000",
        "-c:a",
        "libopus",
        "-b:a",
        "128k",
        "-vbr",
        "on",
        "-ar",
        "48000",
        "-ac",
        "1",
    ]
    for key, value in metadata.items():
        args.extend(["-metadata", f"{key}={value}"])
    args.append(str(target))
    run_ffmpeg(ffmpeg, args)


def generate_cover(path: Path) -> None:
    try:
        from PIL import Image
    except ImportError:
        fail("Pillow não encontrado. Instale com: python -m pip install Pillow")

    width = height = 128
    image = Image.new("RGB", (width, height))
    pixels = image.load()
    for y in range(height):
        for x in range(width):
            pixels[x, y] = (30 + x, 60 + y // 2, 120 + (x + y) % 100)
    image.save(path, format="PNG", optimize=True)


def generate_broken(path: Path) -> None:
    """Bytes determinísticos que não são áudio válido (magic bytes desconhecidos)."""
    seed = 0x5A
    payload = bytearray(b"ZUNO-BROKEN-FIXTURE\n")
    while len(payload) < 4096:
        seed = (seed * 1103515245 + 12345) & 0x7FFF_FFFF
        payload.append((seed >> 16) & 0xFF)
    path.write_bytes(bytes(payload))


def main() -> None:
    ffmpeg = require_ffmpeg()
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    print(f"gerando fixtures em {OUT_DIR}")

    with tempfile.TemporaryDirectory(prefix="zuno-fixtures-") as temporary_dir:
        temporary = Path(temporary_dir)
        cover = temporary / "cover.png"
        generate_cover(cover)
        print(f"capa gerada: {cover.name} ({cover.stat().st_size} bytes)")

        tone_a = OUT_DIR / "tone-a.opus"
        opus_tone(
            ffmpeg,
            tone_a,
            frequency=440,
            duration=1.5,
            metadata={
                "title": "Tone A",
                "artist": "Zuno Test",
                "album_artist": "Zuno Test",
                "album": "Fixture Album",
                "track": "1",
                "disc": "1",
                "date": "2024",
            },
        )
        inject_cover(
            tone_a,
            flac_picture_block(cover.read_bytes(), "image/png", width=128, height=128),
        )
        print(f"tone-a.opus: capa embutida ({tone_a.stat().st_size} bytes)")

        opus_tone(
            ffmpeg,
            OUT_DIR / "tone-b.opus",
            frequency=660,
            duration=1.5,
            metadata={
                "title": "Tone B",
                "artist": "Zuno Test",
                "album": "Other Album",
                "track": "2",
            },
        )

        opus_tone(
            ffmpeg,
            OUT_DIR / "no-tags.opus",
            frequency=523,
            duration=1.0,
            metadata={},
        )

        # Cópia byte a byte para exercitar a detecção de duplicatas (feita
        # depois da normalização do serial, para continuar idêntica).
        run_ffmpeg(
            ffmpeg,
            [
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=330:duration=1.0:sample_rate=44100",
                "-c:a",
                "libvorbis",
                "-q:a",
                "3",
                "-metadata",
                "title=Vorbis Fixture",
                "-metadata",
                "artist=Zuno Test",
                str(OUT_DIR / "vorbis.ogg"),
            ],
        )

        generate_broken(OUT_DIR / "broken.opus")

        # Serial determinístico para que rodar o script de novo reproduza os
        # mesmos bytes (o hash das fixtures pode ser usado em outros testes).
        for index, name in enumerate(
            [
                "tone-a.opus",
                "tone-b.opus",
                "no-tags.opus",
                "vorbis.ogg",
            ]
        ):
            rewrite_serial(OUT_DIR / name, 0x5A00_0001 + index)

        shutil.copyfile(tone_a, OUT_DIR / "duplicate-a.opus")

    expected = [
        "tone-a.opus",
        "tone-b.opus",
        "no-tags.opus",
        "duplicate-a.opus",
        "vorbis.ogg",
        "broken.opus",
    ]

    missing = [name for name in expected if not (OUT_DIR / name).is_file()]
    if missing:
        fail(f"fixtures ausentes após a geração: {', '.join(missing)}")

    empty = [name for name in expected if (OUT_DIR / name).stat().st_size == 0]
    if empty:
        fail(f"fixtures vazias: {', '.join(empty)}")

    total = 0
    for name in expected:
        path = OUT_DIR / name
        size = path.stat().st_size
        total += size
        print(f"  {name:<18} {size:>7} bytes")

    print(f"total: {total} bytes")
    if total > MAX_TOTAL_BYTES:
        fail(
            f"fixtures somam {total} bytes, acima do limite de {MAX_TOTAL_BYTES} bytes"
        )

    if (OUT_DIR / "tone-a.opus").read_bytes() != (OUT_DIR / "duplicate-a.opus").read_bytes():
        fail("duplicate-a.opus não é uma cópia byte a byte de tone-a.opus")

    print("fixtures prontas")


if __name__ == "__main__":
    main()
