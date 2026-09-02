//! `opensherwood-tools`: inspect game files and export pictures for local viewing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use opensherwood_formats::rhp;
use opensherwood_formats::{
    FileKind, chunk, cpf, detect, dic, font, image_blob, rhm, rhs, scb, sprite_decode, sres,
};

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
    /// Decode one sprite bank frame (dictionary page or span encoded) and write it as PNG.
    ExportFrame {
        dic: PathBuf,
        bks: PathBuf,
        frame_index: u32,
        out: PathBuf,
    },
    /// Render every glyph of an SBFONT bitmap font (.bfn / .fnt) in a grid and write it as PNG.
    FontSheet {
        file: PathBuf,
        out: PathBuf,
        /// Glyphs per row.
        #[arg(long, default_value_t = 16)]
        columns: u32,
    },
    /// Summarise the decoded geometry of an RHP map file.
    Rhp { file: PathBuf },
    /// Draw the decoded RHP geometry over the map background (.map) and write it as PNG.
    RhpOverlay {
        rhp: PathBuf,
        map: PathBuf,
        out: PathBuf,
    },
    /// Render a string with an SBFONT bitmap font and write it as PNG.
    FontText {
        file: PathBuf,
        text: String,
        out: PathBuf,
        /// Background colour as RRGGBB hex (glyphs are alpha-blended onto it).
        #[arg(long, default_value = "404040")]
        background: String,
    },
    /// Dump a mission (.rhm): header, actors, waypoints, patrol paths, script polygons, scrolls.
    Rhm {
        file: PathBuf,
        /// Print every waypoint command program.
        #[arg(long)]
        programs: bool,
    },
    /// Draw the actors, waypoints, patrol paths and script polygons of a mission over its map background.
    RhmOverlay {
        rhm: PathBuf,
        map: PathBuf,
        out: PathBuf,
    },
    /// Dump the profile table (profile.cpf): player, soldier and civilian sprites and the level table.
    Cpf {
        file: PathBuf,
        /// Print the raw `unknown_*` bytes of every record as hex.
        #[arg(long)]
        hex: bool,
    },
    /// Dump a compiled script (.scb): classes, variables, function table and a raw disassembly.
    Scb {
        file: PathBuf,
        /// Only this class.
        #[arg(long)]
        class: Option<String>,
        /// Skip the instruction listing.
        #[arg(long)]
        no_code: bool,
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
                    "  {:?}: {}x{} origin=({},{}) animations={} frame refs={}",
                    s.name,
                    s.width,
                    s.height,
                    s.origin_x,
                    s.origin_y,
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
        Cmd::ExportFrame {
            dic,
            bks,
            frame_index,
            out,
        } => {
            use std::io::{Read, Seek, SeekFrom};
            let dic_data = read(&dic)?;
            let d = dic::parse(&dic_data)?;
            let pages = sprite_decode::parse_pages(&d)?;
            let rec = d
                .frame(frame_index)
                .with_context(|| format!("table has {} frames", d.frames.len()))?;
            // The same policy the engine applies: a hostile record is refused before the stream
            // buffer exists, and the buffer itself is obtained with `try_reserve`.
            let limits = sprite_decode::DecodeLimits::RETAIL;
            limits
                .check_record(rec)
                .with_context(|| format!("frame {frame_index} record"))?;
            let mut f =
                std::fs::File::open(&bks).with_context(|| format!("opening {}", bks.display()))?;
            let mut stream = Vec::new();
            stream.try_reserve_exact(rec.length as usize).map_err(|_| {
                anyhow::anyhow!("frame {frame_index}: cannot allocate {} bytes", rec.length)
            })?;
            stream.resize(rec.length as usize, 0);
            f.seek(SeekFrom::Start(u64::from(rec.offset)))?;
            f.read_exact(&mut stream)
                .with_context(|| format!("reading frame {frame_index} stream"))?;
            let img = sprite_decode::decode_frame_with(rec, &stream, &pages, &limits)?;
            write_png(
                &out,
                u32::from(img.width),
                u32::from(img.height),
                &sprite_decode::to_rgba8_keyed(&img),
            )?;
            let enc = if rec.page == dic::NO_PAGE {
                String::from("span encoded")
            } else {
                format!("page {}", rec.page)
            };
            println!(
                "wrote {} ({}x{}, {enc}, {} stream bytes)",
                out.display(),
                img.width,
                img.height,
                rec.length
            );
            Ok(())
        }
        Cmd::FontSheet { file, out, columns } => {
            let f = font::parse_bitmap(&read(&file)?)?;
            let columns = columns.max(1);
            let cell_w = f.glyphs.iter().map(|g| g.width).max().unwrap_or(1).max(1) + 2;
            let cell_h = f.height() + 2;
            let rows = (f.glyphs.len() as u32).div_ceil(columns);
            let mut canvas = Canvas::new(cell_w * columns, cell_h * rows, [0x40, 0x40, 0x40]);
            for (i, g) in f.glyphs.iter().enumerate() {
                let i = i as u32;
                let x = (i % columns) * cell_w + 1;
                let y = (i / columns) * cell_h + 1;
                canvas.blit(&f.glyph_rgba(g), x as i64, y as i64);
            }
            write_png(&out, canvas.width, canvas.height, &canvas.rgba)?;
            println!(
                "{}: face {:?} {} glyphs, strip {}x{}, spacing {}, unknown_2e={} unknown_36={} unknown_3a={}",
                file.display(),
                f.name,
                f.glyphs.len(),
                f.colour.width,
                f.colour.height,
                f.spacing,
                f.unknown_2e,
                f.unknown_36,
                f.unknown_3a
            );
            println!(
                "wrote {} ({}x{})",
                out.display(),
                canvas.width,
                canvas.height
            );
            Ok(())
        }
        Cmd::FontText {
            file,
            text,
            out,
            background,
        } => {
            let f = font::parse_bitmap(&read(&file)?)?;
            let bg =
                u32::from_str_radix(&background, 16).context("background must be RRGGBB hex")?;
            let bg = [(bg >> 16) as u8, (bg >> 8) as u8, bg as u8];
            let glyphs: Vec<&font::Glyph> = text.chars().filter_map(|c| f.glyph(c)).collect();
            let mut pen: i64 = 2;
            let mut positions = Vec::with_capacity(glyphs.len());
            for g in &glyphs {
                pen += i64::from(g.x_adjust);
                positions.push(pen);
                pen += i64::from(g.width) + i64::from(g.advance_adjust) + i64::from(f.spacing);
            }
            let width = u32::try_from(pen.max(1) + 2).unwrap_or(1);
            let mut canvas = Canvas::new(width, f.height() + 4, bg);
            for (g, x) in glyphs.iter().zip(positions) {
                // The space record of the Scroll-face fonts aliases the "!" cell: spaces only advance.
                if g.code != 0x20 {
                    canvas.blit(&f.glyph_rgba(g), x, 2);
                }
            }
            write_png(&out, canvas.width, canvas.height, &canvas.rgba)?;
            let missing: String = text.chars().filter(|&c| f.glyph(c).is_none()).collect();
            if !missing.is_empty() {
                println!("no glyph for: {missing:?}");
            }
            println!(
                "wrote {} ({}x{})",
                out.display(),
                canvas.width,
                canvas.height
            );
            Ok(())
        }
        Cmd::Rhm { file, programs } => {
            dump_rhm(&rhm::parse(&read(&file)?)?, programs);
            Ok(())
        }
        Cmd::RhmOverlay { rhm, map, out } => {
            let m = rhm::parse(&read(&rhm)?)?;
            let imgs = image_blob::parse_sequence(&read(&map)?)?;
            let img = imgs.first().context("map has no image")?;
            let mut canvas = OverlayCanvas {
                w: usize::from(img.width),
                h: usize::from(img.height),
                rgba: img.to_rgba8_565(),
            };
            draw_mission(&mut canvas, &m);
            write_png(&out, img.width.into(), img.height.into(), &canvas.rgba)?;
            println!("wrote {} ({}x{})", out.display(), img.width, img.height);
            Ok(())
        }
        Cmd::Cpf { file, hex } => {
            dump_cpf(&cpf::parse(&read(&file)?)?, hex);
            Ok(())
        }
        Cmd::Scb {
            file,
            class,
            no_code,
        } => {
            dump_scb(&scb::parse(&read(&file)?)?, class.as_deref(), !no_code);
            Ok(())
        }
        Cmd::Rhp { file } => {
            let m = rhp::parse(&read(&file)?)?;
            print_rhp(&m);
            Ok(())
        }
        Cmd::RhpOverlay { rhp, map, out } => {
            let m = rhp::parse(&read(&rhp)?)?;
            let imgs = image_blob::parse_sequence(&read(&map)?)?;
            let img = imgs.first().context("map file has no image")?;
            let mut canvas = RhpCanvas {
                w: i64::from(img.width),
                h: i64::from(img.height),
                px: img.to_rgba8_565(),
            };
            draw_rhp(&mut canvas, &m);
            write_png(
                &out,
                u32::from(img.width),
                u32::from(img.height),
                &canvas.px,
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
            println!("fingerprint: {}", g.fingerprint()?);
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

/// Opaque RGBA8 canvas for the font subcommands.
struct Canvas {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32, bg: [u8; 3]) -> Self {
        let n = width as usize * height as usize;
        let mut rgba = Vec::with_capacity(n * 4);
        for _ in 0..n {
            rgba.extend_from_slice(&[bg[0], bg[1], bg[2], 255]);
        }
        Canvas {
            width,
            height,
            rgba,
        }
    }

    /// Alpha-blend `img` with its top-left corner at (`x0`, `y0`); parts outside the canvas are clipped.
    fn blit(&mut self, img: &font::RgbaImage, x0: i64, y0: i64) {
        for y in 0..img.height {
            for x in 0..img.width {
                let (cx, cy) = (x0 + i64::from(x), y0 + i64::from(y));
                if cx < 0 || cy < 0 || cx >= i64::from(self.width) || cy >= i64::from(self.height) {
                    continue;
                }
                let s = ((y * img.width + x) * 4) as usize;
                let d = ((cy as u32 * self.width + cx as u32) * 4) as usize;
                let a = u32::from(img.pixels[s + 3]);
                for c in 0..3 {
                    let src = u32::from(img.pixels[s + c]);
                    let dst = u32::from(self.rgba[d + c]);
                    self.rgba[d + c] = ((src * a + dst * (255 - a)) / 255) as u8;
                }
            }
        }
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
            if kind == FileKind::Rhm {
                let m = rhm::parse(&data)?;
                println!(
                    "  map {:?} (id {}, variant {}), mission {}: {} PCs, {} NPCs, {} civilians, {} VIPs, {} objects, {} rails, {} waypoints, {} script polygons, {} scrolls",
                    m.header.map,
                    m.header.map_id,
                    m.header.variant,
                    m.header.mission_id,
                    m.player_characters().len(),
                    m.npcs().len(),
                    m.civilians().len(),
                    m.vips().len(),
                    m.objects().len(),
                    m.rails.len(),
                    m.brains.waypoints.len(),
                    m.script_areas.polygons.len(),
                    m.scrolls.len()
                );
            }
        }
        FileKind::Scb => {
            let h = scb::parse_header(&data)?;
            println!(
                "  version {} classes={} source {:?} body at {:#x}",
                h.version, h.class_count, h.source_path, h.body_offset
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
        FileKind::ProfileTable => {
            let t = cpf::parse(&data)?;
            println!(
                "  table A {} blocks, table B {} records, {} player characters, {} soldiers, {} levels, {} civilians",
                t.table_a.len(),
                t.table_b.len(),
                t.player_characters.len(),
                t.soldiers.len(),
                t.levels.len(),
                t.civilians.len()
            );
        }
        _ => {}
    }
    Ok(())
}

fn dump_cpf(t: &cpf::ProfileTable, with_hex: bool) {
    println!(
        "profile table: table A {} blocks, table B {} records, PC {}, SD {}, LEVEL {}, CV {}",
        t.table_a.len(),
        t.table_b.len(),
        t.player_characters.len(),
        t.soldiers.len(),
        t.levels.len(),
        t.civilians.len()
    );
    println!("== PC (TOTO.profile; the player's team)");
    for (i, p) in t.player_characters.iter().enumerate() {
        println!(
            "{i:3} sprite {:<20} sequence {:<22} label {:<30} voice {:4}",
            p.sprite,
            format!("{:?}", p.sequence),
            format!("{:?}", p.label),
            p.voice
        );
        if with_hex {
            println!(
                "    unknown_pre {} unknown_post {}",
                hex(&p.unknown_pre),
                hex(&p.unknown_post)
            );
        }
    }
    println!("== SD (BORG.profile)");
    for (i, p) in t.soldiers.iter().enumerate() {
        println!(
            "{i:3} sprite {:<20} sequence {:<22} label {:<30} voice {:4}",
            p.sprite,
            format!("{:?}", p.sequence),
            format!("{:?}", p.label),
            p.voice
        );
        if with_hex {
            println!(
                "    unknown_pre {} unknown_post {}",
                hex(&p.unknown_pre),
                hex(&p.unknown_post)
            );
        }
    }
    println!("== CV (OILE.profile)");
    for (i, p) in t.civilians.iter().enumerate() {
        println!(
            "{i:3} sprite {:<20} sequence {:<22} label {:<30} voice {:4}",
            p.sprite,
            format!("{:?}", p.sequence),
            format!("{:?}", p.label),
            p.voice
        );
        if with_hex {
            println!("    unknown_pre {}", hex(&p.unknown_pre));
        }
    }
    println!("== LEVEL");
    for (i, l) in t.levels.iter().enumerate() {
        println!(
            "{i:3} {:2} map {:<13} mission {:<19} title {:<28} kind {} location {} after {:?} until {:?} music {:?} / {:?} / {:?}",
            l.code,
            l.map,
            l.mission_file,
            format!("{:?}", l.title),
            l.unknown_a,
            l.location,
            l.after,
            l.until,
            l.music_ambient,
            l.music_alarm,
            l.music_fight
        );
        if with_hex {
            println!(
                "    unknown c {} d {} e {} f {} g {} h {} fixed {:?} i {} j {} k {:?} l {} slots {:?} m {:?}",
                l.unknown_c,
                l.unknown_d,
                l.unknown_e,
                l.unknown_f,
                l.unknown_g,
                l.unknown_h,
                l.unknown_fixed,
                l.unknown_i,
                l.unknown_j,
                l.unknown_k,
                hex(&l.unknown_l),
                l.unknown_slots,
                l.unknown_m
            );
        }
    }
    if with_hex {
        println!("== table A");
        for (i, b) in t.table_a.iter().enumerate() {
            println!("{i:3} head {}", hex(&b.unknown_head));
            for r in &b.unknown_records {
                println!("    {}", hex(r));
            }
        }
        println!("== table B");
        for r in &t.table_b {
            println!("    {}", hex(r));
        }
    }
}

fn dump_rhm(m: &rhm::Mission, programs: bool) {
    let h = &m.header;
    println!(
        "DUTY version {}; map {:?} id {} variant {} mission {}; chunks: {}",
        m.version,
        h.map,
        h.map_id,
        h.variant,
        h.mission_id,
        m.chunk_versions
            .iter()
            .map(|(t, v)| format!("{}v{v}", String::from_utf8_lossy(t)))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("POUF: {} animated elements", m.tenants.len());
    for t in &m.tenants {
        println!(
            "  {:?} {:?} ({} body bytes)",
            t.sprite,
            t.label,
            t.body.len()
        );
    }
    for g in &m.actor_groups {
        match g {
            rhm::ActorGroup::Meow { version, count } => println!("MEOW v{version}: {count}"),
            rhm::ActorGroup::PlayerCharacters { version, records } => {
                println!("SCOT v{version}: {} player characters", records.len());
                for r in records {
                    println!(
                        "  ({},{}) dir {} u08={:#x} q=({},{},{}) u12={} u16={:02x?} trailer={} {}",
                        r.placement.x,
                        r.placement.y,
                        r.placement.direction,
                        r.placement.unknown_0x08,
                        r.placement.unknown_0x0c,
                        r.placement.unknown_0x0e,
                        r.placement.unknown_0x10,
                        r.unknown_0x12,
                        r.unknown_0x16,
                        r.unknown_trailer,
                        r.name.as_deref().unwrap_or("-")
                    );
                }
            }
            rhm::ActorGroup::Npcs { version, records } => {
                println!("BORG v{version}: {} NPCs", records.len());
                for (i, r) in records.iter().enumerate() {
                    println!(
                        "  #{i} ({},{}) dir {} u08={:#x} q=({},{},{}) u12={} profile {} u1a={} u1b={} u23={} members {:?} rail {} u_i16 {} {}",
                        r.placement.x,
                        r.placement.y,
                        r.placement.direction,
                        r.placement.unknown_0x08,
                        r.placement.unknown_0x0c,
                        r.placement.unknown_0x0e,
                        r.placement.unknown_0x10,
                        r.unknown_0x12,
                        r.profile,
                        r.unknown_0x1a,
                        r.unknown_0x1b,
                        r.unknown_0x23,
                        r.members,
                        r.rail,
                        r.unknown_i16,
                        r.name.as_deref().unwrap_or("-")
                    );
                }
            }
            rhm::ActorGroup::Civilians { version, records } => {
                println!("OILE v{version}: {} civilians", records.len());
                for r in records {
                    println!(
                        "  ({},{}) dir {} u08={:#x} u12={} profile {} i16=({},{}) lists {:?} {}",
                        r.placement.x,
                        r.placement.y,
                        r.placement.direction,
                        r.placement.unknown_0x08,
                        r.unknown_0x12,
                        r.profile,
                        r.unknown_i16_a,
                        r.unknown_i16_b,
                        r.lists,
                        r.name.as_deref().unwrap_or("-")
                    );
                }
            }
            rhm::ActorGroup::Vips { version, records } => {
                println!("TOTO v{version}: {} named NPCs", records.len());
                for r in records {
                    println!(
                        "  ({},{}) dir {} u08={:#x} u12={} profile {} i16=({},{}) {}",
                        r.placement.x,
                        r.placement.y,
                        r.placement.direction,
                        r.placement.unknown_0x08,
                        r.unknown_0x12,
                        r.profile,
                        r.unknown_i16_a,
                        r.unknown_i16_b,
                        r.name.as_deref().unwrap_or("-")
                    );
                }
            }
            rhm::ActorGroup::Objects { version, records } => {
                println!("BOOM v{version}: {} objects", records.len());
                for r in records {
                    println!(
                        "  ({},{}) u04={} u0a={} q=({},{},{}) {:?}/{:?} flags={:#x} anchor ({},{}) poly {} pts {}",
                        r.x,
                        r.y,
                        r.unknown_0x04,
                        r.unknown_0x0a,
                        r.unknown_0x0e,
                        r.unknown_0x10,
                        r.unknown_0x12,
                        r.sprite,
                        r.label,
                        r.unknown_flags,
                        r.x2,
                        r.y2,
                        r.polygon.points.len(),
                        r.name.as_deref().unwrap_or("-")
                    );
                }
            }
            rhm::ActorGroup::Unknown { tag, version, body } => println!(
                "{} v{version}: {} bytes (unknown class)",
                String::from_utf8_lossy(tag),
                body.len()
            ),
        }
    }
    println!("ZORG: {} entries", m.zorg.len());
    for z in &m.zorg {
        println!(
            "  a={} b={} ({},{}) u08={} q=({},{},{})",
            z.unknown_a,
            z.unknown_b,
            z.placement.x,
            z.placement.y,
            z.placement.unknown_0x08,
            z.placement.unknown_0x0c,
            z.placement.unknown_0x0e,
            z.placement.unknown_0x10
        );
    }
    let b = &m.brains;
    println!(
        "HIRN: {} waypoints, {} bushes, {} beam-me points, {} nlips",
        b.waypoints.len(),
        b.bushes.len(),
        b.beam_points.len(),
        b.nlips.len()
    );
    for w in &b.waypoints {
        println!(
            "  waypoint ({},{}) dir {} q=({},{})",
            w.x, w.y, w.direction, w.unknown_0x04, w.unknown_0x06
        );
    }
    for p in &b.beam_points {
        println!(
            "  beam-me ({},{}) dir {} u08={}",
            p.x, p.y, p.direction, p.unknown_0x08
        );
    }
    println!("RAIL: {} paths", m.rails.len());
    for (i, rail) in m.rails.iter().enumerate() {
        let pts: Vec<String> = rail
            .iter()
            .map(|p| match &p.name {
                Some(n) => format!("({},{}){n:?}", p.point.x, p.point.y),
                None => format!(
                    "({},{}){}",
                    p.point.x,
                    p.point.y,
                    if p.tables.is_empty() { "" } else { "*" }
                ),
            })
            .collect();
        println!("  rail {i}: {}", pts.join(" "));
        if programs {
            for (j, p) in rail.iter().enumerate() {
                for t in &p.tables {
                    for blk in &t.blocks {
                        let cmds: Vec<String> = blk
                            .commands
                            .iter()
                            .map(|c| format!("cmd_{:02x}({})", c.opcode, hex(&c.args)))
                            .collect();
                        println!(
                            "    point {j} table {} {}%: {}",
                            t.id,
                            blk.percent,
                            cmds.join(" ")
                        );
                    }
                }
            }
        }
    }
    println!("SKRO: {} scrolls", m.scrolls.len());
    for s in &m.scrolls {
        println!(
            "  ({},{}) u08={} flags {:02x?} {}",
            s.placement.x,
            s.placement.y,
            s.placement.unknown_0x08,
            s.unknown_flags,
            s.name.as_deref().unwrap_or("-")
        );
    }
    println!("TING: {} mobile elements", m.mobiles.len());
    for t in &m.mobiles {
        let anims: Vec<String> = t
            .animations
            .iter()
            .map(|a| format!("{}/{} d=({},{})", a.sprite, a.animation, a.dx, a.dy))
            .collect();
        println!(
            "  ({},{}) u_b={} poly {} pts: {}",
            t.x,
            t.y,
            t.unknown_b,
            t.polygon.points.len(),
            anims.join(", ")
        );
    }
    println!(
        "GULP: {} points, {} script polygons",
        m.script_areas.points.len(),
        m.script_areas.polygons.len()
    );
    for p in &m.script_areas.polygons {
        println!(
            "  {} pts q=({},{}) {}",
            p.polygon.points.len(),
            p.unknown_0x00,
            p.unknown_0x02,
            p.name.as_deref().unwrap_or("-")
        );
    }
    println!("CAVE: {} entries", m.cave.len());
    for (i, c) in m.cave.iter().enumerate() {
        if !c.ids.is_empty() || c.unknown_flag != 0 {
            println!("  #{i} ids {:?} flag {}", c.ids, c.unknown_flag);
        }
    }
}

fn hex(b: &[u8]) -> String {
    use std::fmt::Write as _;
    b.iter().fold(String::new(), |mut acc, x| {
        let _ = write!(acc, "{x:02x}");
        acc
    })
}

fn dump_scb(s: &scb::Script, only: Option<&str>, code: bool) {
    println!(
        "SBSCRIPT version {} with {} classes",
        s.version,
        s.classes.len()
    );
    for c in &s.classes {
        if only.is_some_and(|o| o != c.name) {
            continue;
        }
        println!(
            "class {:?} (source {:?}): {} variables ({} bytes), {} functions, {} quads",
            c.name,
            c.source_path,
            c.variables.len(),
            c.size_of_variables,
            c.functions.len(),
            c.quads.len()
        );
        for v in &c.variables {
            println!(
                "  var {} type {} {:?} at +{}",
                v.name, v.type_tag, v.type_name, v.offset
            );
        }
        for (i, f) in c.functions.iter().enumerate() {
            println!(
                "  fn {} @{} u0={} u1={} u2={} volatile={} tempor={} ({} quads)",
                f.name,
                f.address,
                f.unknown_0,
                f.unknown_1,
                f.unknown_2,
                f.size_of_volatile,
                f.size_of_tempor,
                c.function_quads(i).len()
            );
            if code {
                for (k, q) in c.function_quads(i).iter().enumerate() {
                    println!("    {:5}: {}", f.address as usize + k, scb::disassemble(q));
                }
            }
        }
    }
}

/// RGBA drawing surface for overlays.
struct OverlayCanvas {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

impl OverlayCanvas {
    fn put(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.w || y >= self.h {
            return;
        }
        let i = (y * self.w + x) * 4;
        self.rgba[i..i + 4].copy_from_slice(&c);
    }

    fn line(&mut self, (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: [u8; 4]) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        for _ in 0..10_000 {
            self.put(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn circle(&mut self, (cx, cy): (i32, i32), r: i32, c: [u8; 4]) {
        for dy in -r..=r {
            for dx in -r..=r {
                let d = dx * dx + dy * dy;
                if d <= r * r && d >= (r - 1) * (r - 1) {
                    self.put(cx + dx, cy + dy, c);
                }
            }
        }
    }

    fn polygon(&mut self, pts: &[(u16, u16)], c: [u8; 4]) {
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            self.line(
                (i32::from(a.0), i32::from(a.1)),
                (i32::from(b.0), i32::from(b.1)),
                c,
            );
        }
    }
}

fn draw_mission(cv: &mut OverlayCanvas, m: &rhm::Mission) {
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const RED: [u8; 4] = [255, 0, 0, 255];
    const YELLOW: [u8; 4] = [255, 255, 0, 255];
    const MAGENTA: [u8; 4] = [255, 0, 255, 255];
    const CYAN: [u8; 4] = [0, 255, 255, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const ORANGE: [u8; 4] = [255, 160, 0, 255];
    const BLUE: [u8; 4] = [64, 64, 255, 255];
    const PURPLE: [u8; 4] = [160, 0, 255, 255];
    const GOLD: [u8; 4] = [255, 215, 0, 255];
    const PINK: [u8; 4] = [255, 128, 192, 255];

    fn actor(cv: &mut OverlayCanvas, p: &rhm::Placement, c: [u8; 4]) {
        let (x, y) = (i32::from(p.x), i32::from(p.y));
        cv.circle((x, y), 5, c);
        let a = f64::from(p.direction) * std::f64::consts::PI / 8.0;
        let (dx, dy) = ((10.0 * a.cos()) as i32, -((10.0 * a.sin()) as i32));
        cv.line((x, y), (x + dx, y + dy), c);
    }
    for pc in m.player_characters() {
        actor(cv, &pc.placement, GREEN);
    }
    for n in m.npcs() {
        actor(cv, &n.placement, RED);
    }
    for c in m.civilians() {
        actor(cv, &c.placement, YELLOW);
    }
    for v in m.vips() {
        actor(cv, &v.placement, MAGENTA);
    }
    for o in m.objects() {
        cv.circle((i32::from(o.x), i32::from(o.y)), 5, CYAN);
    }
    for w in &m.brains.waypoints {
        cv.circle((i32::from(w.x), i32::from(w.y)), 3, WHITE);
    }
    for p in &m.brains.beam_points {
        cv.circle((i32::from(p.x), i32::from(p.y)), 6, ORANGE);
    }
    for b in &m.brains.bushes {
        cv.circle((i32::from(b.x), i32::from(b.y)), 3, BLUE);
    }
    for rail in &m.rails {
        for w in rail.windows(2) {
            cv.line(
                (i32::from(w[0].point.x), i32::from(w[0].point.y)),
                (i32::from(w[1].point.x), i32::from(w[1].point.y)),
                GREEN,
            );
        }
        for p in rail {
            cv.circle((i32::from(p.point.x), i32::from(p.point.y)), 2, GREEN);
        }
    }
    for poly in &m.script_areas.polygons {
        cv.polygon(&poly.polygon.points, PURPLE);
    }
    for p in &m.script_areas.points {
        cv.circle((i32::from(p.x), i32::from(p.y)), 2, PURPLE);
    }
    for s in &m.scrolls {
        cv.circle(
            (i32::from(s.placement.x), i32::from(s.placement.y)),
            6,
            GOLD,
        );
    }
    for z in &m.zorg {
        cv.circle(
            (i32::from(z.placement.x), i32::from(z.placement.y)),
            5,
            PINK,
        );
    }
    for t in &m.mobiles {
        cv.circle((i32::from(t.x), i32::from(t.y)), 8, CYAN);
        cv.polygon(&t.polygon.points, CYAN);
    }
}

fn print_rhp(m: &rhp::Rhp) {
    println!(
        "MEUH version {} SPOK unknown_0x00={} unknown_0x04={} unknown_0x08={}",
        m.version, m.spok.unknown_0x00, m.spok.unknown_0x04, m.spok.unknown_0x08
    );
    let s = &m.stat;
    println!(
        "STAT: unknown ({}, {}), boundary {} points, {} segments, {} obstacles (flags {:?}), {} bytes undecoded",
        s.unknown_0x00,
        s.unknown_0x02,
        s.boundary.len(),
        s.segments.len(),
        s.obstacles.len(),
        s.obstacles
            .iter()
            .map(|o| o.flags)
            .filter(|&f| f != 0)
            .collect::<std::collections::BTreeSet<_>>(),
        s.rest.len()
    );
    let kinds: BTreeMap<u8, usize> = m.text.iter().fold(BTreeMap::new(), |mut acc, z| {
        *acc.entry(z.kind).or_default() += 1;
        acc
    });
    println!("TEXT: {} zones, kinds {kinds:?}", m.text.len());
    println!(
        "WOAW: {} layers {:?}, {} areas ({} with zone links)",
        m.woaw.layers.len(),
        m.woaw.layers,
        m.woaw.areas.len(),
        m.woaw.areas.iter().filter(|a| !a.links.is_empty()).count()
    );
    println!(
        "007 : {} bonds ({} to no area)",
        m.bonds.len(),
        m.bonds.iter().filter(|b| b.area_b == rhp::NO_AREA).count()
    );
    let face_kinds: BTreeMap<u8, usize> = m.faces.iter().fold(BTreeMap::new(), |mut acc, f| {
        *acc.entry(f.kind).or_default() += 1;
        acc
    });
    println!(
        "FACE: {} masks, kinds {face_kinds:?}, {} with references, largest {}x{}",
        m.faces.len(),
        m.faces.iter().filter(|f| !f.refs.is_empty()).count(),
        m.faces.iter().map(|f| f.width).max().unwrap_or(0),
        m.faces.iter().map(|f| f.height).max().unwrap_or(0)
    );
    println!("FLIM: {} animated elements", m.flims.len());
    for f in &m.flims {
        println!(
            "  {:?} {:?} at {},{} unknown_0x04={} flags={:?} line={:?}",
            f.sprite,
            f.name,
            f.x,
            f.y,
            f.unknown_0x04,
            f.unknown_flags,
            f.line.points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>()
        );
    }
    println!(
        "DARK: {} zones; PPPP: {} zones, {} jump lines; raw: FARM {} bytes, AZ {} bytes, TUPO {} bytes, LOUD {} bytes",
        m.dark.len(),
        m.pppp.zones.len(),
        m.pppp.jump_lines.len(),
        m.farm.len(),
        m.az.len(),
        m.tupo.len(),
        m.loud.len()
    );
}

/// RGBA pixel buffer with alpha-blended line drawing (for the overlay export).
struct RhpCanvas {
    w: i64,
    h: i64,
    px: Vec<u8>,
}

impl RhpCanvas {
    fn blend(&mut self, x: i64, y: i64, c: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let a = u32::from(c[3]);
        for (dst, &src) in self.px[i..i + 3].iter_mut().zip(&c[..3]) {
            let d = u32::from(*dst);
            *dst = ((u32::from(src) * a + d * (255 - a)) / 255) as u8;
        }
    }

    fn line(&mut self, (x0, y0): (i64, i64), (x1, y1): (i64, i64), c: [u8; 4]) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.blend(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn polygon(&mut self, pts: &[(i64, i64)], close: bool, c: [u8; 4]) {
        for w in pts.windows(2) {
            self.line(w[0], w[1], c);
        }
        if close && pts.len() > 2 {
            self.line(pts[pts.len() - 1], pts[0], c);
        }
    }
}

fn draw_rhp(cv: &mut RhpCanvas, m: &rhp::Rhp) {
    let pts = |v: &[rhp::Point]| -> Vec<(i64, i64)> {
        v.iter().map(|p| (i64::from(p.x), i64::from(p.y))).collect()
    };
    // FACE occluder masks, tinted.
    let tints = [
        [255, 0, 0, 110],
        [0, 255, 0, 110],
        [0, 128, 255, 110],
        [255, 255, 0, 110],
    ];
    for (i, f) in m.faces.iter().enumerate() {
        for y in 0..usize::from(f.height) {
            for x in 0..usize::from(f.width) {
                if f.pixel(x, y) {
                    cv.blend(
                        i64::from(f.x) + x as i64,
                        i64::from(f.y) + y as i64,
                        tints[i % tints.len()],
                    );
                }
            }
        }
    }
    // WOAW projection areas (green) and bonds (cyan).
    for a in &m.woaw.areas {
        let poly: Vec<(i64, i64)> = a.points.iter().map(|p| (p.x as i64, p.y as i64)).collect();
        cv.polygon(&poly, true, [0, 255, 0, 255]);
    }
    for b in &m.bonds {
        cv.line(
            (i64::from(b.x1), i64::from(b.y1)),
            (i64::from(b.x2), i64::from(b.y2)),
            [0, 255, 255, 255],
        );
    }
    // STAT: boundary (red), segments (orange), obstacles (yellow).
    let boundary = pts(&m.stat.boundary);
    cv.polygon(&boundary, true, [255, 0, 0, 255]);
    for s in &m.stat.segments {
        cv.line(
            (i64::from(s.a.x), i64::from(s.a.y)),
            (i64::from(s.b.x), i64::from(s.b.y)),
            [255, 128, 0, 255],
        );
    }
    for o in &m.stat.obstacles {
        cv.polygon(&pts(&o.polygon.points), true, [255, 255, 0, 255]);
    }
    // TEXT zones (blue), DARK zones (black), PPPP zones (magenta), jump lines (orange to white).
    for z in &m.text {
        cv.polygon(&pts(&z.polygon.points), true, [64, 128, 255, 255]);
    }
    for d in &m.dark {
        cv.polygon(&pts(&d.polygon.points), true, [0, 0, 0, 255]);
    }
    for z in &m.pppp.zones {
        cv.polygon(&pts(&z.polygon.points), true, [255, 0, 255, 255]);
    }
    for j in &m.pppp.jump_lines {
        let seg = |s: &[rhp::Point3; 2]| {
            (
                (i64::from(s[0].x), i64::from(s[0].y)),
                (i64::from(s[1].x), i64::from(s[1].y)),
            )
        };
        let (a0, a1) = seg(&j.from);
        let (b0, b1) = seg(&j.to);
        cv.line(a0, a1, [255, 128, 0, 255]);
        cv.line(b0, b1, [255, 255, 255, 255]);
        cv.line(
            (a0.0.midpoint(a1.0), a0.1.midpoint(a1.1)),
            (b0.0.midpoint(b1.0), b0.1.midpoint(b1.1)),
            [255, 255, 255, 160],
        );
    }
    // FLIM positions (white cross) and their sorting lines (grey).
    for f in &m.flims {
        let (x, y) = (i64::from(f.x), i64::from(f.y));
        cv.line((x - 4, y), (x + 4, y), [255, 255, 255, 255]);
        cv.line((x, y - 4), (x, y + 4), [255, 255, 255, 255]);
        cv.polygon(&pts(&f.line.points), false, [200, 200, 200, 255]);
    }
}
