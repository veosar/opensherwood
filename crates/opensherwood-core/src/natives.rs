//! The 265 engine functions the scripts call by number: the call table of
//! `docs/original/spec-script-vm.md` section 6, behind the wrapper protocol of VM-086.
//!
//! **The protocol.** Every id has a row in [`NATIVES`]: its *arity* (the cells the wrapper takes
//! off the instance's native argument buffer, the last pushed being the last argument), which of
//! its arguments the wrapper converts to a bool (non-zero -> 1, VM-086), and its *result
//! convention* - `void` leaves 0 in the native result register, `bool` the low 8 bits of the
//! native's value, `int` / `handle` the full 32-bit word. [`World::native_invoke`] is that
//! wrapper: it pops, converts, dispatches and writes the register, so no caller ever fabricates
//! a result or an argument. Arity 0 means the wrapper takes nothing and pushed cells stay in the
//! buffer (VM-087).
//!
//! **How much of a row this engine implements** is [`Kind`], one per id, and the kind is what
//! the call records in the taint model (ADR-0008, `vm::Assumption`):
//!
//! - [`Kind::Settled`] - the row is implemented as the specification states it; nothing recorded.
//! - [`Kind::Partial`] - the row is settled but its effect reaches a subsystem this engine models
//!   only in part (the camera, animation, the AI, navigation, the campaign): the engine's own
//!   behaviour is kept behind the new calling convention and every call records
//!   `Assumption::Policy(id)`.
//! - [`Kind::Stub`] - the effect is not modelled at all: the call is counted, the row's neutral
//!   value answered, and `Assumption::StubResult(id)` recorded.
//! - [`Kind::Unresolved`] - one of the eighteen ids of the specification's 4.3, whose effect is
//!   settled only up to a code, flag or consumer no reviewed specification names: the value is
//!   passed through verbatim and `Assumption::UnresolvedEffect(id)` recorded.
//! - [`Kind::Unknown`] - one of the twelve ids excluded from clearance (4.2): the placeholder
//!   the specification prescribes, always taken and recorded as
//!   `Assumption::UnknownNative(id)` (the specification's optional strict fault is not
//!   implemented); with `MissionSpec::lenient_natives` the call is also logged with its
//!   arguments in `VmState::unknown_calls`.
//!
//! **Recording.** The ids of [`RECORDING_NATIVES`] build a sequence element instead of acting
//! (VM-203): with a recording open (native 30) the element is appended and the row's result is 1;
//! without one the element is dropped with an error and the result is 0. The sequence machinery
//! that later runs those elements is *not* cleared (specification 3.7), so it stays the engine's
//! own and records `Assumption::ElementAdmission` / `Assumption::ElementDuration` where it
//! decides what the original's actors would decide.
//!
//! **Handles.** The original's handles are pointers with `0` = none; ours are table indices, so
//! [`NONE_HANDLE`] (-1) is the engine's null and index 0 is a real element. Natives 3 / 10 / 75
//! follow VM-030b over that representation (the cart table is empty: this engine has no carts).

use crate::ai::ActorStatus;
use crate::fixed::Fixed;
use crate::geom::point_in_polygon;
use crate::vm::{
    Assumption, Element, Fault, LOCATION_POINT_BIT, Location, MAX_MISSION_VARIABLES, MAX_QUEUE,
    MISSION_VARIABLE_GROWTH, Message, NATIVE_TABLE_SIZE, NONE_HANDLE, Objective, Program,
    Recording, SeqElement, UnknownCall, charge_budget, location_of_point,
};
use crate::world::{EntityKind, Gait, World};
use std::collections::BTreeSet;

/// Saturating increment of a per-id diagnostic counter.
fn count(map: &mut std::collections::BTreeMap<u32, u64>, id: u32) {
    let c = map.entry(id).or_insert(0);
    *c = c.saturating_add(1);
}

/// The result convention of a native's wrapper (`spec-script-vm.md` VM-086).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultKind {
    /// The register takes 0 whatever the native computed.
    Void,
    /// The register takes the low 8 bits of the native's value.
    Bool,
    /// The register takes the full 32-bit value.
    Int,
    /// The register takes the full 32-bit value, which is an element handle.
    Handle,
}

/// One row of the native call table: what the wrapper takes and leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeRow {
    /// Cells the wrapper pops, in push order.
    pub arity: u8,
    /// What the wrapper leaves in the native result register.
    pub result: ResultKind,
    /// Bit `k` set = argument `k` is converted to a bool by the wrapper (VM-086).
    pub bool_args: u16,
}

const fn row(arity: u8, result: ResultKind, bool_args: u16) -> NativeRow {
    NativeRow {
        arity,
        result,
        bool_args,
    }
}

/// How much of a native's row this engine implements (module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Implemented as the specification states it.
    Settled,
    /// Settled row, partly modelled effect: records `Assumption::Policy(id)`.
    Partial,
    /// Effect not modelled: records `Assumption::StubResult(id)`.
    Stub,
    /// Specification 4.3: records `Assumption::UnresolvedEffect(id)`.
    Unresolved,
    /// Specification 4.2: records `Assumption::UnknownNative(id)` (or faults in strict mode).
    Unknown,
}

