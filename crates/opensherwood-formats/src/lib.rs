//! Parsers for the data files of *Robin Hood: The Legend of Sherwood*.
//!
//! Every parser is a pure function from bytes to typed structures. Nothing here does I/O policy
//! (that is `opensherwood-assets`) or game logic (`opensherwood-core`). Field names follow the specs in
//! `docs/formats/`; fields whose meaning is not established are called `unknown_*`.
//!
//! All parsers must be safe on arbitrary input: they return [`FormatError`] instead of panicking.

pub mod anim_table;
pub mod chunk;
pub mod dic;
pub mod font;
pub mod image_blob;
pub mod reader;
pub mod rhm;
pub mod rhp;
pub mod rhs;
pub mod scb;
pub mod sprite_decode;
pub mod sres;

pub use reader::{FormatError, Reader};

/// Kinds of files recognised by [`detect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// `SRES` resource archive (`docs/formats/sres.md`).
    Sres,
    /// `MEUH` map container (`docs/formats/rhp.md`).
    Rhp,
    /// `DUTY` mission container (`docs/formats/rhm.md`).
    Rhm,
    /// `SBSCRIPT` compiled script (`docs/formats/scb.md`).
    Scb,
    /// Sprite profile or dictionary starting with the bank generation id (`docs/formats/sprites.md`).
    SpriteBank,
    /// `SBFONT` bitmap font.
    BitmapFont,
    /// `SBTTFT` TrueType descriptor.
    TrueTypeFont,
    /// `FXBK` effect table.
    Fxbk,
    /// `SFPK` sound pack.
    Sfpk,
    /// `NEUF` remark table.
    Neuf,
    /// `GSHR` save game.
    SaveGame,
    /// `FORP` profile list.
    Profiles,
    /// `BIKi` Bink video.
    Bink,
    /// Compressed 16-bit picture with a 12-byte header (`docs/formats/image-blob.md`).
    ImageBlob,
    /// Not recognised.
    Unknown,
}

/// Guess the kind of a file from its first bytes (and, for image blobs, its size).
pub fn detect(data: &[u8]) -> FileKind {
    if data.len() >= 8 && &data[..8] == b"SBSCRIPT" {
        return FileKind::Scb;
    }
    if data.len() >= 6 {
        match &data[..6] {
            b"SBFONT" => return FileKind::BitmapFont,
            b"SBTTFT" => return FileKind::TrueTypeFont,
            _ => {}
        }
    }
    if data.len() >= 4 {
        match &data[..4] {
            b"SRES" => return FileKind::Sres,
            b"MEUH" => return FileKind::Rhp,
            b"DUTY" => return FileKind::Rhm,
            b"FXBK" => return FileKind::Fxbk,
            b"SFPK" => return FileKind::Sfpk,
            b"NEUF" => return FileKind::Neuf,
            b"GSHR" => return FileKind::SaveGame,
            b"FORP" => return FileKind::Profiles,
            b"BIKi" => return FileKind::Bink,
            _ => {}
        }
        if data[..4] == dic::BANK_GENERATION_ID.to_le_bytes() {
            return FileKind::SpriteBank;
        }
    }
    if image_blob::looks_like_image_blob(data) {
        return FileKind::ImageBlob;
    }
    FileKind::Unknown
}
