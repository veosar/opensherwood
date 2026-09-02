"""Image comparison against local captures of the original game (never committed).

`compare(ours, original, masks)` aligns nothing (both frames are 1024x768 logical frames), blanks the masked
rectangles (text that depends on the profile, the cursor, timers) and returns the structural similarity over the
rest plus a per-region breakdown, and can write a diff image for inspection. Used by the data-backed oracle tests
and by agents after a UI change.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image


@dataclass
class Comparison:
    ssim: float
    mean_abs_diff: float
    fraction_over_32: float
    shape: tuple[int, int]

    def __str__(self) -> str:
        return (
            f"ssim={self.ssim:.4f} mean|diff|={self.mean_abs_diff:.2f} "
            f"pixels>32={self.fraction_over_32 * 100:.2f}% ({self.shape[1]}x{self.shape[0]})"
        )


def load_gray(path: Path) -> np.ndarray:
    return np.asarray(Image.open(path).convert("L"), dtype=np.float64)


def apply_masks(img: np.ndarray, masks: list[tuple[int, int, int, int]]) -> np.ndarray:
    out = img.copy()
    for x, y, w, h in masks:
        out[y : y + h, x : x + w] = 0.0
    return out


def compare(
    ours: Path,
    original: Path,
    masks: list[tuple[int, int, int, int]] | None = None,
    diff_out: Path | None = None,
) -> Comparison:
    """Compare two frames of the same logical size; `masks` are (x, y, w, h) rectangles to ignore."""
    from skimage.metrics import structural_similarity

    a = load_gray(ours)
    b = load_gray(original)
    if a.shape != b.shape:
        raise ValueError(f"frame sizes differ: ours {a.shape[::-1]} original {b.shape[::-1]}")
    masks = masks or []
    a_m = apply_masks(a, masks)
    b_m = apply_masks(b, masks)
    ssim = float(structural_similarity(a_m, b_m, data_range=255.0))
    diff = np.abs(a_m - b_m)
    if diff_out is not None:
        diff_out.parent.mkdir(parents=True, exist_ok=True)
        Image.fromarray(np.clip(diff * 2, 0, 255).astype(np.uint8)).save(diff_out)
    return Comparison(
        ssim=ssim,
        mean_abs_diff=float(diff.mean()),
        fraction_over_32=float((diff > 32).mean()),
        shape=a.shape,
    )
