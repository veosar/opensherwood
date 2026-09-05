# Movies (`2047/data/Cinematics/*.vid`)

Status: **identified, not decoded** (2026-09-05). The two cinematics are Bink 1 files under a `.vid` extension
(the game ships `binkw32.dll`; the "Show movies" menu entry and the outro play them).

| File | Size | Magic | Frames | Size | Rate |
|---|---|---|---|---|---|
| `Intro.vid` | 68,707,752 bytes | `BIKi` | 4292 (~172 s) | 640x320 | 25 fps |
| `Outro.vid` | 45,827,336 bytes | `BIKi` | 2739 (~110 s) | 640x320 | 25 fps |

Header fields read (little-endian, offsets as in the public Bink container description): `BIKi` at 0, file
size minus 8 at 4, frame count at 8, largest frame size at 12, frame count again at 16, width at 20,
height at 24, frame rate numerator at 28 (25) with denominator 1 at 32. Audio tracks and the frame index
follow; not read yet.

## Decoding path (decision pending, ADR to come)

The format is proprietary but publicly documented by third parties (the MultimediaWiki Bink pages and
FFmpeg's decoder). A decoder written in Rust from those descriptions is clean-room compliant here: the
provenance rule of ADR-0003 concerns this game's executable, not a codec documented elsewhere. Options,
in order of preference:

1. A Rust Bink 1 video + audio decoder written from the public description (`opensherwood-formats` or a
   separate crate); several weeks of work, no third-party licence question.
2. NihAV's RAD decoders (Rust; the project states its licence as AGPL-3.0 and says other free licences can
   be discussed with the author: https://nihav.org/intro.html, https://codecs.multimedia.cx/2020/07/nihav-released/):
   a Git dependency; the AGPL is GPLv3-compatible under GPLv3 section 13 but would attach its network
   clause to the combined work; to be decided by the maintainer after reading the upstream licence files.
3. Shelling out to a system `ffmpeg` for a pre-decoded frame stream: works on developer machines, not a
   shippable player experience.

Until one lands, "Show movies" stays a disabled plate and the outro level is reached without its movie; the
harness marks both as not implemented (`ui.items[].enabled = false`).

## Provenance

Observation only (2026-09-05, lead session), GOG English build (executable SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`): the first 36 bytes of each file read
with `xxd -l 36 <file>` from the player's copy under `2047/data/Cinematics/`, field meanings from the public
container description (MultimediaWiki, "Bink Container", https://wiki.multimedia.cx/index.php/Bink_Container;
the codec page https://wiki.multimedia.cx/index.php/Bink_Video); file sizes from the directory listing;
frame counts and rate converted from the header words. No executable analysis. No test depends on this file.
Status of every claim: `observed` for the table and the header fields, `inferred` for the play-back places
(menu entry, outro).
