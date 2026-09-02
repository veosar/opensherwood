//! Sprite bank access: the frame table and dictionary pages are kept in memory, frame streams are
//! read from `robinhood.bks` on demand and decoded frames are cached.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;

use opensherwood_formats::dic::{self, FrameRecord};
use opensherwood_formats::rhs;
use opensherwood_formats::sprite_decode::{self, Pages};

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

/// The open sprite bank.
pub struct SpriteBank {
    frames: Vec<FrameRecord>,
    pages: Pages,
    bks: File,
    bks_path: PathBuf,
    cache: HashMap<u32, Arc<SpriteImage>>,
    cache_bytes: usize,
    /// Maximum bytes of decoded frames kept in the cache before it is cleared.
    pub cache_limit: usize,
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
            cache_bytes: 0,
            cache_limit: 256 * 1024 * 1024,
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

    /// Decode (or fetch from cache) one frame.
    pub fn frame(&mut self, index: u32) -> Result<Arc<SpriteImage>, AssetError> {
        if let Some(f) = self.cache.get(&index) {
            return Ok(f.clone());
        }
        let rec = *self
            .record(index)
            .ok_or_else(|| AssetError::Missing(format!("sprite frame {index}")))?;
        let mut stream = vec![0u8; rec.length as usize];
        self.bks
            .seek(SeekFrom::Start(u64::from(rec.offset)))
            .and_then(|_| self.bks.read_exact(&mut stream))
            .map_err(|source| AssetError::Io {
                path: self.bks_path.clone(),
                source,
            })?;
        let img = sprite_decode::decode_frame(&rec, &stream, &self.pages).map_err(|e| {
            AssetError::Format {
                path: format!("sprite frame {index}"),
                message: e.to_string(),
            }
        })?;
        let sprite = Arc::new(SpriteImage {
            width: u32::from(img.width),
            height: u32::from(img.height),
            rgba: sprite_decode::to_rgba8_keyed(&img),
        });
        if self.cache_bytes + sprite.rgba.len() > self.cache_limit {
            self.cache.clear();
            self.cache_bytes = 0;
        }
        self.cache_bytes += sprite.rgba.len();
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