pub const NATIVES: [NativeRow; NATIVE_TABLE_SIZE] = [
    row(2, ResultKind::Void, 0x0000),   // 0
    row(2, ResultKind::Void, 0x0000),   // 1
    row(1, ResultKind::Int, 0x0000),    // 2
    row(1, ResultKind::Handle, 0x0000), // 3
    row(1, ResultKind::Handle, 0x0000), // 4
    row(1, ResultKind::Handle, 0x0000), // 5
    row(1, ResultKind::Handle, 0x0000), // 6
    row(1, ResultKind::Handle, 0x0000), // 7
    row(1, ResultKind::Handle, 0x0000), // 8
    row(1, ResultKind::Handle, 0x0000), // 9
    row(1, ResultKind::Int, 0x0000),    // 10
    row(1, ResultKind::Int, 0x0000),    // 11
    row(1, ResultKind::Int, 0x0000),    // 12
    row(1, ResultKind::Int, 0x0000),    // 13
    row(1, ResultKind::Int, 0x0000),    // 14
    row(1, ResultKind::Int, 0x0000),    // 15
    row(1, ResultKind::Int, 0x0000),    // 16
    row(1, ResultKind::Void, 0x0000),   // 17
    row(1, ResultKind::Bool, 0x0000),   // 18
    row(2, ResultKind::Bool, 0x0000),   // 19
    row(1, ResultKind::Bool, 0x0000),   // 20
    row(1, ResultKind::Bool, 0x0000),   // 21
    row(1, ResultKind::Bool, 0x0001),   // 22
    row(0, ResultKind::Void, 0x0000),   // 23
    row(2, ResultKind::Void, 0x0000),   // 24
    row(2, ResultKind::Void, 0x0000),   // 25
    row(2, ResultKind::Void, 0x0002),   // 26
    row(1, ResultKind::Void, 0x0000),   // 27
    row(1, ResultKind::Void, 0x0000),   // 28
    row(0, ResultKind::Void, 0x0000),   // 29
    row(0, ResultKind::Bool, 0x0000),   // 30
    row(0, ResultKind::Bool, 0x0000),   // 31
    row(0, ResultKind::Int, 0x0000),    // 32
    row(1, ResultKind::Bool, 0x0000),   // 33
    row(1, ResultKind::Bool, 0x0000),   // 34
    row(1, ResultKind::Bool, 0x0000),   // 35
    row(1, ResultKind::Bool, 0x0001),   // 36
    row(3, ResultKind::Bool, 0x0004),   // 37
    row(2, ResultKind::Bool, 0x0002),   // 38
    row(1, ResultKind::Bool, 0x0000),   // 39
    row(0, ResultKind::Bool, 0x0000),   // 40
    row(1, ResultKind::Bool, 0x0000),   // 41
    row(2, ResultKind::Bool, 0x0000),   // 42
    row(2, ResultKind::Void, 0x0000),   // 43
    row(4, ResultKind::Void, 0x0000),   // 44
    row(3, ResultKind::Bool, 0x0000),   // 45
    row(4, ResultKind::Bool, 0x0000),   // 46
    row(4, ResultKind::Bool, 0x0000),   // 47
    row(2, ResultKind::Bool, 0x0000),   // 48
    row(2, ResultKind::Bool, 0x0000),   // 49
    row(2, ResultKind::Bool, 0x0000),   // 50
    row(2, ResultKind::Bool, 0x0000),   // 51
    row(1, ResultKind::Bool, 0x0000),   // 52
    row(1, ResultKind::Bool, 0x0000),   // 53
    row(0, ResultKind::Bool, 0x0000),   // 54
    row(0, ResultKind::Bool, 0x0000),   // 55
    row(1, ResultKind::Bool, 0x0000),   // 56
    row(4, ResultKind::Bool, 0x0000),   // 57
    row(1, ResultKind::Bool, 0x0000),   // 58
    row(3, ResultKind::Bool, 0x0000),   // 59
    row(3, ResultKind::Bool, 0x0000),   // 60
    row(2, ResultKind::Bool, 0x0000),   // 61
    row(3, ResultKind::Bool, 0x0000),   // 62
    row(3, ResultKind::Bool, 0x0000),   // 63
    row(3, ResultKind::Bool, 0x0000),   // 64
    row(1, ResultKind::Bool, 0x0000),   // 65
    row(1, ResultKind::Bool, 0x0000),   // 66
    row(1, ResultKind::Void, 0x0000),   // 67
    row(1, ResultKind::Void, 0x0000),   // 68
    row(2, ResultKind::Bool, 0x0000),   // 69
    row(6, ResultKind::Bool, 0x0000),   // 70
    row(8, ResultKind::Bool, 0x0000),   // 71
    row(1, ResultKind::Void, 0x0000),   // 72
    row(1, ResultKind::Void, 0x0000),   // 73
    row(0, ResultKind::Handle, 0x0000), // 74
    row(0, ResultKind::Int, 0x0000),    // 75
    row(1, ResultKind::Bool, 0x0000),   // 76
    row(1, ResultKind::Bool, 0x0000),   // 77
    row(1, ResultKind::Bool, 0x0000),   // 78
    row(1, ResultKind::Bool, 0x0000),   // 79
    row(1, ResultKind::Bool, 0x0000),   // 80
    row(1, ResultKind::Bool, 0x0000),   // 81
    row(1, ResultKind::Bool, 0x0000),   // 82
    row(1, ResultKind::Bool, 0x0000),   // 83
    row(1, ResultKind::Bool, 0x0000),   // 84
    row(1, ResultKind::Bool, 0x0000),   // 85
    row(2, ResultKind::Bool, 0x0000),   // 86
    row(1, ResultKind::Bool, 0x0000),   // 87
    row(1, ResultKind::Bool, 0x0000),   // 88
    row(1, ResultKind::Bool, 0x0000),   // 89
    row(1, ResultKind::Bool, 0x0000),   // 90
    row(1, ResultKind::Int, 0x0000),    // 91
    row(2, ResultKind::Void, 0x0000),   // 92
    row(1, ResultKind::Int, 0x0000),    // 93
    row(2, ResultKind::Bool, 0x0000),   // 94
    row(1, ResultKind::Handle, 0x0000), // 95
    row(2, ResultKind::Bool, 0x0000),   // 96
    row(2, ResultKind::Bool, 0x0000),   // 97
    row(2, ResultKind::Bool, 0x0000),   // 98
    row(1, ResultKind::Bool, 0x0000),   // 99
    row(1, ResultKind::Int, 0x0000),    // 100
    row(1, ResultKind::Int, 0x0000),    // 101
    row(3, ResultKind::Void, 0x0004),   // 102
    row(1, ResultKind::Bool, 0x0000),   // 103
    row(2, ResultKind::Bool, 0x0000),   // 104
    row(1, ResultKind::Void, 0x0000),   // 105
    row(0, ResultKind::Bool, 0x0000),   // 106
    row(1, ResultKind::Void, 0x0001),   // 107
    row(3, ResultKind::Bool, 0x0000),   // 108
    row(2, ResultKind::Void, 0x0000),   // 109
    row(4, ResultKind::Void, 0x0000),   // 110
    row(0, ResultKind::Handle, 0x0000), // 111
    row(1, ResultKind::Bool, 0x0000),   // 112
    row(1, ResultKind::Bool, 0x0000),   // 113
    row(1, ResultKind::Bool, 0x0000),   // 114
    row(3, ResultKind::Bool, 0x0004),   // 115
    row(2, ResultKind::Bool, 0x0000),   // 116
    row(3, ResultKind::Bool, 0x0000),   // 117
    row(2, ResultKind::Int, 0x0000),    // 118
    row(0, ResultKind::Bool, 0x0000),   // 119
    row(0, ResultKind::Bool, 0x0000),   // 120
    row(0, ResultKind::Int, 0x0000),    // 121
    row(0, ResultKind::Int, 0x0000),    // 122
    row(2, ResultKind::Bool, 0x0000),   // 123
    row(1, ResultKind::Int, 0x0000),    // 124
    row(2, ResultKind::Bool, 0x0000),   // 125
    row(1, ResultKind::Int, 0x0000),    // 126
    row(2, ResultKind::Bool, 0x0000),   // 127
    row(1, ResultKind::Int, 0x0000),    // 128
    row(3, ResultKind::Bool, 0x0000),   // 129
    row(3, ResultKind::Void, 0x0004),   // 130
    row(3, ResultKind::Void, 0x0004),   // 131
    row(2, ResultKind::Void, 0x0000),   // 132
    row(3, ResultKind::Void, 0x0000),   // 133
    row(2, ResultKind::Void, 0x0002),   // 134
    row(1, ResultKind::Void, 0x0000),   // 135
    row(2, ResultKind::Void, 0x0000),   // 136
    row(2, ResultKind::Void, 0x0000),   // 137
    row(2, ResultKind::Void, 0x0002),   // 138
    row(1, ResultKind::Void, 0x0001),   // 139
    row(2, ResultKind::Void, 0x0000),   // 140
    row(1, ResultKind::Int, 0x0000),    // 141
    row(1, ResultKind::Bool, 0x0000),   // 142
    row(2, ResultKind::Bool, 0x0002),   // 143
    row(1, ResultKind::Bool, 0x0000),   // 144
    row(1, ResultKind::Bool, 0x0000),   // 145
    row(1, ResultKind::Bool, 0x0000),   // 146
    row(0, ResultKind::Bool, 0x0000),   // 147
    row(0, ResultKind::Bool, 0x0000),   // 148
    row(1, ResultKind::Bool, 0x0000),   // 149
    row(1, ResultKind::Bool, 0x0000),   // 150
    row(1, ResultKind::Bool, 0x0000),   // 151
    row(1, ResultKind::Bool, 0x0000),   // 152
    row(2, ResultKind::Bool, 0x0000),   // 153
    row(2, ResultKind::Bool, 0x0000),   // 154
    row(1, ResultKind::Void, 0x0000),   // 155
    row(2, ResultKind::Void, 0x0000),   // 156
    row(2, ResultKind::Void, 0x0002),   // 157
    row(1, ResultKind::Handle, 0x0000), // 158
    row(0, ResultKind::Handle, 0x0000), // 159
    row(2, ResultKind::Int, 0x0000),    // 160
    row(1, ResultKind::Int, 0x0000),    // 161
    row(1, ResultKind::Void, 0x0000),   // 162
    row(0, ResultKind::Int, 0x0000),    // 163
    row(1, ResultKind::Handle, 0x0000), // 164
    row(1, ResultKind::Void, 0x0000),   // 165
    row(1, ResultKind::Void, 0x0000),   // 166
    row(0, ResultKind::Int, 0x0000),    // 167
    row(1, ResultKind::Handle, 0x0000), // 168
    row(1, ResultKind::Bool, 0x0000),   // 169
    row(0, ResultKind::Bool, 0x0000),   // 170
    row(0, ResultKind::Int, 0x0000),    // 171
    row(0, ResultKind::Int, 0x0000),    // 172
    row(0, ResultKind::Bool, 0x0000),   // 173
    row(0, ResultKind::Int, 0x0000),    // 174
    row(2, ResultKind::Void, 0x0000),   // 175
    row(2, ResultKind::Void, 0x0000),   // 176
    row(2, ResultKind::Void, 0x0002),   // 177
    row(1, ResultKind::Void, 0x0000),   // 178
    row(1, ResultKind::Void, 0x0000),   // 179
    row(2, ResultKind::Void, 0x0002),   // 180
    row(1, ResultKind::Bool, 0x0000),   // 181
    row(1, ResultKind::Bool, 0x0000),   // 182
    row(1, ResultKind::Bool, 0x0000),   // 183
    row(1, ResultKind::Bool, 0x0000),   // 184
    row(1, ResultKind::Bool, 0x0000),   // 185
    row(2, ResultKind::Void, 0x0002),   // 186
    row(2, ResultKind::Void, 0x0002),   // 187
    row(2, ResultKind::Void, 0x0002),   // 188
    row(2, ResultKind::Void, 0x0002),   // 189
    row(3, ResultKind::Void, 0x0004),   // 190
    row(2, ResultKind::Void, 0x0001),   // 191
    row(0, ResultKind::Handle, 0x0000), // 192
    row(1, ResultKind::Int, 0x0000),    // 193
    row(2, ResultKind::Void, 0x0000),   // 194
    row(1, ResultKind::Int, 0x0000),    // 195
    row(2, ResultKind::Void, 0x0000),   // 196
    row(2, ResultKind::Int, 0x0000),    // 197
    row(3, ResultKind::Void, 0x0000),   // 198
    row(3, ResultKind::Void, 0x0000),   // 199
    row(2, ResultKind::Void, 0x0000),   // 200
    row(1, ResultKind::Handle, 0x0000), // 201
    row(1, ResultKind::Void, 0x0000),   // 202
    row(1, ResultKind::Void, 0x0000),   // 203
    row(1, ResultKind::Int, 0x0000),    // 204
    row(2, ResultKind::Handle, 0x0000), // 205
    row(2, ResultKind::Int, 0x0000),    // 206
    row(2, ResultKind::Int, 0x0000),    // 207
    row(2, ResultKind::Int, 0x0000),    // 208
    row(2, ResultKind::Bool, 0x0000),   // 209
    row(1, ResultKind::Bool, 0x0000),   // 210
    row(0, ResultKind::Handle, 0x0000), // 211
    row(4, ResultKind::Bool, 0x0000),   // 212
    row(3, ResultKind::Handle, 0x0000), // 213
    row(1, ResultKind::Void, 0x0000),   // 214
    row(1, ResultKind::Handle, 0x0000), // 215
    row(0, ResultKind::Int, 0x0000),    // 216
    row(1, ResultKind::Handle, 0x0000), // 217
    row(2, ResultKind::Void, 0x0000),   // 218
    row(1, ResultKind::Void, 0x0000),   // 219
    row(1, ResultKind::Void, 0x0000),   // 220
    row(1, ResultKind::Bool, 0x0000),   // 221
    row(1, ResultKind::Bool, 0x0000),   // 222
    row(1, ResultKind::Bool, 0x0000),   // 223
    row(4, ResultKind::Int, 0x0000),    // 224
    row(1, ResultKind::Void, 0x0000),   // 225
    row(1, ResultKind::Void, 0x0001),   // 226
    row(1, ResultKind::Void, 0x0000),   // 227
    row(3, ResultKind::Void, 0x0000),   // 228
    row(1, ResultKind::Void, 0x0000),   // 229
    row(1, ResultKind::Bool, 0x0000),   // 230
    row(1, ResultKind::Bool, 0x0000),   // 231
    row(1, ResultKind::Void, 0x0000),   // 232
    row(2, ResultKind::Void, 0x0000),   // 233
    row(0, ResultKind::Bool, 0x0000),   // 234
    row(1, ResultKind::Bool, 0x0000),   // 235
    row(0, ResultKind::Int, 0x0000),    // 236
    row(1, ResultKind::Void, 0x0000),   // 237
    row(0, ResultKind::Int, 0x0000),    // 238
    row(0, ResultKind::Void, 0x0000),   // 239
    row(1, ResultKind::Bool, 0x0000),   // 240
    row(3, ResultKind::Void, 0x0000),   // 241
    row(3, ResultKind::Void, 0x0000),   // 242
    row(1, ResultKind::Bool, 0x0000),   // 243
    row(2, ResultKind::Void, 0x0002),   // 244
    row(0, ResultKind::Int, 0x0000),    // 245
    row(1, ResultKind::Bool, 0x0000),   // 246
    row(1, ResultKind::Void, 0x0000),   // 247
    row(1, ResultKind::Bool, 0x0000),   // 248
    row(0, ResultKind::Int, 0x0000),    // 249
    row(1, ResultKind::Handle, 0x0000), // 250
    row(0, ResultKind::Void, 0x0000),   // 251
    row(1, ResultKind::Void, 0x0000),   // 252
    row(1, ResultKind::Bool, 0x0000),   // 253
    row(2, ResultKind::Void, 0x0002),   // 254
    row(1, ResultKind::Bool, 0x0000),   // 255
    row(1, ResultKind::Int, 0x0000),    // 256
    row(2, ResultKind::Void, 0x0002),   // 257
    row(1, ResultKind::Bool, 0x0000),   // 258
    row(1, ResultKind::Int, 0x0000),    // 259
    row(2, ResultKind::Void, 0x0000),   // 260
    row(0, ResultKind::Bool, 0x0000),   // 261
    row(1, ResultKind::Void, 0x0000),   // 262
    row(2, ResultKind::Void, 0x0000),   // 263
    row(3, ResultKind::Void, 0x0004),   // 264
];

