//! Compiled mission scripts (`.scb`, magic `SBSCRIPT`). Spec: `docs/formats/scb.md`.
//!
//! A script is a list of classes (one `StartUp` level class plus one class per named mission element).
//! Each class has a source path, variables, a function table and a flat array of 9-byte instructions
//! ("quads"). The container layout and the operand encoding are established by observation over the
//! 39 retail scripts; the meaning of most opcodes is not (see the spec), so the disassembler prints
//! opcode numbers and decoded operand fields.

use crate::reader::{FormatError, Reader, latin1};

/// Format version in retail data.
pub const VERSION: f32 = 1.5;
/// Size of one instruction.
pub const QUAD_SIZE: usize = 9;

/// Header of a compiled script.
#[derive(Debug, Clone, PartialEq)]
pub struct ScbHeader {
    /// Format version as a float (1.5 in retail data).
    pub version: f32,
    /// Number of classes in the file.
    pub class_count: u32,
    /// Path of the source file on the developer's machine (first class).
    pub source_path: String,
    /// Offset of the first byte after the header (start of the first class name).
    pub body_offset: usize,
}

/// A class variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// Type tag: 2 (plain 4-byte value: int/bool) or 7 (object reference with a type name).
    pub type_tag: u8,
    /// Type name for tag 7 ("Actor", "Location"); empty otherwise.
    pub type_name: String,
    /// Variable name.
    pub name: String,
    /// Byte offset in the class variable block.
    pub offset: u32,
}

/// A function table entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Name ("Initialize", "EnterZone", ...).
    pub name: String,
    /// Index of the first instruction in the class quad array.
    pub address: u32,
    /// 2..=6; parameter count including hidden ones is the hypothesis.
    pub unknown_0: u32,
    /// 0 or 4 (return value size is the hypothesis).
    pub unknown_1: u32,
    /// 0, 4, 8, 12, 16, 20 (parameter block size is the hypothesis).
    pub unknown_2: u32,
    /// First operand of the `0x03` instruction at `address` (in every retail function).
    pub size_of_volatile: u32,
    /// Second operand of the `0x03` instruction at `address`.
    pub size_of_tempor: u32,
}

/// One 9-byte instruction: opcode, two `u16` operands and one `u32` operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quad {
    /// Opcode (0x01..=0x2b in retail data).
    pub opcode: u8,
    /// Bytes 1-2.
    pub a: u16,
    /// Bytes 3-4.
    pub b: u16,
    /// Bytes 5-8.
    pub c: u32,
}

/// A script class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// Source path.
    pub source_path: String,
    /// Class name: `StartUp` for the level, otherwise a mission element name.
    pub name: String,
    /// Size of the variable block in bytes.
    pub size_of_variables: u32,
    /// Variables.
    pub variables: Vec<Variable>,
    /// Functions.
    pub functions: Vec<Function>,
    /// Instructions.
    pub quads: Vec<Quad>,
}

/// A parsed script.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    /// Version.
    pub version: f32,
    /// Classes in file order.
    pub classes: Vec<Class>,
}

impl Script {
    /// Find a class by name.
    #[must_use]
    pub fn class(&self, name: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.name == name)
    }
}

impl Class {
    /// Instructions of the function starting at `address` (up to the next function or the end).
    #[must_use]
    pub fn function_quads(&self, index: usize) -> &[Quad] {
        let Some(f) = self.functions.get(index) else {
            return &[];
        };
        let start = (f.address as usize).min(self.quads.len());
        let end = self.functions.get(index + 1).map_or(self.quads.len(), |n| {
            (n.address as usize).min(self.quads.len())
        });
        &self.quads[start..end.max(start)]
    }
}

fn pstring32(r: &mut Reader<'_>, what: &'static str) -> Result<String, FormatError> {
    let n = r.u32(what)? as usize;
    if n > 4096 {
        return Err(FormatError::Invalid {
            offset: r.pos() - 4,
            what,
            value: n.to_string(),
        });
    }
    Ok(latin1(r.bytes(n, what)?))
}

/// Parse the header of a `.scb` file.
pub fn parse_header(data: &[u8]) -> Result<ScbHeader, FormatError> {
    let mut r = Reader::new(data);
    r.expect(b"SBSCRIPT", "SBSCRIPT magic")?;
    let version = r.f32("scb version")?;
    let class_count = r.u32("scb class count")?;
    let source_path = pstring32(&mut r, "scb source path")?;
    Ok(ScbHeader {
        version,
        class_count,
        source_path,
        body_offset: r.pos(),
    })
}

