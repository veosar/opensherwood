//! Sprite bank access: the frame table and dictionary pages are kept in memory, frame streams are
//! read from `robinhood.bks` on demand and decoded frames are cached.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;

use opensherwood_formats::dic::{self, FrameRecord};
use opensherwood_formats::image_blob::RgbaBudget;
use opensherwood_formats::rhs;
use opensherwood_formats::sprite_decode::{self, DecodeLimits, Pages};

use crate::{AssetError, GameDir};

/// A decoded frame as RGBA8 (colour key -> alpha 0, shadow key -> semi-transparent black).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteImage {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Pixels.
    pub rgba: Vec<u8>,
}

/// Decode policy applied to every frame record when the bank is opened and again when a frame
/// is decoded (the same [`DecodeLimits::RETAIL`] the format crate and the tools use).
pub const LIMITS: DecodeLimits = DecodeLimits::RETAIL;
/// Largest frame dimension accepted (retail maximum is 674x583).
pub const MAX_FRAME_DIMENSION: u16 = LIMITS.max_dimension;
/// Largest frame stream accepted, in bytes (a 4096x4096 span frame is at most ~33 MiB).
pub const MAX_STREAM_BYTES: u32 = LIMITS.max_stream_bytes;
/// Bytes of RGBA8 frames the bank materialises before its cache is cleared: the cumulative
/// budget every decoded frame is charged to (the largest accepted frame, 4096x4096, is 64 MiB).
pub const CACHE_LIMIT: usize = 256 * 1024 * 1024;

/// The open sprite bank.
pub struct SpriteBank {
    frames: Vec<FrameRecord>,
    pages: Pages,
    bks: File,
    bks_path: PathBuf,
    cache: HashMap<u32, Arc<SpriteImage>>,
    /// RGBA bytes materialised and held by `cache`.
    budget: RgbaBudget,
}

impl std::fmt::Debug for SpriteBank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpriteBank")
            .field("frames", &self.frames.len())
            .field("pages", &self.pages.pages.len())
            .field("cached", &self.cache.len())
            .finish_non_exhaustive()
    }
}

impl SpriteBank {
    /// Open `Data/robinhood.dic` and `Data/robinhood.bks` from the game directory.
    pub fn open(game: &GameDir) -> Result<Self, AssetError> {
        let dic_data = game.read("Data/robinhood.dic")?;
        let dictionary = dic::parse(&dic_data).map_err(|e| AssetError::Format {
            path: "Data/robinhood.dic".into(),
            message: e.to_string(),
        })?;
        let pages = sprite_decode::parse_pages(&dictionary).map_err(|e| AssetError::Format {
            path: "Data/robinhood.dic".into(),
            message: e.to_string(),
        })?;
        let frames = dictionary.frames;
        let bks_len = game
            .resolve("Data/robinhood.bks")
            .and_then(|(_, p)| std::fs::metadata(p).ok())
            .map_or(0, |m| m.len());
        // Validate the whole table now so a corrupt record can never reach the decoder later.
        if pages.frame_count as usize != frames.len() {
            return Err(AssetError::Format {
                path: "Data/robinhood.dic".into(),
                message: format!(
                    "frame count {} does not match the table ({})",
                    pages.frame_count,
                    frames.len()
                ),
            });
        }
        for (i, f) in frames.iter().enumerate() {
            LIMITS.check_record(f).map_err(|e| AssetError::Format {
                path: "Data/robinhood.dic".into(),
                message: format!("frame record {i}: {e}"),
            })?;
            let end = u64::from(f.offset) + u64::from(f.length);
            let bad = end > bks_len
                || (f.page != dic::NO_PAGE && usize::from(f.page) >= pages.pages.len());
            if bad {
                return Err(AssetError::Format {
                    path: "Data/robinhood.dic".into(),
                    message: format!("frame record {i} is out of range"),
                });
            }
        }
        let (_, bks_path) = game
            .resolve("Data/robinhood.bks")
            .ok_or_else(|| AssetError::Missing("Data/robinhood.bks".into()))?;
        let bks_path = bks_path.to_path_buf();
        let bks = File::open(&bks_path).map_err(|source| AssetError::Io {
            path: bks_path.clone(),
            source,
        })?;
        Ok(Self {
            frames,
            pages,
            bks,
            bks_path,
            cache: HashMap::new(),
            budget: RgbaBudget::new(CACHE_LIMIT),
        })
    }

    /// Number of frames in the bank.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Frame record by index.
    #[must_use]
    pub fn record(&self, index: u32) -> Option<&FrameRecord> {
        self.frames.get(index as usize)
    }

    /// RGBA bytes of the frames currently cached.
    #[must_use]
    pub fn cached_bytes(&self) -> usize {
        self.budget.used()
    }

    /// Number of frames currently cached.
    #[must_use]
    pub fn cached_frames(&self) -> usize {
        self.cache.len()
    }

    /// Decode (or fetch from cache) one frame. The RGBA form is charged to the bank's budget
    /// ([`CACHE_LIMIT`]): when it does not fit next to the cached frames the cache is cleared
    /// first, and a frame that does not fit on its own is an error.
    pub fn frame(&mut self, index: u32) -> Result<Arc<SpriteImage>, AssetError> {
        if let Some(f) = self.cache.get(&index) {
            return Ok(f.clone());
        }
        let rec = *self
            .record(index)
            .ok_or_else(|| AssetError::Missing(format!("sprite frame {index}")))?;
        let mut stream = Vec::new();
        stream
            .try_reserve_exact(rec.length as usize)
            .map_err(|_| AssetError::Missing(format!("sprite frame {index} (allocation)")))?;
        stream.resize(rec.length as usize, 0);
        self.bks
            .seek(SeekFrom::Start(u64::from(rec.offset)))
            .and_then(|_| self.bks.read_exact(&mut stream))
            .map_err(|source| AssetError::Io {
                path: self.bks_path.clone(),
                source,
            })?;
        let img =
            sprite_decode::decode_frame_with(&rec, &stream, &self.pages, &LIMITS).map_err(|e| {
                AssetError::Format {
                    path: format!("sprite frame {index}"),
                    message: e.to_string(),
                }
            })?;
        let format = |e: opensherwood_formats::reader::FormatError| AssetError::Format {
            path: format!("sprite frame {index}"),
            message: e.to_string(),
        };
        let bytes = img.rgba8_len().map_err(format)?;
        if bytes > self.budget.remaining() {
            self.cache.clear();
            self.budget.reset();
        }
        let rgba =
            sprite_decode::to_rgba8_keyed_budgeted(&img, &mut self.budget).map_err(format)?;
        let sprite = Arc::new(SpriteImage {
            width: u32::from(img.width),
            height: u32::from(img.height),
            rgba,
        });
        self.cache.insert(index, sprite.clone());
        Ok(sprite)
    }

    /// Load and parse a character profile by base name (`Data/Characters/<name>.rhs`).
    pub fn load_profile(game: &GameDir, name: &str) -> Result<rhs::Profile, AssetError> {
        let logical = format!("Data/Characters/{name}.rhs");
        let data = game.read(&logical)?;
        rhs::parse(&data).map_err(|e| AssetError::Format {
            path: logical,
            message: e.to_string(),
        })
    }
}