pub const NATIVE_KINDS: [Kind; NATIVE_TABLE_SIZE] = [
    Kind::Settled,    // 0
    Kind::Settled,    // 1
    Kind::Settled,    // 2
    Kind::Settled,    // 3
    Kind::Partial,    // 4
    Kind::Partial,    // 5
    Kind::Settled,    // 6
    Kind::Stub,       // 7
    Kind::Partial,    // 8
    Kind::Settled,    // 9
    Kind::Settled,    // 10
    Kind::Partial,    // 11
    Kind::Partial,    // 12
    Kind::Unknown,    // 13
    Kind::Stub,       // 14
    Kind::Partial,    // 15
    Kind::Settled,    // 16
    Kind::Partial,    // 17
    Kind::Partial,    // 18
    Kind::Partial,    // 19
    Kind::Partial,    // 20
    Kind::Partial,    // 21
    Kind::Stub,       // 22
    Kind::Stub,       // 23
    Kind::Stub,       // 24
    Kind::Stub,       // 25
    Kind::Settled,    // 26
    Kind::Settled,    // 27
    Kind::Settled,    // 28
    Kind::Settled,    // 29
    Kind::Settled,    // 30
    Kind::Settled,    // 31
    Kind::Settled,    // 32
    Kind::Partial,    // 33
    Kind::Partial,    // 34
    Kind::Stub,       // 35
    Kind::Stub,       // 36
    Kind::Stub,       // 37
    Kind::Stub,       // 38
    Kind::Stub,       // 39
    Kind::Stub,       // 40
    Kind::Stub,       // 41
    Kind::Partial,    // 42
    Kind::Settled,    // 43
    Kind::Settled,    // 44
    Kind::Unresolved, // 45
    Kind::Unresolved, // 46
    Kind::Unresolved, // 47
    Kind::Partial,    // 48
    Kind::Stub,       // 49
    Kind::Stub,       // 50
    Kind::Stub,       // 51
    Kind::Partial,    // 52
    Kind::Partial,    // 53
    Kind::Stub,       // 54
    Kind::Stub,       // 55
    Kind::Settled,    // 56
    Kind::Unresolved, // 57
    Kind::Settled,    // 58
    Kind::Unresolved, // 59
    Kind::Stub,       // 60
    Kind::Stub,       // 61
    Kind::Unresolved, // 62
    Kind::Unresolved, // 63
    Kind::Partial,    // 64
    Kind::Stub,       // 65
    Kind::Stub,       // 66
    Kind::Stub,       // 67
    Kind::Stub,       // 68
    Kind::Stub,       // 69
    Kind::Unresolved, // 70
    Kind::Unresolved, // 71
    Kind::Stub,       // 72
    Kind::Stub,       // 73
    Kind::Settled,    // 74
    Kind::Settled,    // 75
    Kind::Partial,    // 76
    Kind::Settled,    // 77
    Kind::Settled,    // 78
    Kind::Settled,    // 79
    Kind::Settled,    // 80
    Kind::Partial,    // 81
    Kind::Partial,    // 82
    Kind::Partial,    // 83
    Kind::Partial,    // 84
    Kind::Settled,    // 85
    Kind::Settled,    // 86
    Kind::Settled,    // 87
    Kind::Partial,    // 88
    Kind::Stub,       // 89
    Kind::Partial,    // 90
    Kind::Unknown,    // 91
    Kind::Unknown,    // 92
    Kind::Partial,    // 93
    Kind::Partial,    // 94
    Kind::Settled,    // 95
    Kind::Settled,    // 96
    Kind::Settled,    // 97
    Kind::Partial,    // 98
    Kind::Stub,       // 99
    Kind::Unresolved, // 100
    Kind::Partial,    // 101
    Kind::Stub,       // 102
    Kind::Partial,    // 103
    Kind::Stub,       // 104
    Kind::Stub,       // 105
    Kind::Unresolved, // 106
    Kind::Unresolved, // 107
    Kind::Partial,    // 108
    Kind::Settled,    // 109
    Kind::Settled,    // 110
    Kind::Settled,    // 111
    Kind::Partial,    // 112
    Kind::Settled,    // 113
    Kind::Settled,    // 114
    Kind::Stub,       // 115
    Kind::Stub,       // 116
    Kind::Partial,    // 117
    Kind::Partial,    // 118
    Kind::Partial,    // 119
    Kind::Partial,    // 120
    Kind::Partial,    // 121
    Kind::Partial,    // 122
    Kind::Partial,    // 123
    Kind::Partial,    // 124
    Kind::Partial,    // 125
    Kind::Partial,    // 126
    Kind::Settled,    // 127
    Kind::Partial,    // 128
    Kind::Partial,    // 129
    Kind::Settled,    // 130
    Kind::Settled,    // 131
    Kind::Partial,    // 132
    Kind::Partial,    // 133
    Kind::Partial,    // 134
    Kind::Partial,    // 135
    Kind::Unresolved, // 136
    Kind::Partial,    // 137
    Kind::Stub,       // 138
    Kind::Stub,       // 139
    Kind::Partial,    // 140
    Kind::Stub,       // 141
    Kind::Stub,       // 142
    Kind::Stub,       // 143
    Kind::Partial,    // 144
    Kind::Partial,    // 145
    Kind::Partial,    // 146
    Kind::Stub,       // 147
    Kind::Stub,       // 148
    Kind::Stub,       // 149
    Kind::Stub,       // 150
    Kind::Stub,       // 151
    Kind::Stub,       // 152
    Kind::Partial,    // 153
    Kind::Partial,    // 154
    Kind::Settled,    // 155
    Kind::Stub,       // 156
    Kind::Unknown,    // 157
    Kind::Partial,    // 158
    Kind::Settled,    // 159
    Kind::Settled,    // 160
    Kind::Settled,    // 161
    Kind::Settled,    // 162
    Kind::Stub,       // 163
    Kind::Stub,       // 164
    Kind::Unresolved, // 165
    Kind::Stub,       // 166
    Kind::Stub,       // 167
    Kind::Partial,    // 168
    Kind::Stub,       // 169
    Kind::Stub,       // 170
    Kind::Stub,       // 171
    Kind::Stub,       // 172
    Kind::Unknown,    // 173
    Kind::Stub,       // 174
    Kind::Partial,    // 175
    Kind::Stub,       // 176
    Kind::Stub,       // 177
    Kind::Partial,    // 178
    Kind::Partial,    // 179
    Kind::Stub,       // 180
    Kind::Stub,       // 181
    Kind::Settled,    // 182
    Kind::Settled,    // 183
    Kind::Settled,    // 184
    Kind::Settled,    // 185
    Kind::Settled,    // 186
    Kind::Settled,    // 187
    Kind::Settled,    // 188
    Kind::Settled,    // 189
    Kind::Unknown,    // 190
    Kind::Settled,    // 191
    Kind::Settled,    // 192
    Kind::Settled,    // 193
    Kind::Settled,    // 194
    Kind::Settled,    // 195
    Kind::Settled,    // 196
    Kind::Settled,    // 197
    Kind::Settled,    // 198
    Kind::Stub,       // 199
    Kind::Stub,       // 200
    Kind::Partial,    // 201
    Kind::Partial,    // 202
    Kind::Partial,    // 203
    Kind::Partial,    // 204
    Kind::Partial,    // 205
    Kind::Settled,    // 206
    Kind::Settled,    // 207
    Kind::Settled,    // 208
    Kind::Stub,       // 209
    Kind::Stub,       // 210
    Kind::Partial,    // 211
    Kind::Unresolved, // 212
    Kind::Settled,    // 213
    Kind::Stub,       // 214
    Kind::Stub,       // 215
    Kind::Settled,    // 216
    Kind::Settled,    // 217
    Kind::Stub,       // 218
    Kind::Stub,       // 219
    Kind::Stub,       // 220
    Kind::Stub,       // 221
    Kind::Stub,       // 222
    Kind::Partial,    // 223
    Kind::Unknown,    // 224
    Kind::Unknown,    // 225
    Kind::Stub,       // 226
    Kind::Unknown,    // 227
    Kind::Stub,       // 228
    Kind::Partial,    // 229
    Kind::Settled,    // 230
    Kind::Partial,    // 231
    Kind::Stub,       // 232
    Kind::Stub,       // 233
    Kind::Partial,    // 234
    Kind::Partial,    // 235
    Kind::Settled,    // 236
    Kind::Settled,    // 237
    Kind::Unknown,    // 238
    Kind::Stub,       // 239
    Kind::Settled,    // 240
    Kind::Unknown,    // 241
    Kind::Stub,       // 242
    Kind::Stub,       // 243
    Kind::Stub,       // 244
    Kind::Settled,    // 245
    Kind::Settled,    // 246
    Kind::Stub,       // 247
    Kind::Partial,    // 248
    Kind::Settled,    // 249
    Kind::Settled,    // 250
    Kind::Stub,       // 251
    Kind::Partial,    // 252
    Kind::Stub,       // 253
    Kind::Unresolved, // 254
    Kind::Stub,       // 255
    Kind::Stub,       // 256
    Kind::Partial,    // 257
    Kind::Stub,       // 258
    Kind::Unresolved, // 259
    Kind::Unresolved, // 260
    Kind::Unknown,    // 261
    Kind::Stub,       // 262
    Kind::Stub,       // 263
    Kind::Stub,       // 264
];

/// The row of native `id`; `None` beyond the table (VM-085: the original has no range check,
/// the engine refuses such a program at `Program::validate`).
#[must_use]
pub fn native_row(id: u32) -> Option<NativeRow> {
    NATIVES.get(id as usize).copied()
}

/// How much of native `id` this engine implements.
#[must_use]
pub fn native_kind(id: u32) -> Option<Kind> {
    NATIVE_KINDS.get(id as usize).copied()
}

/// The twelve ids excluded from clearance (`spec-script-vm.md` 4.2).
pub const EXCLUDED: &[u32] = &[13, 91, 92, 157, 173, 190, 224, 225, 227, 238, 241, 261];

/// The eighteen ids whose effect is settled only up to an unresolved code, flag or consumer
/// (`spec-script-vm.md` 4.3).
pub const UNRESOLVED_EFFECT: &[u32] = &[
    45, 46, 47, 57, 59, 62, 63, 70, 71, 100, 106, 107, 136, 165, 212, 254, 259, 260,
];

/// The natives that build a sequence element instead of acting (VM-203; the "records" rows of
/// section 6). Native 32 is the barrier, which is not an element of its own (VM-201).
pub const RECORDING_NATIVES: &[u32] = &[
    33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56,
    57, 59, 60, 61, 62, 63, 64, 65, 67, 68, 69, 70, 71, 72, 73, 203, 212, 226, 243,
];

/// The placeholder an excluded native answers with (`spec-script-vm.md` 4.2, an explicit
/// choice recorded as `Assumption::UnknownNative`): 13 inverts native 6, 91 / 173 / 224 / 238 /
/// 261 answer 0, the rest are no-ops.
fn excluded_placeholder(world: &World, id: u32, args: &[i32]) -> i32 {
    match id {
        // The scripts' evident intent: the inverse of native 6 over the location table.
        13 => {
            let handle = args.first().copied().unwrap_or(NONE_HANDLE);
            let known = world
                .vm
                .as_ref()
                .is_some_and(|vm| handle >= 0 && (handle as usize) < vm.program.locations.len());
            if known { handle } else { NONE_HANDLE }
        }
        _ => 0,
    }
}

/// Facing units per sixteenth of a turn: the scripts' sixteen directions (natives 93 / 94 / 133,
/// 0..=15) on the entities' 256-unit facing. Which direction is 0 is not settled; the engine
/// takes direction 0 as facing 0 (the `+x` axis, `world::facing_of`) and counts the same way, a
/// choice pinned by `facing_natives_map_sixteen_directions_onto_facing256` and recorded as a
/// `Policy` row.
pub const FACING_UNITS_PER_DIRECTION: i32 = 16;

/// The script-visible campaign values of natives 195 / 196: twenty values with indices 0..19
/// (`spec-script-vm.md` rows 195 / 196; their meaning and their place in the save file belong to
/// the campaign specification).
pub const CAMPAIGN_VALUES: usize = 20;
/// Custom values a script keeps per NPC (natives 197 / 198): indices 0..9.
pub const NPC_VALUES: usize = 10;
/// Highest remark id natives 69 and 264 accept (ids 0..119).
pub const MAX_REMARK_ID: i32 = 120;
/// Smallest message id native 71 accepts (VM-122).
pub const MIN_CUSTOM_MESSAGE: i32 = 1000;
/// The action id that means "no action" (natives 101, callback `ActionChange`).
pub const NO_ACTION: i32 = 283;
/// The answer natives 91 and 259 give for a state outside their code set.
pub const NO_STATE: i32 = 666;

/// Decode a packed actor position from a location value.
#[must_use]
pub fn unpack_point(v: i32) -> Option<(i32, i32)> {
    if v >= 0 && v & LOCATION_POINT_BIT != 0 {
        Some(((v >> 15) & 0x7fff, v & 0x7fff))
    } else {
        None
    }
}

/// An argument that the row's arity guarantees; 0 keeps the accessor total.
fn arg(args: &[i32], i: usize) -> i32 {
    args.get(i).copied().unwrap_or(0)
}

