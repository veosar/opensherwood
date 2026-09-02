//! Locating the player's game installation and resolving logical asset paths through overlays.
//!
//! Lookup order for a logical path such as `Data/Text/Level.res`: mod overlays (configured order),
//! then language overlays (`<id>/data/...`, e.g. `2047/data`), then the base `DATA/` tree.
//! All lookups are case-insensitive on every platform (see `docs/architecture.md`).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use thiserror::Error;

pub mod sprites;
pub use sprites::{SpriteBank, SpriteImage};

/// Errors of this crate.
#[derive(Debug, Error)]
pub enum AssetError {
    /// No installation could be found.
    #[error("game directory not found; pass --game-dir or set OPENSHERWOOD_GAME_DIR")]
    NotFound,
    /// The directory exists but does not look like the game.
    #[error("{0} does not contain DATA/robinhood.bks")]
    NotAGameDir(PathBuf),
    /// A logical path did not resolve.
    #[error("asset not found: {0}")]
    Missing(String),
    /// Two files differ only by case (or Unicode form): the lookup would be ambiguous.
    #[error("case-insensitive name collision: {0}")]
    Collision(String),
    /// A file name is not valid UTF-8.
    #[error("file name is not valid UTF-8: {0}")]
    BadName(PathBuf),
    /// A file was found but could not be parsed.
    #[error("{path}: {message}")]
    Format {
        /// Logical path.
        path: String,
        /// Parser message.
        message: String,
    },
    /// I/O failure.
    #[error("io error on {path}: {source}")]
    Io {
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
}

/// One layer of the VFS: a root directory whose files are indexed case-insensitively.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Display name (`base`, `lang:2047`, `mod:<name>`).
    pub name: String,
    /// Directory that corresponds to the logical root (the parent of `Data`).
    pub root: PathBuf,
    /// Real name of the `Data` child (its case as on disk).
    data_dir_name: String,
    /// lowercase logical path -> real path.
    index: BTreeMap<String, PathBuf>,
    /// lowercase logical path -> size, for fingerprinting.
    sizes: BTreeMap<String, u64>,
}

impl Layer {
    fn build(name: String, root: PathBuf, data_dir_name: &str) -> Result<Self, AssetError> {
        let mut index = BTreeMap::new();
        let mut sizes = BTreeMap::new();
        let data_dir = root.join(data_dir_name);
        walk(&data_dir, "data", &mut index, &mut sizes)?;
        Ok(Self {
            name,
            root,
            data_dir_name: data_dir_name.to_string(),
            index,
            sizes,
        })
    }

    /// Walk the directory again: the layer as it is on disk now (files added or removed since
    /// this layer was indexed included).
    pub fn rescan(&self) -> Result<Self, AssetError> {
        Self::build(self.name.clone(), self.root.clone(), &self.data_dir_name)
    }

    /// Resolve a logical path (`Data/Levels/x.rhm`, any case, `/` or `\`) in this layer.
    #[must_use]
    pub fn resolve(&self, logical: &str) -> Option<&Path> {
        self.index.get(&normalise(logical)).map(PathBuf::as_path)
    }

    /// Logical paths present in this layer, sorted.
    pub fn paths(&self) -> impl Iterator<Item = (&str, u64)> {
        self.sizes.iter().map(|(k, v)| (k.as_str(), *v))
    }
}

fn walk(
    dir: &Path,
    logical_prefix: &str,
    index: &mut BTreeMap<String, PathBuf>,
    sizes: &mut BTreeMap<String, u64>,
) -> Result<(), AssetError> {
    let rd = std::fs::read_dir(dir).map_err(|source| AssetError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    // Sort by the raw file name so the index never depends on filesystem enumeration order.
    let mut entries: Vec<std::fs::DirEntry> =
        rd.collect::<Result<_, _>>()
            .map_err(|source| AssetError::Io {
                path: dir.to_path_buf(),
                source,
            })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_lowercase) else {
            return Err(AssetError::BadName(path));
        };
        let logical = format!("{logical_prefix}/{name}");
        let meta = entry.metadata().map_err(|source| AssetError::Io {
            path: path.clone(),
            source,
        })?;
        if meta.is_dir() {
            walk(&path, &logical, index, sizes)?;
        } else {
            if index.contains_key(&logical) {
                return Err(AssetError::Collision(logical));
            }
            sizes.insert(logical.clone(), meta.len());
            index.insert(logical, path);
        }
    }
    Ok(())
}

fn normalise(logical: &str) -> String {
    logical
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_lowercase()
}

/// Version tag mixed into the content fingerprint; bump whenever what is hashed changes.
const FINGERPRINT_VERSION: &[u8] = b"opensherwood-content-fingerprint-v3\0";

