//! `opensherwood-tools`: inspect game files and export pictures for local viewing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use opensherwood_formats::{FileKind, chunk, detect, dic, image_blob, rhs, scb, sres};

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Detect the kind of a file and print a summary.
    Inspect { file: PathBuf },
    /// List the entries of an SRES archive.
    Sres { file: PathBuf },
    /// Dump the sequences and animations of a sprite profile.
    Rhs {
        file: PathBuf,
        /// Print every frame reference.
        #[arg(long)]
        frames: bool,
    },
    /// Statistics of the sprite dictionary.
    Dic { file: PathBuf },
    /// List the child chunks of an RHP or RHM container.
    Chunks { file: PathBuf },
    /// Export an image blob (.map/.min/.pak/.sxt/thumbnail) as PNG (RGB565 assumption).
    ExportImage {
        file: PathBuf,
        out: PathBuf,
        /// Which image of a multi-image .pak to export.
        #[arg(long, default_value_t = 0)]
        index: usize,
    },
    /// Export one picture entry of an SRES archive as PNG (first frame for collections).
    ExportSres {
        file: PathBuf,
        id: u32,
        out: PathBuf,
        #[arg(long, default_value_t = 0)]
        frame: usize,
    },
    /// Show the resolved game directory, its layers and fingerprint.
    GameDir {
        #[arg(long)]
        path: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    match Args::parse().cmd {
        Cmd::Inspect { file } => inspect(&file),
        Cmd::Sres { file } => {
            let a = sres::parse(&read(&file)?)?;
            println!(
                "SRES version {:#x}, {} entries, {} offsets in trailer",
                a.version,
                a.entries.len(),
                a.offsets.len()
            );
            for e in &a.entries {
                let desc = match &e.body {
                    sres::Body::Picture(p) => format!("{}x{}", p.width, p.height),
                    sres::Body::PictureCollection(v)
                    | sres::Body::Widget { pictures: v, .. }
                    | sres::Body::Cursor { frames: v, .. } => {
                        format!(
                            "{} pictures, first {}",
                            v.len(),
                            v.first()
                                .map_or(String::from("-"), |p| format!("{}x{}", p.width, p.height))
                        )
                    }
                    sres::Body::Text(v) => format!(
                        "{} strings, first {:?}",
                        v.len(),
                        v.first().map(|s| truncate(s, 60))
                    ),
                    sres::Body::Wave(v) => format!("{} paths, first {:?}", v.len(), v.first()),
                };
                println!(
                    "  {:#08x} {} id={:<8} {}",
                    e.offset,
                    e.body.kind(),
                    e.id,
                    desc
                );
            }
            Ok(())
        }
        Cmd::Rhs { file, frames } => {
            let p = rhs::parse(&read(&file)?)?;
            println!(
                "bank generation {:#x}, {} sequences",
                p.bank_generation,
                p.sequences.len()
            );
            for s in &p.sequences {
                let nframes: usize = s.animations.iter().map(|a| a.frames.len()).sum();
                println!(
                    "  {:?}: {}x{} unknown_0x26={} unknown_0x2a={} animations={} frame refs={}",
                    s.name,
                    s.width,
                    s.height,
                    s.unknown_0x26,
                    s.unknown_0x2a,
                    s.animations.len(),
                    nframes
                );
                if frames {
                    for (i, a) in s.animations.iter().enumerate() {
                        println!(
                            "    anim {i}: u02={} u04={} u08={} u0c={} frames={:?}",
                            a.unknown_0x02,
                            a.unknown_0x04,
                            a.unknown_0x08,
                            a.unknown_0x0c,
                            a.frames
                                .iter()
                                .map(|f| (f.frame, f.duration, f.anchor_x, f.anchor_y))
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
            Ok(())
        }
        Cmd::Dic { file } => {
            let data = read(&file)?;
            let d = dic::parse(&data)?;
            println!(
                "bank generation {:#x}, pages {}, symbols/page {}, dictionary region {} bytes, frames {}",
                d.bank_generation,
                d.page_count,
                d.symbols_per_page,
                d.dictionary_region.len(),
                d.frames.len()
            );
            let mut per_page: BTreeMap<u16, (usize, u64)> = BTreeMap::new();
            for f in &d.frames {
                let e = per_page.entry(f.page).or_default();
                e.0 += 1;
                e.1 += u64::from(f.length);
            }
            for (page, (n, bytes)) in per_page.iter().take(10) {
                println!("  page {page}: {n} frames, {bytes} bytes of symbols");
            }
            let maxw = d.frames.iter().map(|f| f.width).max().unwrap_or(0);
            let maxh = d.frames.iter().map(|f| f.height).max().unwrap_or(0);
            println!("  max frame {maxw}x{maxh}");
            Ok(())
        }
        Cmd::Chunks { file } => {
            let data = read(&file)?;
            let root = match detect(&data) {
                FileKind::Rhp => b"MEUH",
                FileKind::Rhm => b"DUTY",
                other => bail!("not a chunk container: {other:?}"),
            };
            let c = chunk::parse_container(&data, root)?;
            println!(
                "{} version {} with {} children",
                String::from_utf8_lossy(&c.tag),
                c.version,
                c.children.len()
            );
            for ch in &c.children {
                println!(
                    "  {:#08x} {:?} version {} body {} bytes",
                    ch.offset,
                    ch.tag_str(),
                    ch.version,
                    ch.body.len()
                );
            }
            Ok(())
        }
        Cmd::ExportImage { file, out, index } => {
            let imgs = image_blob::parse_sequence(&read(&file)?)?;
            let img = imgs
                .get(index)
                .with_context(|| format!("file has {} images", imgs.len()))?;
            write_png(
                &out,
                u32::from(img.width),
                u32::from(img.height),
                &img.to_rgba8_565(),
            )?;
            println!("wrote {} ({}x{})", out.display(), img.width, img.height);
            Ok(())
        }
        Cmd::ExportSres {
            file,
            id,
            out,
            frame,
        } => {
            let a = sres::parse(&read(&file)?)?;
            let e = a
                .get(id)
                .with_context(|| format!("no entry with id {id}"))?;
            let img = match &e.body {
                sres::Body::Picture(p) => p,
                sres::Body::PictureCollection(v)
                | sres::Body::Widget { pictures: v, .. }
                | sres::Body::Cursor { frames: v, .. } => v
                    .get(frame)
                    .with_context(|| format!("entry has {} frames", v.len()))?,
                _ => bail!("entry {id} is not a picture"),
            };
            write_png(
                &out,
                u32::from(img.width),
                u32::from(img.height),
                &img.to_rgba8_565(),
            )?;
            println!("wrote {} ({}x{})", out.display(), img.width, img.height);
            Ok(())
        }
        Cmd::GameDir { path } => {
            let g = opensherwood_assets::GameDir::discover(path.as_deref())?;
            println!("root: {}", g.root.display());
            for l in &g.layers {
                println!(
                    "  layer {} at {} ({} files)",
                    l.name,
                    l.root.display(),
                    l.paths().count()
                );
            }
            println!("fingerprint: {}", g.fingerprint());
            Ok(())
        }
    }
}

fn read(p: &Path) -> anyhow::Result<Vec<u8>> {
    std::fs::read(p).with_context(|| format!("reading {}", p.display()))
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(n).collect::<String>())
    }
}

fn write_png(out: &Path, w: u32, h: u32, rgba: &[u8]) -> anyhow::Result<()> {
    let f = std::fs::File::create(out).with_context(|| format!("creating {}", out.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

fn inspect(file: &Path) -> anyhow::Result<()> {
    let data = read(file)?;
    let kind = detect(&data);
    println!("{}: {} bytes, {kind:?}", file.display(), data.len());
    match kind {
        FileKind::Sres => {
            let a = sres::parse(&data)?;
            let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
            for e in &a.entries {
                *kinds.entry(e.body.kind()).or_default() += 1;
            }
            println!("  {} entries: {kinds:?}", a.entries.len());
        }
        FileKind::Rhp | FileKind::Rhm => {
            let root = if kind == FileKind::Rhp {
                b"MEUH"
            } else {
                b"DUTY"
            };
            let c = chunk::parse_container(&data, root)?;
            let tags: Vec<String> = c.children.iter().map(chunk::RawChunk::tag_str).collect();
            println!("  version {} children: {}", c.version, tags.join(" "));
        }
        FileKind::Scb => {
            let h = scb::parse_header(&data)?;
            println!(
                "  version {} unknown_0x0c={} source {:?} body at {:#x}",
                h.version, h.unknown_0x0c, h.source_path, h.body_offset
            );
        }
        FileKind::SpriteBank => {
            if let Ok(p) = rhs::parse(&data) {
                for s in &p.sequences {
                    println!(
                        "  sequence {:?} {}x{} {} animations",
                        s.name,
                        s.width,
                        s.height,
                        s.animations.len()
                    );
                }
            } else if let Ok(d) = dic::parse(&data) {
                println!(
                    "  dictionary: {} pages, {} frames",
                    d.page_count,
                    d.frames.len()
                );
            }
        }
        FileKind::ImageBlob => {
            let imgs = image_blob::parse_sequence(&data)?;
            for img in &imgs {
                println!("  {}x{} 16bpp", img.width, img.height);
            }
        }
        _ => {}
    }
    Ok(())
}