/// Polygon of a location value (zones) borrowed from the program, if it is one.
fn polygon_in(program: &Program, value: i32) -> Option<&[(i32, i32)]> {
    if value < 0 {
        return None;
    }
    match program.locations.get(value as usize)? {
        Location::Polygon(p) => Some(p.as_slice()),
        Location::Point { .. } => None,
    }
}

impl World {
    /// The wrapper of `spec-script-vm.md` VM-086: take the row's arity off the running
    /// instance's native argument buffer (last pushed = last argument), convert the bool
    /// arguments, dispatch, and leave the row's result convention in the native result
    /// register. Returns `false` when the running callback must terminate (`spec-script-vm.md`
    /// 8.1: an unknown native in strict mode, an arithmetic trap, a container range error).
    pub(crate) fn native_invoke(&mut self, id: u32) -> bool {
        let Some(class) = self
            .vm
            .as_ref()
            .and_then(|vm| vm.frames.last())
            .map(|f| f.class as usize)
        else {
            return false;
        };
        let Some(row) = native_row(id) else {
            if let Some(vm) = self.vm.as_mut() {
                vm.set_fault(Fault::UnknownNative(id));
            }
            return false;
        };
        // (1) the wrapper pops its arity, (2) converts the bool arguments.
        let mut args = Vec::with_capacity(row.arity as usize);
        {
            let Some(vm) = self.vm.as_mut() else {
                return false;
            };
            let short = vm.instances[class].args.len() < row.arity as usize;
            if short {
                // The original reads the cells that are there whatever was left in them
                // (VM-087): an unchecked read with a deterministic 0 (8.1), the callback
                // continues.
                vm.counters.faults = vm.counters.faults.saturating_add(1);
                vm.set_fault(Fault::UncheckedAccess(id));
            }
            let buf = &mut vm.instances[class].args;
            let take = (row.arity as usize).min(buf.len());
            let mut popped = buf.split_off(buf.len() - take);
            while popped.len() < row.arity as usize {
                popped.insert(0, 0);
            }
            for (k, v) in popped.into_iter().enumerate() {
                let v = if row.bool_args & (1 << k) != 0 {
                    i32::from(v != 0)
                } else {
                    v
                };
                args.push(v);
            }
        }
        // A native that fails with `Err` terminates the callback (8.1: the U-write, T, X and
        // strict-mode rows); everything the wrapper already did stands, the arity included.
        let value = match self.native_dispatch(id, &args) {
            Ok(v) => v,
            Err(f) => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.counters.faults = vm.counters.faults.saturating_add(1);
                    vm.set_fault(f);
                }
                return false;
            }
        };
        // (3) the result convention.
        let result = match row.result {
            ResultKind::Void => 0,
            ResultKind::Bool => value & 0xff,
            ResultKind::Int | ResultKind::Handle => value,
        };
        if let Some(vm) = self.vm.as_mut() {
            vm.instances[class].native_result = result;
        }
        true
    }

    /// Run native `id` with `args` as if a wrapper had popped and converted them: the
    /// entry point of the tools and the tests, and the second half of
    /// [`World::native_invoke`]. A terminating failure answers the row's failure value here.
    pub fn native_call(&mut self, id: u32, args: &[i32]) -> i32 {
        let Some(row) = native_row(id) else {
            return 0;
        };
        let conv: Vec<i32> = args
            .iter()
            .enumerate()
            .map(|(k, &v)| {
                if row.bool_args & (1 << k) != 0 {
                    i32::from(v != 0)
                } else {
                    v
                }
            })
            .collect();
        let value = match self.native_dispatch(id, &conv) {
            Ok(v) => v,
            Err(f) => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.counters.faults = vm.counters.faults.saturating_add(1);
                    vm.set_fault(f);
                }
                0
            }
        };
        match row.result {
            ResultKind::Void => 0,
            ResultKind::Bool => value & 0xff,
            ResultKind::Int | ResultKind::Handle => value,
        }
    }

    /// Record what calling `id` costs the taint model, then run the native: the recording step
    /// of VM-203 for a recording native, the placeholder of 4.2 for an excluded one, the body
    /// otherwise. `Err` is a failure class that the caller turns into a fault.
    fn native_dispatch(&mut self, id: u32, args: &[i32]) -> Result<i32, Fault> {
        let kind = native_kind(id).unwrap_or(Kind::Unknown);
        if let Some(vm) = self.vm.as_mut() {
            count(&mut vm.counters.native_calls, id);
            match kind {
                Kind::Settled => {}
                Kind::Partial => vm.assume(Assumption::Policy(id)),
                Kind::Stub => {
                    count(&mut vm.counters.stub_natives, id);
                    vm.assume(Assumption::StubResult(id));
                }
                Kind::Unresolved => vm.assume(Assumption::UnresolvedEffect(id)),
                Kind::Unknown => {
                    // 4.2: the placeholder is the engine's explicit choice and is always
                    // taken, recorded as `UnknownNative(id)`; the specification's optional
                    // strict "unresolved operation" fault is not implemented. With
                    // `MissionSpec::lenient_natives` the call is also logged with its
                    // arguments for diagnosis.
                    count(&mut vm.counters.unknown_natives, id);
                    vm.assume(Assumption::UnknownNative(id));
                    if vm.lenient && vm.unknown_calls.len() < MAX_QUEUE {
                        vm.unknown_calls.push(UnknownCall {
                            id,
                            args: args.to_vec(),
                        });
                    }
                }
            }
        }
        if kind == Kind::Unknown {
            return Ok(excluded_placeholder(self, id, args));
        }
        if RECORDING_NATIVES.contains(&id) {
            return Ok(self.native_record(id, args));
        }
        self.native_body(id, args)
    }

    /// The recording step of VM-203: with a recording open the element is appended and tagged
    /// with the current level, and the native answers 1; without one the element is dropped
    /// with an error and the native answers 0.
    fn native_record(&mut self, id: u32, args: &[i32]) -> i32 {
        let Some(element) = self.sequence_element(id, args) else {
            return 0;
        };
        let Some(vm) = self.vm.as_mut() else {
            return 0;
        };
        let Some(rec) = vm.collecting.as_mut() else {
            vm.counters.recording_dropped = vm.counters.recording_dropped.saturating_add(1);
            return 0;
        };
        if rec.elements.len() >= MAX_QUEUE {
            return 0;
        }
        rec.elements.push(element);
        rec.last_level = rec.level;
        1
    }
}