/// Read buffer of the streaming digest.
const DIGEST_CHUNK: usize = 1 << 20;

/// Times a file that changes while it is being hashed is read again before the fingerprint fails.
const DIGEST_ATTEMPTS: usize = 3;

/// What two `stat`s of a file are compared on to decide whether the bytes streamed between them
/// were one consistent version: size, modification time and, where the platform reports one, the
/// inode change time (Unix) or the creation time and attributes (Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    len: u64,
    modified: Option<SystemTime>,
    extra: (i64, i64),
}

impl FileIdentity {
    fn of(meta: &std::fs::Metadata) -> Self {
        #[cfg(unix)]
        let extra = {
            use std::os::unix::fs::MetadataExt;
            (meta.ctime(), meta.ctime_nsec())
        };
        #[cfg(windows)]
        let extra = {
            use std::os::windows::fs::MetadataExt;
            (
                i64::try_from(meta.creation_time()).unwrap_or(i64::MAX),
                i64::from(meta.file_attributes()),
            )
        };
        #[cfg(not(any(unix, windows)))]
        let extra = (0, 0);
        Self {
            len: meta.len(),
            modified: meta.modified().ok(),
            extra,
        }
    }
}

/// The player's installation plus overlays.
#[derive(Debug, Clone)]
pub struct GameDir {
    /// Installation root (contains `DATA/`).
    pub root: PathBuf,
    /// Layers in lookup order (highest priority first).
    pub layers: Vec<Layer>,
}