fn parse_class(r: &mut Reader<'_>) -> Result<Class, FormatError> {
    let source_path = pstring32(r, "class source path")?;
    let name = pstring32(r, "class name")?;
    let nvars = r.u32("variable count")? as usize;
    let size_of_variables = r.u32("size of variables")?;
    if nvars > 4096 {
        return Err(FormatError::Invalid {
            offset: r.pos() - 8,
            what: "variable count",
            value: nvars.to_string(),
        });
    }
    let mut variables = Vec::with_capacity(nvars);
    for _ in 0..nvars {
        let type_tag = r.u8("variable type")?;
        let tlen = usize::from(r.u8("variable type name length")?);
        let type_name = latin1(r.bytes(tlen, "variable type name")?);
        let name = pstring32(r, "variable name")?;
        let offset = r.u32("variable offset")?;
        variables.push(Variable {
            type_tag,
            type_name,
            name,
            offset,
        });
    }
    let nfuncs = r.u32("function count")? as usize;
    if nfuncs > 4096 {
        return Err(FormatError::Invalid {
            offset: r.pos() - 4,
            what: "function count",
            value: nfuncs.to_string(),
        });
    }
    let mut functions = Vec::with_capacity(nfuncs);
    for _ in 0..nfuncs {
        functions.push(Function {
            name: pstring32(r, "function name")?,
            address: r.u32("function address")?,
            unknown_0: r.u32("function unknown_0")?,
            unknown_1: r.u32("function unknown_1")?,
            unknown_2: r.u32("function unknown_2")?,
            size_of_volatile: r.u32("function size of volatile")?,
            size_of_tempor: r.u32("function size of tempor")?,
        });
    }
    let nquads = r.u32("quad count")? as usize;
    let bytes = r.bytes(
        nquads.checked_mul(QUAD_SIZE).ok_or(FormatError::Invalid {
            offset: r.pos() - 4,
            what: "quad count",
            value: nquads.to_string(),
        })?,
        "quads",
    )?;
    let quads = bytes
        .chunks_exact(QUAD_SIZE)
        .map(|q| Quad {
            opcode: q[0],
            a: u16::from_le_bytes([q[1], q[2]]),
            b: u16::from_le_bytes([q[3], q[4]]),
            c: u32::from_le_bytes([q[5], q[6], q[7], q[8]]),
        })
        .collect();
    Ok(Class {
        source_path,
        name,
        size_of_variables,
        variables,
        functions,
        quads,
    })
}

/// Parse a whole script.
pub fn parse(data: &[u8]) -> Result<Script, FormatError> {
    let h = parse_header(data)?;
    // The header's path is the first class's path: re-read from the class start.
    let mut r = Reader::at(data, 16)?;
    let n = h.class_count as usize;
    if n > 4096 {
        return Err(FormatError::Invalid {
            offset: 12,
            what: "scb class count",
            value: n.to_string(),
        });
    }
    let mut classes = Vec::with_capacity(n);
    for _ in 0..n {
        classes.push(parse_class(&mut r)?);
    }
    r.expect_end("scb classes")?;
    Ok(Script {
        version: h.version,
        classes,
    })
}

/// Storage class of a `u16` operand (top two bits), as observed in retail data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    /// `00`: not a variable reference (jump targets, native ids, zero).
    None,
    /// `01`: class variable block (`class_var`).
    ClassVar,
    /// `10`: function-local ("volatile") slot.
    Local,
    /// `11`: temporary slot.
    Temp,
}

/// Decode a `u16` operand into storage class and slot offset.
#[must_use]
pub fn operand(v: u16) -> (Storage, u16) {
    let s = match v >> 14 {
        0 => Storage::None,
        1 => Storage::ClassVar,
        2 => Storage::Local,
        _ => Storage::Temp,
    };
    (s, v & 0x3fff)
}

fn fmt_operand(v: u16) -> String {
    match operand(v) {
        (Storage::None, n) => format!("{n:#x}"),
        (Storage::ClassVar, n) => format!("cv{n}"),
        (Storage::Local, n) => format!("lv{n}"),
        (Storage::Temp, n) => format!("tv{n}"),
    }
}

/// Operand layout of an opcode, established by observation (see spec table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// No operands.
    Nothing,
    /// One variable reference in `a`.
    A,
    /// Two variable references in `a` and `b`.
    Ab,
    /// Variable references in `a`, `b` and the low half of `c`.
    Abc16,
    /// Variable reference in `a`, immediate in `c`.
    AImm,
    /// Immediate in `a` only (function address, native id, jump target).
    AIndex,
    /// Variable reference in `a`, quad index in `c` (conditional jump).
    AJump,
    /// Two `u16` immediates in `a` and `b` (function prologue).
    Sizes,
    /// Not seen in retail data.
    Unknown,
}

/// Operand layout of an opcode.
#[must_use]
pub fn layout(opcode: u8) -> Layout {
    match opcode {
        0x01 | 0x04 | 0x06 => Layout::Nothing,
        0x02 | 0x07 | 0x0a | 0x0b | 0x0d => Layout::A,
        0x03 => Layout::Sizes,
        0x05 | 0x0c | 0x0e => Layout::AIndex,
        0x08 | 0x13 | 0x14 => Layout::AImm,
        0x0f | 0x10 => Layout::AJump,
        0x11 | 0x12 | 0x15 | 0x16 | 0x18 => Layout::Ab,
        0x19..=0x2c => Layout::Abc16,
        _ => Layout::Unknown,
    }
}