impl World {
    /// The body of a native that neither records nor is excluded. Each arm cites its row of
    /// `spec-script-vm.md` section 6 and answers the row's failure value on an error; the
    /// wrapper of [`World::native_invoke`] applies the result convention afterwards.
    #[allow(
        clippy::too_many_lines,
        clippy::match_same_arms,
        clippy::float_cmp,
        clippy::single_match_else,
        clippy::single_match,
        clippy::manual_range_patterns
    )]
    pub(crate) fn native_body(&mut self, id: u32, args: &[i32]) -> Result<i32, Fault> {
        let a0 = arg(args, 0);
        match id {
            // 0 (k, v): mission variable k := v, growing the array to k + 16 (VM-020); a
            // negative k is an unchecked write.
            0 => {
                let (k, v) = (a0, arg(args, 1));
                if k < 0 {
                    return Err(Fault::UncheckedAccess(0));
                }
                let Some(vm) = self.vm.as_mut() else {
                    return Ok(0);
                };
                let k = k as usize;
                if k >= vm.mission_vars.len() {
                    let size = k
                        .saturating_add(MISSION_VARIABLE_GROWTH)
                        .min(MAX_MISSION_VARIABLES);
                    vm.mission_vars.resize(size, 0);
                }
                if let Some(slot) = vm.mission_vars.get_mut(k) {
                    *slot = v;
                }
                Ok(0)
            }
            // 1 (k, v): store only into a declared variable, else an error and no effect.
            1 => {
                let (k, v) = (a0, arg(args, 1));
                if let Some(vm) = self.vm.as_mut() {
                    match usize::try_from(k)
                        .ok()
                        .and_then(|k| vm.mission_vars.get_mut(k))
                    {
                        Some(slot) => *slot = v,
                        None => {
                            vm.counters.native_errors = vm.counters.native_errors.saturating_add(1);
                        }
                    }
                }
                Ok(0)
            }
            // 2 (k) -> int: the value, or -1 outside the declared range.
            2 => Ok(self
                .vm
                .as_ref()
                .and_then(|vm| {
                    usize::try_from(a0)
                        .ok()
                        .and_then(|k| vm.mission_vars.get(k))
                })
                .copied()
                .unwrap_or(-1)),
            // 3 (i) -> handle: element i of the table (VM-030b): -1 is null without an error,
            // i < N mod 65536 the entry, the cart range beyond it (empty here), anything else
            // an error; i < -1 is an unchecked read.
            3 => {
                if a0 == NONE_HANDLE {
                    return Ok(NONE_HANDLE);
                }
                let n = self.vm.as_ref().map_or(0, |vm| vm.program.elements.len());
                if a0 < NONE_HANDLE {
                    self.record_read_fault(3);
                    return Ok(NONE_HANDLE);
                }
                let n16 = (n % 65536) as i32;
                if a0 < n16 {
                    return Ok(a0);
                }
                self.record_error();
                Ok(NONE_HANDLE)
            }
            // 10 (e) -> int: the index of e in the element table, else -1 (the cart table is
            // empty here).
            10 => {
                let n = self.vm.as_ref().map_or(0, |vm| vm.program.elements.len()) as i32;
                Ok(if a0 >= 0 && a0 < n { a0 } else { -1 })
            }
            // 75 () -> int: the element table's full size N (not truncated, unlike 3).
            75 => Ok(self
                .vm
                .as_ref()
                .map_or(0, |vm| vm.program.elements.len() as i32)),
            // 4 / 5 (i) -> handle: door / patch i of the map's list; -1 or out of range -> null.
            // The engine has no door or patch lists of its own: the handle is the index.
            4 | 5 => Ok(if a0 >= 0 { a0 } else { NONE_HANDLE }),
            // 6 (i) -> handle: location i of the level's list, i modulo 65536 and no upper
            // check (an unchecked read); -1 or an empty entry -> null.
            6 => {
                if a0 == NONE_HANDLE {
                    return Ok(NONE_HANDLE);
                }
                let k = ((a0 as u32) % 65536) as usize;
                let len = self.vm.as_ref().map_or(0, |vm| vm.program.locations.len());
                if k < len {
                    Ok(k as i32)
                } else {
                    self.record_read_fault(6);
                    Ok(NONE_HANDLE)
                }
            }
            // 7 (k) -> handle: sound source k; this engine has no sound sources.
            7 => Ok(NONE_HANDLE),
            // 8 (i) -> handle: building i of the map's list, **no check** (U). This engine
            // keeps no building list, so the handle is the index and there is nothing to read
            // past: the reading itself is the `Policy(8)` this row records.
            8 => Ok(a0),
            // 9 (i) -> handle: patrol path i (modulo 65536, no upper check); -1 -> null.
            9 => {
                if a0 == NONE_HANDLE {
                    return Ok(NONE_HANDLE);
                }
                let k = ((a0 as u32) % 65536) as usize;
                let len = self.vm.as_ref().map_or(0, |vm| vm.paths.len());
                if k < len {
                    Ok(k as i32)
                } else {
                    self.record_read_fault(9);
                    Ok(NONE_HANDLE)
                }
            }
            // 11 / 12 / 15 (x) -> int: the index of a door / patch / building, else -1.
            11 | 12 | 15 => Ok(if a0 >= 0 { a0 } else { -1 }),
            // 14 (sound) -> int: the index of a sound source; none exist.
            14 => Ok(-1),
            // 16 (path) -> int: the index in the patrol-path list, 16-bit; 65535 if absent.
            16 => {
                let len = self.vm.as_ref().map_or(0, |vm| vm.paths.len()) as i32;
                Ok(if a0 >= 0 && a0 < len { a0 } else { 65535 })
            }
            // 17 (k): show dialog page k now in the modal loop (VM-218). The engine has no
            // modal loop: the page is queued like native 202's and the app dismisses it.
            17 => {
                if let Some(vm) = self.vm.as_mut() {
                    let _ = vm.show_text(a0, true);
                }
                Ok(0)
            }
            // 18 / 19 (loc[, f]) -> bool: the scroll request of VM-219: the destination is the
            // converted point. The camera model is the movement specification's; the engine
            // centres its camera on the point and remembers it.
            18 | 19 => match self.location_position(a0) {
                Some((x, y)) => {
                    self.vm_camera_to(x, y);
                    Ok(1)
                }
                None => {
                    self.record_error();
                    Ok(0)
                }
            },
            // 20 (loc) -> bool: the camera jump of VM-219.
            20 => match self.location_position(a0) {
                Some((x, y)) => {
                    self.vm_camera_to(x, y);
                    Ok(1)
                }
                None => {
                    self.record_error();
                    Ok(0)
                }
            },
            // 21 (f) -> bool: the requested zoom; only 0.5 / 1.0 / 2.0 are accepted. The engine
            // draws at one zoom, so the request is remembered and nothing else follows.
            21 => {
                let f = f32::from_bits(a0 as u32);
                if f == 0.5 || f == 1.0 || f == 2.0 {
                    Ok(1)
                } else {
                    self.record_error();
                    Ok(0)
                }
            }
            // 29 (): force the next victory check (VM-242): the flag the scheduler consumes.
            29 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.force_victory = true;
                }
                Ok(0)
            }
            // 26 (k, main): add objective k; 27 (k): objective k accomplished (VM-240).
            26 => {
                let (k, main) = (a0, arg(args, 1));
                if let Some(vm) = self.vm.as_mut() {
                    if let Some(o) = vm.objectives.iter_mut().find(|o| o.index == k) {
                        o.primary = main != 0;
                    } else if vm.objectives.len() < MAX_QUEUE {
                        vm.objectives.push(Objective {
                            index: k,
                            primary: main != 0,
                            done: false,
                        });
                    }
                }
                Ok(0)
            }
            27 => {
                if let Some(vm) = self.vm.as_mut() {
                    match vm.objectives.iter_mut().find(|o| o.index == a0) {
                        Some(o) => o.done = true,
                        None => {
                            vm.counters.objective_done_before_added =
                                vm.counters.objective_done_before_added.saturating_add(1);
                        }
                    }
                }
                Ok(0)
            }
            // 28 (k): the debriefing variant read when the level ends (VM-241).
            28 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.debriefing = Some(a0);
                }
                Ok(0)
            }
            // 30 () -> bool: begin recording (VM-200); a recording already open is an error.
            30 => {
                let Some(vm) = self.vm.as_mut() else {
                    return Ok(0);
                };
                if vm.collecting.is_some() {
                    self.record_error();
                    return Ok(0);
                }
                let provenance = vm.provenance();
                vm.collecting = Some(Recording {
                    elements: Vec::new(),
                    level: 1,
                    last_level: 0,
                    entered: BTreeSet::new(),
                    provenance,
                });
                Ok(1)
            }
            // 31 () -> bool: end the recording and launch it at once (VM-202); an empty
            // recording is an error and is discarded, but the result is 1 either way.
            31 => {
                let Some(vm) = self.vm.as_mut() else {
                    return Ok(0);
                };
                let Some(rec) = vm.collecting.take() else {
                    self.record_error();
                    return Ok(0);
                };
                if rec.elements.is_empty() {
                    self.record_error();
                    return Ok(1);
                }
                let total: usize = vm.sequences.iter().map(|s| s.elements.len()).sum();
                if vm.sequences.len() < MAX_QUEUE
                    && total.saturating_add(rec.elements.len()) <= crate::vm::MAX_SEQUENCE_ELEMENTS
                {
                    vm.sequences.push(crate::vm::Sequence {
                        elements: rec.elements,
                        next: 0,
                        wait: crate::vm::SeqWait::None,
                        tokens: Vec::new(),
                        provenance: rec.provenance,
                    });
                }
                Ok(1)
            }
            // 32 () -> int: the barrier of VM-201; two consecutive barriers count once.
            32 => {
                let Some(vm) = self.vm.as_mut() else {
                    return Ok(0);
                };
                let Some(rec) = vm.collecting.as_mut() else {
                    self.record_error();
                    return Ok(0);
                };
                if rec.last_level != 0 && rec.last_level == rec.level {
                    let Some(next) = rec.level.checked_add(1) else {
                        return Err(Fault::BarrierOverflow);
                    };
                    rec.level = next;
                    if rec.elements.len() < MAX_QUEUE {
                        rec.elements.push(SeqElement::Barrier);
                    }
                }
                Ok(i32::from(rec.level))
            }
            // 58 (x) -> bool: a placeholder that is always 0.
            58 => Ok(0),
            // 74 () -> handle: the current actor (VM-093).
            74 => Ok(self.vm.as_ref().map_or(NONE_HANDLE, |vm| vm.current_actor)),
            // 192 () -> handle: the current scroll (VM-094).
            192 => Ok(self.vm.as_ref().map_or(NONE_HANDLE, |vm| vm.current_scroll)),
            // 76 (x) -> bool: x is an animated map element.
            76 => Ok(i32::from(matches!(
                self.vm.as_ref().and_then(|vm| vm.element(a0)),
                Some(Element::Map(_))
            ))),
            // 77 (x) -> bool: x is of the target family (objects, scrolls, pick-up items).
            77 => Ok(i32::from(matches!(
                self.vm.as_ref().and_then(|vm| vm.element(a0)),
                Some(Element::Object { .. } | Element::Scroll { .. } | Element::Item { .. })
            ))),
            // 78 (x) -> bool: x is of the actor family.
            78 => Ok(i32::from(self.entity_of(a0).is_some())),
            // 79 (x) -> bool: x is a player character.
            79 => {
                Ok(i32::from(self.entity_of(a0).is_some_and(|i| {
                    self.entities[i].kind == EntityKind::Player
                })))
            }
            // 80 (x) -> bool: x is an NPC (a human that is not a player character).
            80 => {
                Ok(i32::from(self.entity_of(a0).is_some_and(|i| {
                    self.entities[i].kind != EntityKind::Player
                })))
            }
            // 81 / 82 (x) -> bool: x is a soldier / a civilian. The engine's non-player actors
            // are all guards; a civilian class does not exist yet.
            81 => {
                Ok(i32::from(self.entity_of(a0).is_some_and(|i| {
                    self.entities[i].kind == EntityKind::Guard
                })))
            }
            82 | 83 | 84 => Ok(0),
            // 85 (x) -> bool: x is null. 86 (a, b) -> bool: the two handles are the same.
            85 => Ok(i32::from(a0 == NONE_HANDLE)),
            86 => Ok(i32::from(a0 == arg(args, 1))),
            // 87 / 88 / 90 (x) -> bool: the human is dead / unconscious / out of action. All
            // three read one state function (`ai::ActorStatus`), so they cannot contradict.
            87 => Ok(match self.entity_of(a0) {
                Some(i) => i32::from(ActorStatus::of(&self.entities[i]).dead),
                None => {
                    self.record_error();
                    0
                }
            }),
            88 => Ok(match self.entity_of(a0) {
                Some(i) => {
                    let s = ActorStatus::of(&self.entities[i]);
                    if s.knocked_out
                        && let Some(vm) = self.vm.as_mut()
                    {
                        vm.assume(Assumption::KnockOut);
                    }
                    i32::from(s.knocked_out)
                }
                None => 0,
            }),
            89 => Ok(0),
            90 => Ok(match self.entity_of(a0) {
                Some(i) => {
                    let s = ActorStatus::of(&self.entities[i]);
                    if s.out_of_action
                        && let Some(vm) = self.vm.as_mut()
                    {
                        vm.counters.out_of_action_true =
                            vm.counters.out_of_action_true.saturating_add(1);
                        if s.knocked_out {
                            vm.assume(Assumption::KnockOut);
                        }
                    }
                    i32::from(s.out_of_action)
                }
                None => 0,
            }),
            // 93 (x) -> int: the facing direction 0..15 of a known element; 94 (x, d): set it.
            93 => Ok(match self.entity_of(a0) {
                Some(i) => self.entities[i].facing256.rem_euclid(256) / FACING_UNITS_PER_DIRECTION,
                None => 0,
            }),
            94 | 133 => {
                if let Some(entity) = self.entity_of(a0) {
                    self.vm_touch_entity(entity);
                    let dir = if id == 133 {
                        let to = self.location_position(arg(args, 1));
                        self.vm_teleport(entity as u32, to);
                        arg(args, 2)
                    } else {
                        arg(args, 1)
                    };
                    self.entities[entity].facing256 =
                        dir.rem_euclid(16) * FACING_UNITS_PER_DIRECTION;
                    Ok(1)
                } else {
                    self.record_error();
                    Ok(0)
                }
            }
            // 95 (x) -> handle: a new point location at x's position; unknown -> null.
            95 => Ok(match self.element_position(a0) {
                Some((x, y)) => location_of_point(x, y),
                None => NONE_HANDLE,
            }),
            // 96 (x, loc) -> bool: a null location takes x off the map; else x is put back and
            // moved to the point.
            96 => {
                if let Some(entity) = self.entity_of(a0) {
                    let to = self.location_position(arg(args, 1));
                    self.vm_teleport(entity as u32, to);
                    Ok(1)
                } else {
                    self.record_error();
                    Ok(0)
                }
            }
            // 97 (x, zone) -> bool: x is inside the zone. One work unit per polygon edge.
            97 => {
                let Some((x, y)) = self.element_position(a0) else {
                    return Ok(0);
                };
                let zone = arg(args, 1);
                let Some(vm) = self.vm.as_mut() else {
                    return Ok(0);
                };
                let Some(poly) = polygon_in(&vm.program, zone) else {
                    return Ok(0);
                };
                if !charge_budget(&mut vm.budget, poly.len() as u64) {
                    vm.counters.budget_aborts = vm.counters.budget_aborts.saturating_add(1);
                    return Ok(0);
                }
                Ok(i32::from(poly.len() >= 3 && point_in_polygon(x, y, poly)))
            }
            // 98 (x, building) -> bool: a null building asks "is x in a building interior",
            // any other one "is x's zone that building". This engine has no interiors, so
            // every actor is outdoors and both answers are 0.
            98 => Ok(0),
            // 101 (x) -> int: the current action id; an actor without one answers 283.
            101 => Ok(match self.entity_of(a0) {
                Some(i) => self.entities[i].action as i32,
                None => {
                    self.record_error();
                    0
                }
            }),
            // 103 (x) -> bool: stop the actor (its movement and order are cancelled).
            103 => {
                if let Some(i) = self.entity_of(a0) {
                    self.vm_touch_entity(i);
                    let e = &mut self.entities[i];
                    e.target = None;
                    e.path.clear();
                    e.clear_pickup();
                    Ok(1)
                } else {
                    self.record_error();
                    Ok(0)
                }
            }
            // 109 (target, msg) / 110 (target, msg, a, b): send the message **now** (VM-120);
            // a null target is the level class, anything but an actor an error.
            109 | 110 => {
                let m = Message {
                    target: a0,
                    id: arg(args, 1),
                    arg: arg(args, 2),
                    arg2: arg(args, 3),
                };
                if m.target != NONE_HANDLE && self.entity_of(m.target).is_none() {
                    self.record_error();
                    return Ok(0);
                }
                self.vm_deliver(m);
                Ok(0)
            }
            // 111 / 159 () -> handle: always null in this build.
            111 | 159 => Ok(NONE_HANDLE),
            // 112 (k) -> bool: select all (31) or none (0); any other code is an error, and the
            // native answers 1 either way.
            112 => {
                match a0 {
                    0 => self.selected = None,
                    31 => {}
                    _ => self.record_error(),
                }
                Ok(1)
            }
            // 113 / 114 (x) -> bool: deactivate / activate an element.
            113 | 114 => {
                self.set_element_active(a0, id == 114);
                Ok(1)
            }
            // 115 (pc, k, b) -> bool / 116 (pc, k) -> bool: the availability of action k
            // (0..5) for a player character; the HUD is not modelled, the flag is stored.
            115 | 116 => {
                let k = arg(args, 1);
                if !(0..6).contains(&k) {
                    self.record_error();
                    return Ok(0);
                }
                let Some(vm) = self.vm.as_mut() else {
                    return Ok(0);
                };
                if id == 115 {
                    let b = arg(args, 2);
                    crate::vm::VmState::set_pair_table(&mut vm.pc_actions, a0, k, b, MAX_QUEUE);
                    Ok(1)
                } else {
                    Ok(crate::vm::VmState::pair_table(&vm.pc_actions, a0, k, 1))
                }
            }
            // 117 (x, prop, v) -> bool / 118 (x, prop) -> int: element properties; the engine
            // keeps them as one hashed table (the per-property meaning is the AI's and the
            // campaign's).
            117 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.set_attribute(a0, arg(args, 1), arg(args, 2));
                }
                Ok(1)
            }
            118 => Ok(self
                .vm
                .as_ref()
                .map_or(-1, |vm| vm.attribute(a0, arg(args, 1)))),
            // 119 / 120 () -> bool: some civilian / soldier of the level is dead.
            119 => Ok(0),
            120 => Ok(i32::from(
                self.entities
                    .iter()
                    .any(|e| e.kind == EntityKind::Guard && !e.alive),
            )),
            // 128 (npc) -> int: 1 if the NPC is hostile. The engine reads its able-to-act
            // state, a policy row.
            128 => Ok(match self.entity_of(a0) {
                Some(i) => {
                    let s = ActorStatus::of(&self.entities[i]);
                    if !s.can_act
                        && s.knocked_out
                        && let Some(vm) = self.vm.as_mut()
                    {
                        vm.assume(Assumption::KnockOut);
                    }
                    i32::from(s.can_act)
                }
                None => 1,
            }),
            // 127 (a, b) -> bool: obsolete, always an error and 0.
            127 => {
                self.record_error();
                Ok(0)
            }
            // 129 (npc, a, b) -> bool: no effect; 1 for an NPC, an error otherwise.
            129 => Ok(i32::from(self.entity_of(a0).is_some())),
            // 130 (npc, target, b) / 131 (npc, loc, b): the operation is empty in this build;
            // only the arguments are validated.
            130 | 131 => {
                if self.entity_of(a0).is_none() {
                    self.record_error();
                }
                Ok(0)
            }
            // 132 (npc, path): assign the patrol path (AI 3.4).
            132 => {
                let path = arg(args, 1);
                let program = self.vm.as_ref().and_then(|vm| {
                    usize::try_from(path)
                        .ok()
                        .and_then(|p| vm.paths.get(p).copied())
                });
                if let Some(i) = self.entity_of(a0) {
                    self.vm_touch_entity(i);
                    let e = &mut self.entities[i];
                    e.program = program.flatten();
                    e.pc = 0;
                    e.target = None;
                    e.path.clear();
                    e.wait_ticks = 0;
                } else {
                    self.record_error();
                }
                Ok(0)
            }
            // 134 (x, b) / 135 (x): lock / unlock the AI. Locking halts the actor's own walk
            // (the engine's reading); a player character is an error and is not touched.
            134 | 135 => {
                match self.entity_of(a0) {
                    Some(i) if self.entities[i].kind != EntityKind::Player => {
                        self.vm_touch_entity(i);
                        let e = &mut self.entities[i];
                        e.ai_locked = id == 134;
                        if e.ai_locked {
                            e.target = None;
                            e.path.clear();
                        }
                    }
                    _ => self.record_error(),
                }
                Ok(0)
            }
            // 137 (loc, k): make a noise at the point; an unknown k is reported and still used.
            137 => {
                if self.location_position(a0).is_none() {
                    self.record_error();
                }
                Ok(0)
            }
            // 140 (npc, k): the walking style of the NPC's own walks.
            140 => {
                if let Some(i) = self.entity_of(a0) {
                    self.vm_touch_entity(i);
                    self.entities[i].npc_gait = if arg(args, 1) == 0 {
                        Gait::Walk
                    } else {
                        Gait::Run
                    };
                } else {
                    self.record_error();
                }
                Ok(0)
            }
            // 144 (patch) -> bool: the patch's active flag; a null patch is the unchecked
            // read of 8.1 (0, the callback continues).
            144 => {
                if a0 == NONE_HANDLE {
                    self.record_read_fault(144);
                    return Ok(0);
                }
                Ok(i32::from(
                    self.vm.as_ref().is_some_and(|vm| vm.patches.contains(&a0)),
                ))
            }
            // 145 / 146 (patch) -> bool: activate / deactivate the patch.
            145 | 146 => {
                if let Some(vm) = self.vm.as_mut() {
                    if id == 145 {
                        if vm.patches.len() < MAX_QUEUE {
                            vm.patches.insert(a0);
                        }
                    } else {
                        vm.patches.remove(&a0);
                    }
                }
                Ok(1)
            }
            // 153 / 154 (x, zone) -> bool: fire `ExitZone` / `EnterZone` for an actor that is
            // already a member of the zone (VM-120b).
            153 | 154 => {
                let zone = arg(args, 1);
                let Some(entity) = self.entity_of(a0) else {
                    self.record_error();
                    return Ok(0);
                };
                let member = self.vm.as_ref().and_then(|vm| {
                    let class = vm
                        .program
                        .classes
                        .iter()
                        .position(|c| c.zone.is_some_and(|z| z as i32 == zone))?;
                    vm.zone_presence
                        .contains(&(class as u32, entity as u32))
                        .then_some(class as u32)
                });
                let Some(class) = member else {
                    self.record_error();
                    return Ok(0);
                };
                let name = if id == 154 {
                    crate::vm::callbacks::ENTER_ZONE
                } else {
                    crate::vm::callbacks::EXIT_ZONE
                };
                self.vm_with_current_actor(a0, |w| {
                    w.vm_callback(class, name, &[a0]);
                });
                Ok(1)
            }
            // 155 (x): no effect.
            155 => Ok(0),
            // 158 (zone) -> handle: the first actor inside the zone, null if none.
            158 => {
                let Some(vm) = self.vm.as_ref() else {
                    return Ok(NONE_HANDLE);
                };
                let Some(poly) = polygon_in(&vm.program, a0) else {
                    self.record_error();
                    return Ok(NONE_HANDLE);
                };
                let poly: Vec<(i32, i32)> = poly.to_vec();
                let found = self.entities.iter().position(|e| {
                    e.alive && e.active && point_in_polygon(e.x.round(), e.y.round(), &poly)
                });
                Ok(match found {
                    Some(i) => self
                        .vm
                        .as_ref()
                        .map_or(NONE_HANDLE, |vm| vm.program.element_of_entity(i as u32)),
                    None => NONE_HANDLE,
                })
            }
            // 160 (a, b) -> int: the distance between two points, truncated; a non-point is an
            // error and 0. The differences are formed in `i64` and squared in `u128`, so any
            // pair of positions gives the same answer in debug and release.
            160 => match (
                self.location_position(a0),
                self.location_position(arg(args, 1)),
            ) {
                (Some(a), Some(b)) => {
                    let dx = u128::from((i64::from(a.0) - i64::from(b.0)).unsigned_abs());
                    let dy = u128::from((i64::from(a.1) - i64::from(b.1)).unsigned_abs());
                    let d = (dx * dx + dy * dy).isqrt();
                    Ok(i32::try_from(d).unwrap_or(i32::MAX))
                }
                _ => {
                    self.record_error();
                    Ok(0)
                }
            },
            // 161 (n) -> int: `rand() mod n`; n = 0 is an arithmetic trap (8.2: the engine
            // draws from its own `script` stream).
            161 => {
                if a0 == 0 {
                    return Err(Fault::Trap);
                }
                let n = a0.unsigned_abs();
                Ok(self.vm.as_mut().map_or(0, |vm| vm.rng.below(n) as i32))
            }
            // 162 (x): debug output of x as a decimal number; no game effect.
            162 => Ok(0),
            // 168 (i) -> handle: the element of team entry i; the index is compared unsigned
            // with the list size, so a negative one takes the same path: an exception (X).
            168 => Err(Fault::Range(168)),
            // 175 (id, loc): place the player character with campaign id `id` at the point.
            175 => {
                let to = self.location_position(arg(args, 1));
                if let Some(entity) = self.entity_of(self.player_element(a0)).map(|i| i as u32) {
                    self.vm_teleport(entity, to);
                }
                Ok(0)
            }
            // 178 (banner): capture a banner (it must be active): deactivated, the campaign's
            // banner counter raised. 179 (banner): lose one again. 223 (banner) -> bool: the
            // banner is inactive (= captured). 234 () -> bool: the counter reached the
            // requirement (the requirement is the campaign specification's; 0 here).
            178 | 179 => {
                let active = self.vm.as_ref().is_some_and(|vm| vm.element_active(a0));
                if (id == 178) != active {
                    self.record_error();
                    return Ok(0);
                }
                self.set_element_active(a0, id == 179);
                if let Some(vm) = self.vm.as_mut() {
                    vm.banner_count = if id == 178 {
                        vm.banner_count.saturating_add(1)
                    } else {
                        vm.banner_count.saturating_sub(1)
                    };
                }
                Ok(0)
            }
            223 => Ok(i32::from(
                self.vm.as_ref().is_some_and(|vm| !vm.element_active(a0)),
            )),
            234 => Ok(0),
            // 182 - 185 (door) -> bool: the four door lock bytes (NAV-172); null unchecked.
            // 186 - 189 (door, b): set them; 186 / 188 / 189 with b = 0 also open the door.
            182..=185 => {
                if a0 == NONE_HANDLE {
                    self.record_read_fault(id);
                    return Ok(0);
                }
                Ok(self.vm.as_ref().map_or(0, |vm| {
                    crate::vm::VmState::pair_table(&vm.door_bytes, a0, id as i32 - 182, 0)
                }))
            }
            186..=189 => {
                if a0 == NONE_HANDLE {
                    return Err(Fault::UncheckedAccess(id));
                }
                let b = arg(args, 1);
                if let Some(vm) = self.vm.as_mut() {
                    crate::vm::VmState::set_pair_table(
                        &mut vm.door_bytes,
                        a0,
                        id as i32 - 186,
                        b,
                        MAX_QUEUE * 16,
                    );
                }
                Ok(0)
            }
            // 191 (b, door): the door leaf's click-target flag; a null door is an error.
            191 => {
                let door = arg(args, 1);
                if door == NONE_HANDLE {
                    self.record_error();
                    return Ok(0);
                }
                if let Some(vm) = self.vm.as_mut() {
                    crate::vm::VmState::set_pair_table(
                        &mut vm.door_bytes,
                        door,
                        4,
                        a0,
                        MAX_QUEUE * 16,
                    );
                }
                Ok(0)
            }
            // 193 (scroll) -> int: the scroll status 0..3; 194 (scroll, s): set it (1 and 3
            // visible, 0 and 2 hidden).
            193 => {
                if self.vm.as_ref().and_then(|vm| vm.scroll(a0)).is_none() {
                    self.record_error();
                    return Ok(0);
                }
                Ok(self
                    .vm
                    .as_ref()
                    .map_or(0, |vm| vm.states.get(&a0).copied().unwrap_or(0)))
            }
            194 => {
                let s = arg(args, 1);
                if !(0..4).contains(&s) || self.vm.as_ref().and_then(|vm| vm.scroll(a0)).is_none() {
                    self.record_error();
                    return Ok(0);
                }
                if let Some(vm) = self.vm.as_mut()
                    && (vm.states.contains_key(&a0) || vm.states.len() < MAX_QUEUE * 16)
                {
                    vm.states.insert(a0, s);
                }
                self.set_element_active(a0, s == 1 || s == 3);
                Ok(0)
            }
            // 195 (k) -> int / 196 (k, v): the twenty script-visible campaign values, k in 0..19.
            195 | 196 => {
                if !(0..CAMPAIGN_VALUES as i32).contains(&a0) {
                    self.record_error();
                    return Ok(0);
                }
                let Some(vm) = self.vm.as_mut() else {
                    return Ok(0);
                };
                if id == 195 {
                    Ok(vm.campaign_values[a0 as usize])
                } else {
                    vm.campaign_values[a0 as usize] = arg(args, 1);
                    Ok(0)
                }
            }
            // 197 (npc, k) -> int / 198 (npc, k, v): ten custom values per NPC.
            197 | 198 => {
                let k = arg(args, 1);
                if !(0..NPC_VALUES as i32).contains(&k) || self.entity_of(a0).is_none() {
                    self.record_error();
                    return Ok(if id == 197 { -1 } else { 0 });
                }
                let Some(vm) = self.vm.as_mut() else {
                    return Ok(0);
                };
                if id == 197 {
                    Ok(crate::vm::VmState::pair_table(&vm.npc_values, a0, k, 0))
                } else {
                    crate::vm::VmState::set_pair_table(
                        &mut vm.npc_values,
                        a0,
                        k,
                        arg(args, 2),
                        MAX_QUEUE * 16,
                    );
                    Ok(0)
                }
            }
            // 201 (id) -> handle: the player-character slot with campaign id `id`.
            201 => Ok(self.player_element(a0)),
            // 202 (k): show the popup page k now (VM-218: a modal loop in the original; the
            // engine queues it and the app dismisses it).
            202 => {
                if let Some(vm) = self.vm.as_mut() {
                    let _ = vm.show_text(a0, false);
                }
                Ok(0)
            }
            // 204 (zone) -> int: the number of actors inside the zone; 205 (zone, i) -> handle:
            // the i-th of them. One unit per entity looked at and one per edge tested.
            204 | 205 => {
                let Some(vm) = self.vm.as_mut() else {
                    return Ok(0);
                };
                let Some(poly) = polygon_in(&vm.program, a0) else {
                    self.record_error();
                    return Ok(if id == 204 { 0 } else { NONE_HANDLE });
                };
                if poly.len() < 3 {
                    return Ok(if id == 204 { 0 } else { NONE_HANDLE });
                }
                let edges = poly.len() as u64;
                let poly: Vec<(i32, i32)> = poly.to_vec();
                let want = arg(args, 1);
                let mut count = 0;
                let mut found = None;
                for (i, e) in self.entities.iter().enumerate() {
                    let live = e.alive && e.active;
                    let cost = if live { 1 + edges } else { 1 };
                    let Some(vm) = self.vm.as_mut() else {
                        return Ok(0);
                    };
                    if !charge_budget(&mut vm.budget, cost) {
                        vm.counters.budget_aborts = vm.counters.budget_aborts.saturating_add(1);
                        return Ok(0);
                    }
                    if live && point_in_polygon(e.x.round(), e.y.round(), &poly) {
                        if count == want {
                            found = Some(i);
                        }
                        count += 1;
                    }
                }
                if id == 204 {
                    return Ok(count);
                }
                Ok(match found {
                    Some(i) => self
                        .vm
                        .as_ref()
                        .map_or(NONE_HANDLE, |vm| vm.program.element_of_entity(i as u32)),
                    None => {
                        self.record_error();
                        NONE_HANDLE
                    }
                })
            }
            // 206 / 207 / 208 (a, b) -> int: full-width bitwise and / or / exclusive or.
            206 => Ok(a0 & arg(args, 1)),
            207 => Ok(a0 | arg(args, 1)),
            208 => Ok(a0 ^ arg(args, 1)),
            // 211 () -> handle: the first player character flagged as the leader, or null.
            211 => Ok(self.player_element(0)),
            // 213 (a, b, t) -> handle: a new point at a + (b - a) * t.
            213 => match (
                self.location_position(a0),
                self.location_position(arg(args, 1)),
            ) {
                (Some(a), Some(b)) => {
                    let t = f64::from(f32::from_bits(arg(args, 2) as u32));
                    let lerp = |p: i32, q: i32| {
                        let v = f64::from(p) + (f64::from(q) - f64::from(p)) * t;
                        if v.is_finite() {
                            v.trunc().clamp(0.0, 32767.0) as i32
                        } else {
                            0
                        }
                    };
                    Ok(location_of_point(lerp(a.0, b.0), lerp(a.1, b.1)))
                }
                _ => {
                    self.record_error();
                    Ok(NONE_HANDLE)
                }
            },
            // 216 () -> int: the number of player characters; 217 (i) -> handle: the i-th.
            216 => Ok(self
                .entities
                .iter()
                .filter(|e| e.kind == EntityKind::Player)
                .count() as i32),
            217 => Ok(self.player_element(a0.rem_euclid(256))),
            // 229 (human): confiscate a non-player human's money into the campaign counter.
            229 => {
                if self.entity_of(a0).is_none() {
                    self.record_error();
                }
                Ok(0)
            }
            // 230 (zone) -> bool: every player character is inside the zone; 246 (zone): every
            // living one.
            230 | 246 => {
                let Some(vm) = self.vm.as_ref() else {
                    return Ok(0);
                };
                let Some(poly) = polygon_in(&vm.program, a0) else {
                    return Ok(0);
                };
                let poly: Vec<(i32, i32)> = poly.to_vec();
                Ok(i32::from(
                    poly.len() >= 3
                        && self
                            .entities
                            .iter()
                            .filter(|e| e.kind == EntityKind::Player && (id == 230 || e.alive))
                            .all(|e| point_in_polygon(e.x.round(), e.y.round(), &poly)),
                ))
            }
            // 231 (zone) -> bool: no active hostile soldier in the zone can still fight.
            231 => {
                let Some(vm) = self.vm.as_ref() else {
                    return Ok(0);
                };
                let Some(poly) = polygon_in(&vm.program, a0) else {
                    self.record_error();
                    return Ok(0);
                };
                let poly: Vec<(i32, i32)> = poly.to_vec();
                Ok(i32::from(!self.entities.iter().any(|e| {
                    e.kind == EntityKind::Guard
                        && ActorStatus::of(e).can_act
                        && point_in_polygon(e.x.round(), e.y.round(), &poly)
                })))
            }
            // 235 (item) -> bool: the bonus item's picked-up flag; anything else is an error.
            235 => {
                if !matches!(
                    self.vm.as_ref().and_then(|vm| vm.element(a0)),
                    Some(Element::Item { .. })
                ) {
                    self.record_error();
                    return Ok(0);
                }
                Ok(i32::from(
                    self.vm
                        .as_ref()
                        .is_some_and(|vm| vm.taken_items.contains(&a0)),
                ))
            }
            // 236 () -> int / 237 (v): the campaign money (counter 1).
            236 => Ok(self.vm.as_ref().map_or(-1, |vm| vm.money)),
            237 => {
                if let Some(vm) = self.vm.as_mut() {
                    vm.money = a0;
                }
                Ok(0)
            }
            // 240 (x) -> bool: the active flag of a known element.
            240 => Ok(match self.entity_of(a0) {
                Some(i) => i32::from(ActorStatus::of(&self.entities[i]).present),
                None => i32::from(self.vm.as_ref().is_some_and(|vm| vm.element_active(a0))),
            }),
            // 245 () -> int: the number of player characters that are not dead.
            245 => Ok(self
                .entities
                .iter()
                .filter(|e| e.kind == EntityKind::Player && e.alive)
                .count() as i32),
            // 248 (pc) -> bool: the player character is selected (a null handle asks whether
            // anything is selected); a non-player character is an error answering 1.
            248 => {
                if a0 == NONE_HANDLE {
                    return Ok(i32::from(self.selected.is_some()));
                }
                let Some(i) = self.entity_of(a0) else {
                    self.record_error();
                    return Ok(1);
                };
                Ok(i32::from(self.selected == Some(self.entities[i].id)))
            }
            // 249 () -> int: the number of selected player characters; 250 (i) -> handle: the
            // i-th of them.
            249 => Ok(i32::from(self.selected.is_some())),
            250 => {
                let selected = self.selected;
                let Some(sel) = selected else {
                    self.record_error();
                    return Ok(NONE_HANDLE);
                };
                if a0 != 0 {
                    self.record_error();
                    return Ok(NONE_HANDLE);
                }
                let idx = self.entities.iter().position(|e| e.id == sel);
                Ok(match idx {
                    Some(i) => self
                        .vm
                        .as_ref()
                        .map_or(NONE_HANDLE, |vm| vm.program.element_of_entity(i as u32)),
                    None => NONE_HANDLE,
                })
            }
            // 252 (pc): make the player character crouch.
            252 => {
                match self.entity_of(a0) {
                    Some(i) if self.entities[i].kind == EntityKind::Player => {
                        self.vm_touch_entity(i);
                        self.entities[i].posture = crate::world::Posture::Crouched;
                    }
                    _ => self.record_error(),
                }
                Ok(0)
            }
            // 257 (pc, b): select or deselect a player character (a null handle acts on all).
            257 => {
                let b = arg(args, 1);
                if a0 == NONE_HANDLE {
                    if b == 0 {
                        self.selected = None;
                    }
                    return Ok(0);
                }
                match self.entity_of(a0) {
                    Some(i) if self.entities[i].kind == EntityKind::Player => {
                        let eid = self.entities[i].id;
                        if b != 0 {
                            self.selected = Some(eid);
                        } else if self.selected == Some(eid) {
                            self.selected = None;
                        }
                    }
                    _ => self.record_error(),
                }
                Ok(0)
            }
            // Every remaining id is a stub of [`Kind::Stub`]: its call was counted and recorded
            // by `native_dispatch`; it answers the row's neutral value.
            other => Ok(stub_value(other)),
        }
    }

    /// Count an error of the failure class E (reported, the row's failure value returned, the
    /// script continues).
    fn record_error(&mut self) {
        if let Some(vm) = self.vm.as_mut() {
            vm.counters.native_errors = vm.counters.native_errors.saturating_add(1);
        }
    }

    /// Record an unchecked read (class U, a read): the value is null / 0 and the callback
    /// continues (`spec-script-vm.md` 8.1).
    fn record_read_fault(&mut self, id: u32) {
        if let Some(vm) = self.vm.as_mut() {
            vm.counters.faults = vm.counters.faults.saturating_add(1);
            vm.set_fault(Fault::UncheckedAccess(id));
        }
    }
}