impl GameDir {
    /// Open an installation root.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, AssetError> {
        let root: PathBuf = root.into();
        if !root.join("DATA").join("robinhood.bks").is_file()
            && !root.join("Data").join("robinhood.bks").is_file()
        {
            // case-insensitive check
            let has = std::fs::read_dir(&root)
                .ok()
                .into_iter()
                .flatten()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("data"));
            if !has {
                return Err(AssetError::NotAGameDir(root));
            }
        }
        let data_name =
            find_child_ci(&root, "data").ok_or_else(|| AssetError::NotAGameDir(root.clone()))?;
        let base = Layer::build("base".into(), root.clone(), &data_name)?;
        if base.resolve("Data/robinhood.bks").is_none() {
            return Err(AssetError::NotAGameDir(root));
        }
        let mut layers = Vec::new();
        // Language overlays: numeric directories containing a `data` child.
        let mut langs: Vec<(String, PathBuf)> = std::fs::read_dir(&root)
            .map_err(|source| AssetError::Io {
                path: root.clone(),
                source,
            })?
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let p = e.path();
                (p.is_dir()
                    && name.chars().all(|c| c.is_ascii_digit())
                    && find_child_ci(&p, "data").is_some())
                .then_some((name, p))
            })
            .collect();
        langs.sort();
        for (name, p) in langs {
            let data_name = find_child_ci(&p, "data").unwrap_or_else(|| "data".into());
            layers.push(Layer::build(format!("lang:{name}"), p, &data_name)?);
        }
        layers.push(base);
        Ok(Self { root, layers })
    }

    /// Discover the installation: explicit path, then `OPENSHERWOOD_GAME_DIR`, then well-known locations.
    pub fn discover(explicit: Option<&Path>) -> Result<Self, AssetError> {
        if let Some(p) = explicit {
            return Self::open(p);
        }
        if let Some(p) = std::env::var_os("OPENSHERWOOD_GAME_DIR") {
            return Self::open(PathBuf::from(p));
        }
        for candidate in well_known_locations() {
            if let Ok(g) = Self::open(&candidate) {
                return Ok(g);
            }
        }
        Err(AssetError::NotFound)
    }

    /// Add a mod overlay directory (its `Data` child is indexed) with highest priority.
    pub fn push_mod(&mut self, name: &str, root: impl Into<PathBuf>) -> Result<(), AssetError> {
        let root: PathBuf = root.into();
        let data_name =
            find_child_ci(&root, "data").ok_or_else(|| AssetError::NotAGameDir(root.clone()))?;
        let layer = Layer::build(format!("mod:{name}"), root, &data_name)?;
        self.layers.insert(0, layer);
        Ok(())
    }

    /// Resolve a logical path through the layers.
    #[must_use]
    pub fn resolve(&self, logical: &str) -> Option<(&Layer, &Path)> {
        self.layers
            .iter()
            .find_map(|l| l.resolve(logical).map(|p| (l, p)))
    }

    /// Read a logical path.
    pub fn read(&self, logical: &str) -> Result<Vec<u8>, AssetError> {
        let (_, path) = self
            .resolve(logical)
            .ok_or_else(|| AssetError::Missing(logical.to_string()))?;
        std::fs::read(path).map_err(|source| AssetError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Content fingerprint: BLAKE3 over layer names (in precedence order), logical paths, sizes and
    /// the full-content BLAKE3 digest of every file under every layer's `Data` directory, so any
    /// edited, added, removed or replaced byte changes the fingerprint. Every call walks the
    /// directories again and streams every file in full: nothing is cached, because a file's size
    /// and timestamps cannot prove its bytes unchanged (a same-size edit with a preserved
    /// modification time is invisible to them, and the change time is not available through the
    /// standard library on every platform). On the retail installation (about 1 GiB) a call takes
    /// under a second once the files are in the page cache, a few seconds cold. Each file is
    /// `stat`ed before and after it is read and hashed again when the two disagree
    /// (`DIGEST_ATTEMPTS` times), so a file replaced concurrently is not hashed as a mix of two
    /// versions. A file that cannot be opened or read makes the whole fingerprint an error: a
    /// partial or silently degraded fingerprint would defeat its purpose (replays and goldens must
    /// never be compared across different data). The lookup index built by [`GameDir::open`] is
    /// not refreshed by this call: a file added after `open` is part of the fingerprint but does
    /// not resolve until the directory is opened again.
    pub fn fingerprint(&self) -> Result<String, AssetError> {
        let mut h = blake3::Hasher::new();
        h.update(FINGERPRINT_VERSION);
        let mut buf = vec![0u8; DIGEST_CHUNK];
        for layer in &self.layers {
            // A fresh walk: files added or removed since the layer was indexed are part of the
            // identity, and a name collision that appeared since is an error like at `open`.
            let current = layer.rescan()?;
            h.update(current.name.as_bytes());
            h.update(b"\0");
            for (p, _) in current.paths() {
                let real = current
                    .resolve(p)
                    .expect("every indexed path resolves in its own layer");
                let (size, digest) = file_digest(real, &mut buf)?;
                h.update(p.as_bytes());
                h.update(b"\0");
                h.update(&size.to_le_bytes());
                h.update(&digest);
            }
        }
        Ok(h.finalize().to_hex().to_string())
    }
}

/// Size and full BLAKE3 digest of one file as one consistent version: the file's identity
/// (size, timestamps) is taken before and after streaming it, the bytes read must match the
/// size, and a disagreement means the file was being replaced, so it is read again (up to
/// `DIGEST_ATTEMPTS` times) before it is reported as unreadable.
fn file_digest(real: &Path, buf: &mut [u8]) -> Result<(u64, [u8; 32]), AssetError> {
    let io = |source| AssetError::Io {
        path: real.to_path_buf(),
        source,
    };
    let mut last = String::new();
    for _ in 0..DIGEST_ATTEMPTS {
        let before = FileIdentity::of(&std::fs::metadata(real).map_err(io)?);
        let mut f = std::fs::File::open(real).map_err(io)?;
        let mut fh = blake3::Hasher::new();
        let mut total = 0u64;
        loop {
            let n = f.read(buf).map_err(io)?;
            if n == 0 {
                break;
            }
            fh.update(&buf[..n]);
            total += n as u64;
        }
        drop(f);
        let after = FileIdentity::of(&std::fs::metadata(real).map_err(io)?);
        if total == before.len && after == before {
            return Ok((before.len, *fh.finalize().as_bytes()));
        }
        last = format!(
            "{} bytes expected, {total} read, metadata {}",
            before.len,
            if after == before {
                "unchanged"
            } else {
                "changed"
            }
        );
    }
    Err(io(std::io::Error::other(format!(
        "file changed while hashing ({last})"
    ))))
}

fn find_child_ci(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let n = e.file_name().to_string_lossy().to_string();
        (n.eq_ignore_ascii_case(name) && e.path().is_dir()).then_some(n)
    })
}