/// Mnemonic for the opcodes whose role is established from the data (see spec); `None` otherwise.
#[must_use]
pub fn mnemonic(opcode: u8) -> Option<&'static str> {
    match opcode {
        0x03 => Some("enter"),
        0x05 => Some("call"),
        0x0c => Some("native"),
        0x0e => Some("jump"),
        0x0f => Some("jump_cond"),
        _ => None,
    }
}

/// Render one instruction. Unknown opcodes print as `op_XX` with every operand field.
#[must_use]
pub fn disassemble(q: &Quad) -> String {
    let name = mnemonic(q.opcode).map_or_else(|| format!("op_{:02x}", q.opcode), String::from);
    let ops = match layout(q.opcode) {
        Layout::Nothing | Layout::Unknown => String::new(),
        Layout::A => fmt_operand(q.a),
        Layout::Ab => format!("{}, {}", fmt_operand(q.a), fmt_operand(q.b)),
        Layout::Abc16 => format!(
            "{}, {}, {}",
            fmt_operand(q.a),
            fmt_operand(q.b),
            fmt_operand((q.c & 0xffff) as u16)
        ),
        Layout::AImm => {
            if q.opcode == 0x14 {
                format!("{}, {}", fmt_operand(q.a), f32::from_bits(q.c))
            } else {
                format!("{}, {}", fmt_operand(q.a), q.c as i32)
            }
        }
        Layout::AIndex => format!("{}", q.a),
        Layout::AJump => format!("{}, {}", fmt_operand(q.a), q.c),
        Layout::Sizes => format!("volatile={}, tempor={}", q.a, q.b),
    };
    let mut s = format!("{name:<10} {ops}");
    // Always show raw fields that the layout does not cover, so nothing is hidden.
    let extra = match layout(q.opcode) {
        Layout::Nothing | Layout::Unknown => q.a != 0 || q.b != 0 || q.c != 0,
        Layout::A | Layout::AIndex => q.b != 0 || q.c != 0,
        Layout::Ab | Layout::Sizes => q.c != 0,
        Layout::Abc16 => q.c >> 16 != 0,
        Layout::AImm | Layout::AJump => q.b != 0,
    };
    if extra {
        use std::fmt::Write as _;
        let _ = write!(s, "   ; raw a={:#06x} b={:#06x} c={:#010x}", q.a, q.b, q.c);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p32(s: &str) -> Vec<u8> {
        let mut v = (s.len() as u32).to_le_bytes().to_vec();
        v.extend_from_slice(s.as_bytes());
        v
    }

    fn synthetic() -> Vec<u8> {
        let mut f = b"SBSCRIPT".to_vec();
        f.extend_from_slice(&1.5f32.to_le_bytes());
        f.extend_from_slice(&1u32.to_le_bytes());
        f.extend(p32("a.scs"));
        f.extend(p32("StartUp"));
        f.extend_from_slice(&2u32.to_le_bytes());
        f.extend_from_slice(&8u32.to_le_bytes());
        f.extend_from_slice(&[2, 0]);
        f.extend(p32("PlusSous"));
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&[7, 5]);
        f.extend_from_slice(b"Actor");
        f.extend(p32("MonActeur"));
        f.extend_from_slice(&4u32.to_le_bytes());
        f.extend_from_slice(&1u32.to_le_bytes());
        f.extend(p32("Initialize"));
        for v in [0u32, 2, 0, 4, 8, 16] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(&3u32.to_le_bytes());
        f.extend_from_slice(&[0x03, 8, 0, 16, 0, 0, 0, 0, 0]);
        f.extend_from_slice(&[0x13, 0x00, 0xc0, 0, 0, 1, 0, 0, 0]);
        f.extend_from_slice(&[0x06, 0, 0, 0, 0, 0, 0, 0, 0]);
        f
    }

    #[test]
    fn parses_header() {
        let h = parse_header(&synthetic()).unwrap();
        assert!((h.version - 1.5).abs() < f32::EPSILON);
        assert_eq!(h.class_count, 1);
        assert_eq!(h.source_path, "a.scs");
    }

    #[test]
    fn parses_classes_and_disassembles() {
        let s = parse(&synthetic()).unwrap();
        let c = s.class("StartUp").unwrap();
        assert_eq!(c.variables.len(), 2);
        assert_eq!(c.variables[1].type_name, "Actor");
        assert_eq!(c.variables[1].offset, 4);
        assert_eq!(c.functions[0].size_of_volatile, 8);
        assert_eq!(c.quads.len(), 3);
        assert_eq!(c.function_quads(0).len(), 3);
        assert_eq!(
            disassemble(&c.quads[0]).trim(),
            "enter      volatile=8, tempor=16"
        );
        assert_eq!(disassemble(&c.quads[1]).trim(), "op_13      tv0, 1");
        assert_eq!(disassemble(&c.quads[2]).trim(), "op_06");
    }

    #[test]
    fn garbage_does_not_panic() {
        let good = synthetic();
        for n in 0..good.len() {
            let _ = parse(&good[..n]);
        }
        for i in 0..good.len() {
            let mut bad = good.clone();
            bad[i] ^= 0xff;
            let _ = parse(&bad);
        }
    }
}