/// The neutral value a [`Kind::Stub`] answers: the row's failure value where a 0 would send a
/// script down a path the original never takes (`STUB_POLICY_VALUES`), 0 otherwise.
fn stub_value(id: u32) -> i32 {
    STUB_POLICY_VALUES
        .iter()
        .find(|(k, _)| *k == id)
        .map_or(0, |(_, v)| *v)
}

/// Stub natives whose neutral value is not 0. 253 / 255 (a lost / present character with a
/// skill) answer 1: with 0 every `CheckVictoryCondition` that tests them loses at tick 1. 174
/// (the mission team's size limit) answers 5, the limit the specification gives when no mission
/// is selected, so a deployment zone admits a character. 141 (a soldier's rank) and 124 (an
/// alert state) answer 0, which is the lowest of each range.
pub const STUB_POLICY_VALUES: &[(u32, i32)] = &[
    (22, 1),
    (147, 1),
    (148, 1),
    (149, 1),
    (150, 1),
    (151, 1),
    (174, 5),
    (253, 1),
    (255, 1),
];

impl World {
    /// The sequence element a recording native builds (VM-203 / VM-204). What the element
    /// *does* when the sequence runs it is the engine's own reading until section 3.7 is
    /// cleared: the categories the engine models keep their behaviour, every other one is a
    /// recorded no-op.
    #[allow(clippy::match_same_arms)]
    fn sequence_element(&self, id: u32, args: &[i32]) -> Option<SeqElement> {
        Some(match id {
            // 203 (k): a page, modal in the original, a queued text here.
            203 => SeqElement::Text(arg(args, 0)),
            // 56 (n): a timer of n logic frames (VM-221, ADR-0010: one to one).
            56 => SeqElement::Wait(arg(args, 0).max(0) as u32),
            // 33 / 34 / 42: camera scroll and jump.
            33 | 34 | 42 => SeqElement::Camera(arg(args, 0)),
            // 43 (target, msg) / 44 (target, msg, a, b): a message element (VM-121).
            43 | 44 => SeqElement::Message(Message {
                target: arg(args, 0),
                id: arg(args, 1),
                arg: arg(args, 2),
                arg2: arg(args, 3),
            }),
            // 45 / 48 / 64 / 212: a walk of an actor to a point.
            45 | 48 | 64 | 212 => {
                let (entity, x, y) = self.walk_target(arg(args, 0), arg(args, 1))?;
                SeqElement::Walk { entity, x, y }
            }
            // 46 / 47: "enter the game" - the placement happens at once (VM-203), the walk is
            // the element.
            46 | 47 => {
                let (entity, x, y) = self.walk_target(arg(args, 0), arg(args, 1))?;
                SeqElement::Walk { entity, x, y }
            }
            // 49 / 50 / 51 (x, anim): animations; 52 / 53 (actor): the AI lock.
            49..=53 => SeqElement::Animation {
                id,
                actor: arg(args, 0),
                anim: arg(args, 1),
            },
            other => SeqElement::Stub { id: other },
        })
    }