fn well_known_locations() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for base in [
        "C:\\GOG Games",
        "C:\\Program Files (x86)\\GOG Galaxy\\Games",
        "C:\\Program Files (x86)\\Steam\\steamapps\\common",
        "C:\\Program Files\\Steam\\steamapps\\common",
    ] {
        v.push(Path::new(base).join("Robin Hood - The Legend of Sherwood"));
        v.push(Path::new(base).join("Robin Hood"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        v.push(home.join(".steam/steam/steamapps/common/Robin Hood - The Legend of Sherwood"));
        v.push(
            home.join(".local/share/Steam/steamapps/common/Robin Hood - The Legend of Sherwood"),
        );
        v.push(home.join("Games/robin-hood"));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_install() -> (tempdir::TempDir, PathBuf) {
        let td = tempdir::TempDir::new("opensherwood").unwrap();
        let root = td.path().join("game");
        std::fs::create_dir_all(root.join("DATA/Text")).unwrap();
        std::fs::write(root.join("DATA/robinhood.bks"), b"x").unwrap();
        std::fs::write(root.join("DATA/Text/actors.res"), b"base").unwrap();
        std::fs::create_dir_all(root.join("2047/data/Text")).unwrap();
        std::fs::write(root.join("2047/data/Text/Level.res"), b"lang").unwrap();
        (td, root)
    }

    #[test]
    fn resolves_case_insensitively_through_overlays() {
        let (_td, root) = fake_install();
        let g = GameDir::open(&root).unwrap();
        assert_eq!(g.layers.len(), 2);
        assert_eq!(g.layers[0].name, "lang:2047");
        assert_eq!(g.read("Data\\TEXT\\level.RES").unwrap(), b"lang");
        assert_eq!(g.read("data/text/ACTORS.res").unwrap(), b"base");
        assert!(g.read("data/nope").is_err());
        let fp = g.fingerprint().unwrap();
        assert_eq!(fp.len(), 64);
        assert_eq!(fp, GameDir::open(&root).unwrap().fingerprint().unwrap());
    }

    #[test]
    fn fingerprint_sees_a_same_size_edit_with_a_preserved_mtime() {
        let (_td, root) = fake_install();
        // A file larger than the digest chunk with an edit in the middle only (same size).
        let big = root.join("DATA/big.bin");
        let mut data = vec![7u8; 3 * DIGEST_CHUNK + 123];
        std::fs::write(&big, &data).unwrap();
        let g = GameDir::open(&root).unwrap();
        let before = g.fingerprint().unwrap();
        assert_eq!(g.fingerprint().unwrap(), before, "stable across calls");
        let mtime = std::fs::metadata(&big).unwrap().modified().unwrap();
        data[DIGEST_CHUNK + DIGEST_CHUNK / 2] = 8;
        std::fs::write(&big, &data).unwrap();
        // Put the original modification time back: size and mtime now look untouched.
        std::fs::File::options()
            .write(true)
            .open(&big)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        let meta = std::fs::metadata(&big).unwrap();
        assert_eq!(meta.len(), data.len() as u64);
        assert_eq!(meta.modified().unwrap(), mtime);
        let after = g.fingerprint().unwrap();
        assert_ne!(
            before, after,
            "a same-size, same-mtime middle edit must change the fingerprint"
        );
        // A fresh GameDir agrees.
        assert_eq!(GameDir::open(&root).unwrap().fingerprint().unwrap(), after);
    }

    #[test]
    fn fingerprint_sees_files_added_and_removed_after_open() {
        let (_td, root) = fake_install();
        let g = GameDir::open(&root).unwrap();
        let before = g.fingerprint().unwrap();
        let extra = root.join("DATA/Text/extra.res");
        std::fs::write(&extra, b"new").unwrap();
        let added = g.fingerprint().unwrap();
        assert_ne!(before, added, "an added file must change the fingerprint");
        assert_eq!(GameDir::open(&root).unwrap().fingerprint().unwrap(), added);
        // The lookup index is the one built at `open`: the new file resolves only after reopening.
        assert!(g.resolve("Data/Text/extra.res").is_none());
        assert!(
            GameDir::open(&root)
                .unwrap()
                .resolve("Data/Text/extra.res")
                .is_some()
        );
        // A file added in the language overlay counts too, and removing files restores the value.
        let overlay = root.join("2047/data/Text/more.res");
        std::fs::write(&overlay, b"lang2").unwrap();
        let both = g.fingerprint().unwrap();
        assert_ne!(both, added);
        std::fs::remove_file(&overlay).unwrap();
        assert_eq!(g.fingerprint().unwrap(), added);
        std::fs::remove_file(&extra).unwrap();
        assert_eq!(g.fingerprint().unwrap(), before);
    }

    #[test]
    fn fingerprint_fails_when_a_file_cannot_be_read() {
        let (_td, root) = fake_install();
        let g = GameDir::open(&root).unwrap();
        let victim = root.join("DATA/Text/actors.res");
        // Make the file unreadable: an exclusive open on Windows, no permissions on Unix.
        #[cfg(windows)]
        let _lock = {
            use std::os::windows::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&victim)
                .unwrap()
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&victim, std::fs::Permissions::from_mode(0)).unwrap();
            if std::fs::File::open(&victim).is_ok() {
                // Running as root: permissions do not bite, nothing to verify here.
                return;
            }
        }
        assert!(matches!(g.fingerprint(), Err(AssetError::Io { .. })));
    }

    #[test]
    fn rejects_non_game_dir() {
        let td = tempdir::TempDir::new("opensherwood").unwrap();
        assert!(matches!(
            GameDir::open(td.path()),
            Err(AssetError::NotAGameDir(_))
        ));
    }
}
