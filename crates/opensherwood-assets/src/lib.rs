//! Locating the player's game installation and resolving logical asset paths through overlays.
//!
//! Lookup order for a logical path such as `Data/Text/Level.res`: mod overlays (configured order),
//! then language overlays (`<id>/data/...`, e.g. `2047/data`), then the base `DATA/` tree.
//! All lookups are case-insensitive on every platform (see `docs/architecture.md`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

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
            index,
            sizes,
        })
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
    for entry in rd {
        let entry = entry.map_err(|source| AssetError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let logical = format!("{logical_prefix}/{name}");
        let meta = entry.metadata().map_err(|source| AssetError::Io {
            path: path.clone(),
            source,
        })?;
        if meta.is_dir() {
            walk(&path, &logical, index, sizes)?;
        } else {
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

    /// Content fingerprint: BLAKE3 over layer names, logical paths and sizes (no file contents, so it is
    /// fast; it distinguishes editions, languages and mods, which is what replays need).
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(b"opensherwood-content-fingerprint-v1\0");
        for layer in &self.layers {
            h.update(layer.name.as_bytes());
            h.update(b"\0");
            for (p, size) in layer.paths() {
                h.update(p.as_bytes());
                h.update(b"\0");
                h.update(&size.to_le_bytes());
            }
        }
        h.finalize().to_hex().to_string()
    }
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
        let fp = g.fingerprint();
        assert_eq!(fp.len(), 64);
        assert_eq!(fp, GameDir::open(&root).unwrap().fingerprint());
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