    /// Entity index of an element handle, if it is a modelled actor.
    pub(crate) fn entity_of(&self, handle: i32) -> Option<usize> {
        match self.vm.as_ref()?.element(handle)? {
            Element::Actor(i) if (i as usize) < self.entities.len() => Some(i as usize),
            _ => None,
        }
    }

    /// Map position of an element (actors, objects, scrolls, items, polygons).
    fn element_position(&self, handle: i32) -> Option<(i32, i32)> {
        let vm = self.vm.as_ref()?;
        match vm.element(handle)? {
            Element::Actor(i) => {
                let e = self.entities.get(i as usize)?;
                Some((e.x.round(), e.y.round()))
            }
            Element::Object { x, y } | Element::Scroll { x, y } | Element::Item { x, y, .. } => {
                Some((x, y))
            }
            Element::Polygon(l) => Some(vm.program.locations.get(l as usize)?.position()),
            Element::Map(_) | Element::Unmodelled(_) => None,
        }
    }

    /// Map position of a location value (table index or packed point).
    pub(crate) fn location_position(&self, value: i32) -> Option<(i32, i32)> {
        if let Some(p) = unpack_point(value) {
            return Some(p);
        }
        if value < 0 {
            return None;
        }
        Some(
            self.vm
                .as_ref()?
                .program
                .locations
                .get(value as usize)?
                .position(),
        )
    }

    /// Element handle of the `i`-th player character in entity order.
    fn player_element(&self, i: i32) -> i32 {
        let Some(vm) = self.vm.as_ref() else {
            return NONE_HANDLE;
        };
        if i < 0 {
            return NONE_HANDLE;
        }
        self.entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == EntityKind::Player)
            .nth(i as usize)
            .map_or(NONE_HANDLE, |(idx, _)| {
                vm.program.element_of_entity(idx as u32)
            })
    }

    fn walk_target(&self, actor: i32, location: i32) -> Option<(u32, i32, i32)> {
        let entity = self.entity_of(actor)? as u32;
        let (x, y) = self.location_position(location)?;
        Some((entity, x, y))
    }

    /// Natives 113 / 114 / 194: entities get their `active` flag (a deactivated entity loses
    /// its movement order and its selection); other elements are remembered.
    fn set_element_active(&mut self, handle: i32, active: bool) {
        match self.entity_of(handle) {
            Some(i) => {
                self.vm_touch_entity(i);
                let e = &mut self.entities[i];
                e.active = active;
                if !active {
                    e.target = None;
                    e.path.clear();
                    e.clear_pickup();
                    if self.selected == Some(e.id) {
                        self.selected = None;
                    }
                }
            }
            None => {
                if let Some(vm) = self.vm.as_mut()
                    && handle >= 0
                {
                    if active {
                        vm.inactive_elements.remove(&handle);
                    } else if vm.inactive_elements.len() < MAX_QUEUE * 16 {
                        vm.inactive_elements.insert(handle);
                    }
                }
            }
        }
    }

    /// Natives 18 / 19 / 20 and the camera elements: centre the camera on the point and record
    /// it for the app. The camera contract is the movement specification's (ANIM-320 - 333),
    /// which is not cleared: every caller is a `Policy` row.
    pub(crate) fn vm_camera(&mut self, location: i32) {
        if let Some((x, y)) = self.location_position(location) {
            self.vm_camera_to(x, y);
        }
    }

    /// [`World::vm_camera`] on a map point.
    pub(crate) fn vm_camera_to(&mut self, x: i32, y: i32) {
        self.center_camera_on(x, y);
        if let Some(vm) = self.vm.as_mut() {
            vm.camera_target = Some((x, y));
        }
    }

    /// Run `f` with the current actor (native 74) set to `actor`, restoring the enclosing value
    /// afterwards (VM-093: the handle has dynamic scope).
    pub(crate) fn vm_with_current_actor(&mut self, actor: i32, f: impl FnOnce(&mut World)) {
        let previous = self.vm.as_ref().map(|vm| vm.current_actor);
        if let Some(vm) = self.vm.as_mut() {
            vm.current_actor = actor;
        }
        f(self);
        if let (Some(vm), Some(p)) = (self.vm.as_mut(), previous) {
            vm.current_actor = p;
        }
    }

    /// Run `f` with the current scroll (native 192) set to `scroll` (VM-094: set for the
    /// duration of a scroll class's callback and cleared afterwards). Setting it while it is
    /// already set is the fatal routine of VM-089 class F, which 8.1 turns into a recorded
    /// [`Fault::ScrollOverlap`]: `f` is **not** run, nothing terminates, and the enclosing
    /// scroll stays current. `false` = the overlap refused the call.
    pub(crate) fn vm_with_current_scroll(
        &mut self,
        scroll: i32,
        f: impl FnOnce(&mut World),
    ) -> bool {
        let previous = self.vm.as_ref().map(|vm| vm.current_scroll);
        if previous.is_some_and(|p| p != NONE_HANDLE) {
            if let Some(vm) = self.vm.as_mut() {
                vm.counters.faults = vm.counters.faults.saturating_add(1);
                vm.set_fault(Fault::ScrollOverlap(scroll));
            }
            return false;
        }
        if let Some(vm) = self.vm.as_mut() {
            vm.current_scroll = scroll;
        }
        f(self);
        if let (Some(vm), Some(p)) = (self.vm.as_mut(), previous) {
            vm.current_scroll = p;
        }
        true
    }

    /// Walk order for an entity through the pathfinding, charged to the VM's work budget: when
    /// the budget runs out the order is dropped (the entity stands, a barrier token completes)
    /// and `budget_aborts` counts it.
    pub(crate) fn vm_walk(&mut self, entity: u32, x: i32, y: i32) {
        let i = entity as usize;
        if i >= self.entities.len() || !self.entities[i].alive || !self.entities[i].active {
            return;
        }
        self.vm_touch_entity(i);
        let mut budget = self.vm.as_ref().map_or(0, |vm| vm.budget);
        let planned = self.plan_path_with(i, (Fixed::from_int(x), Fixed::from_int(y)), &mut budget);
        if let Some(vm) = self.vm.as_mut() {
            vm.budget = budget;
            if planned.is_err() {
                vm.counters.budget_aborts = vm.counters.budget_aborts.saturating_add(1);
            }
        }
    }

    /// Native 96: teleport; `None` puts the entity off the map (deactivated).
    pub(crate) fn vm_teleport(&mut self, entity: u32, to: Option<(i32, i32)>) {
        self.vm_touch_entity(entity as usize);
        let Some(e) = self.entities.get_mut(entity as usize) else {
            return;
        };
        e.target = None;
        e.path.clear();
        match to {
            Some((x, y)) => {
                let w = self.map_size.0 as i32;
                let h = self.map_size.1 as i32;
                e.x = Fixed::from_int(x.clamp(0, w));
                e.y = Fixed::from_int(y.clamp(0, h));
            }
            None => e.active = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The call table has one row per id 0..=264, its arity-0 rows are the ones VM-087 names
    /// (native 192 excepted: its row takes no argument although the claim's list omits it, an
    /// implementer question on the specification), and every bool-converted argument of VM-086
    /// lies inside its row's arity.
    #[test]
    fn the_call_table_follows_the_specification() {
        assert_eq!(NATIVES.len(), NATIVE_TABLE_SIZE);
        assert_eq!(NATIVE_KINDS.len(), NATIVE_TABLE_SIZE);
        let arity0: Vec<u32> = (0..NATIVE_TABLE_SIZE as u32)
            .filter(|&i| NATIVES[i as usize].arity == 0)
            .collect();
        let claimed = [
            23u32, 29, 30, 31, 32, 40, 54, 55, 74, 75, 106, 111, 119, 120, 121, 122, 147, 148, 159,
            163, 167, 170, 171, 172, 173, 174, 192, 211, 216, 234, 236, 238, 239, 245, 249, 251,
            261,
        ];
        assert_eq!(arity0, claimed);
        for (id, row) in NATIVES.iter().enumerate() {
            assert!(
                row.bool_args < (1u16 << row.arity.max(1)),
                "native {id} converts an argument it does not take"
            );
            assert!(
                row.arity as usize <= crate::vm::NATIVE_ARG_CELLS,
                "native {id}"
            );
        }
        // The two lists the specification's 4.2 and 4.3 name are exactly the kinds they map to.
        for id in 0..NATIVE_TABLE_SIZE as u32 {
            let kind = native_kind(id).unwrap();
            assert_eq!(
                kind == Kind::Unknown,
                EXCLUDED.contains(&id),
                "native {id} kind {kind:?}"
            );
            assert_eq!(
                kind == Kind::Unresolved,
                UNRESOLVED_EFFECT.contains(&id),
                "native {id} kind {kind:?}"
            );
        }
        // Every stub value names a stub whose row leaves a value.
        for (id, _) in STUB_POLICY_VALUES {
            assert_eq!(native_kind(*id), Some(Kind::Stub), "{id}");
            assert_ne!(native_row(*id).unwrap().result, ResultKind::Void, "{id}");
        }
        // The recording natives are rows that answer a bool or nothing, never a handle.
        for id in RECORDING_NATIVES {
            assert!(
                matches!(
                    native_row(*id).unwrap().result,
                    ResultKind::Bool | ResultKind::Void | ResultKind::Int
                ),
                "{id}"
            );
        }
    }
}
