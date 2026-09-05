//! NPC perception, alert states and the knock-out: the stealth layer of
//! `docs/original/stealth-and-combat.md` (section 6, items 1-3 and the knock-out of 4), as far as
//! the first mission needs it. Everything here is fixed point or integer, part of [`Entity`] (and
//! so of the snapshot, of `validate` and of the `actors` hash) and stepped by [`World::ai_tick`]
//! from `World::simulate`, before the waypoint programs run.
//!
//! Every constant below is a **hypothesis**: the spec gives the animation ids, their tick counts
//! and the punch's 30-35 px reach from the data, but no sight range, cone angle, noise radius or
//! timer was measured (its section 7 lists the oracle captures that would settle them). The
//! values are chosen to make the first mission playable and are pinned by tests so that a
//! correction is a deliberate ruleset bump.
//!
//! Not modelled (documented gaps): occluders and walls do not block sight; civilians neither
//! perceive nor raise the alarm; walking makes no noise; soldiers do not fight, shoot, revive
//! comrades or report to the script through `FilterAIEvent`; a knock-out never fails (the
//! manual's chance is not modelled beyond the resistance threshold); knocked-down bodies do
//! not move by the animation's displacement.
//!
//! Work. One budget per tick ([`AI_WORK_PER_TICK`]) pays for everything the layer does:
//! perception charges one unit per entity of the pre-index pass (the perceivable player
//! characters are collected once per tick), one per entity the scan inspects and one per
//! (soldier, player character) pair tested; every path search the layer issues (the alert run,
//! the return, the attack approach) draws from the same budget, capped per search at the
//! per-order budget `world::ORDER_SEARCH_WORK`. The scan walks the entities from
//! `World::ai_cursor` (round robin: when the budget runs out mid-scan the cursor stays on the
//! entity not finished and the next tick resumes there; a completed scan resets it to 0). The
//! cursor is authoritative (snapshot, `validate`, the `world` hash); the budget itself is
//! granted afresh every tick and never stored.

use serde::{Deserialize, Serialize};

use crate::anim::{AnimSet, direction_of};
use crate::fixed::Fixed;
use crate::vm::{Assumption, charge_budget};
use crate::world::{Entity, EntityKind, Gait, ORDER_SEARCH_WORK, Posture, Team, World, facing_of};

/// Behaviour state of a human (`observe` reports it as `ai_state`; `docs/original/stealth-and-combat.md`
/// 2.4 and 3.1). Player characters use `Patrol` (= normal) and `Punching`; soldiers cycle through
/// the alert states; every human can be knocked out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiState {
    /// Normal: idle, following the waypoint program or the player's orders (actions 0 / 6 / 7).
    #[default]
    Patrol,
    /// Noticed something: the question-mark reaction (action 141), timed.
    Noticed,
    /// Raises the alarm: the exclamation-mark reaction (action 142), timed.
    Alarm,
    /// Alerted: walks / runs to the last seen position with the weapon ready (140 / 143 / 151)
    /// until the alert timeout runs out without a new sighting.
    Alerted,
    /// Returns to where the alert (or the knock-out) took it from (143), then patrols again.
    Returning,
    /// Delivers the knock-out blow (action 123), timed; player characters only.
    Punching,
    /// Knocked down (action 41 forward, 44 backward), timed.
    KnockedDown,
    /// Lying knocked out (47 / 48) until the knock-out timer runs out.
    Lying,
    /// Gets up (action 49), timed.
    GettingUp,
    /// Dead: lies for good (no damage model kills anyone yet; the state exists for the script
    /// predicates).
    Dead,
}

impl AiState {
    /// Stable tag for canonical encodings (never derived from declaration order).
    #[must_use]
    pub fn tag(self) -> u8 {
        match self {
            AiState::Patrol => 1,
            AiState::Noticed => 2,
            AiState::Alarm => 3,
            AiState::Alerted => 4,
            AiState::Returning => 5,
            AiState::Punching => 6,
            AiState::KnockedDown => 7,
            AiState::Lying => 8,
            AiState::GettingUp => 9,
            AiState::Dead => 10,
        }
    }

    /// Knocked out (down or lying) or dead: what script native 90 reports as "out of action"
    /// (a soldier getting up is back in action; hypothesis).
    #[must_use]
    pub fn out_of_action(self) -> bool {
        matches!(self, AiState::KnockedDown | AiState::Lying | AiState::Dead)
    }

    /// On its feet: can perceive, walk, be ordered, be knocked out (script native 128, "able
    /// to act", also requires `alive` and `active`).
    #[must_use]
    pub fn standing(self) -> bool {
        !matches!(
            self,
            AiState::KnockedDown | AiState::Lying | AiState::GettingUp | AiState::Dead
        )
    }

    /// One of the alert states a soldier's perception drives.
    #[must_use]
    pub fn alert(self) -> bool {
        matches!(
            self,
            AiState::Noticed | AiState::Alarm | AiState::Alerted | AiState::Returning
        )
    }

    /// A state that lasts a counted number of ticks (`Entity::state_ticks` is at least 1 while
    /// it is in force); the others hold no timer (`state_ticks` is 0).
    #[must_use]
    pub fn timed(self) -> bool {
        matches!(
            self,
            AiState::Noticed
                | AiState::Alarm
                | AiState::Alerted
                | AiState::Punching
                | AiState::KnockedDown
                | AiState::Lying
                | AiState::GettingUp
        )
    }
}

/// The one reading of an actor's state the script predicates share (natives 85 / 87 / 90 / 128
/// / 240, `natives.rs`), so that no two of them can contradict each other: `dead` is the
/// `Dead` state, which `World::validate` requires to coincide with `!alive`; `present` is the
/// `active` flag; a knocked-out actor is one down or lying by the blow; `out_of_action` is
/// dead or knocked out; `can_act` is present, alive and on his feet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorStatus {
    /// Dead (the `Dead` state).
    pub dead: bool,
    /// Present on the map (`active`).
    pub present: bool,
    /// Knocked down or lying knocked out (not dead).
    pub knocked_out: bool,
    /// Dead or knocked out: native 90.
    pub out_of_action: bool,
    /// Alive, present and standing: native 128.
    pub can_act: bool,
}

impl ActorStatus {
    /// The status of an entity.
    #[must_use]
    pub fn of(e: &Entity) -> Self {
        let dead = !e.alive || e.ai_state == AiState::Dead;
        let knocked_out = !dead && matches!(e.ai_state, AiState::KnockedDown | AiState::Lying);
        ActorStatus {
            dead,
            present: e.active,
            knocked_out,
            out_of_action: dead || knocked_out,
            can_act: !dead && e.active && e.ai_state.standing(),
        }
    }
}

/// Half opening angle of a soldier's view cone in 1/256 turns: 32 = 45 degrees, a 90 degree
/// cone. Hypothesis (no field of the data reads as an angle; `stealth-and-combat.md` 2.3 / 7.2).
pub const VIEW_CONE_HALF_ANGLE_256: i32 = 32;
/// Range of the view cone in map pixels. Hypothesis (the profile has no field in the 100..500 px
/// range a cone would need; the rails' check-for radii are 25..125 and native 160 compares
/// distances of 10..150, so 200 px puts a guard's sight beyond his patrol checks).
pub const VIEW_RANGE: i32 = 200;
/// A crouched player character is seen at the range over this divisor (manual p. 16: crouching
/// characters are less visible; the factor is a hypothesis).
pub const CROUCH_VIEW_DIVISOR: i32 = 2;
/// Radius in map pixels within which a running player character is heard whatever the
/// soldier faces (manual p. 16: running makes a lot of noise; the console's `NOISE` shows such a
/// radius exists). Hypothesis. Walking and sneaking make no noise here.
pub const RUN_NOISE_RADIUS: i32 = 150;
/// Ticks an alerted soldier keeps searching after his last sighting before he returns to his
/// patrol (5 s at 60 ticks per second). Hypothesis.
pub const ALERT_TIMEOUT_TICKS: u32 = 300;
/// An alerted soldier re-plans his walk when the last seen position moved this far from the
/// point he is walking to (map pixels; bounds the path searches per tick).
pub const REPLAN_DISTANCE: i32 = 32;
/// Knock-out duration in ticks for a victim with no knock-out resistance (10 s at 60 ticks per
/// second); the profile's `p4` shortens it, see [`knock_out_ticks`]. Hypothesis.
pub const KNOCK_OUT_BASE_TICKS: u32 = 600;
/// Knock-out resistance (`profile.md`, SD `p4`) at or above which the blow does not fell the
/// victim at all (the antagonists' 100: "impossible to knock out"). Hypothesis.
pub const KNOCK_OUT_IMMUNE_RESISTANCE: i32 = 100;
/// Reach of the knock-out blow in map pixels: the victim's spot 30-35 px ahead in action 123's
/// displacement (`stealth-and-combat.md` 3.2, observed); 32 is the engine's choice.
pub const PUNCH_REACH: i32 = 32;
/// Half angle of the arc behind the victim, in 1/256 turns, within which the attacker counts as
/// striking from behind: 48 = 67.5 degrees either side of straight behind (a 135 degree arc).
/// Hypothesis (the manual only says the punch works best unseen).
pub const BACK_ARC_HALF_ANGLE_256: i32 = 48;
/// Work units the stealth layer may spend in one tick: perception (one per entity pre-indexed,
/// one per entity inspected, one per soldier / player character pair tested) and the path
/// searches it issues (the units of `nav.rs`, capped per search at `ORDER_SEARCH_WORK`), granted
/// at the start of [`World::ai_tick`] and nowhere else. 2^24: a `MAX_ENTITIES` world of
/// soldiers and no player characters costs 2^17 a tick, and a search over the largest accepted
/// grid (`nav::MAX_CELLS`) fits three times.
pub const AI_WORK_PER_TICK: u64 = 1 << 24;
/// Fallback durations in ticks of the timed states when the profile has no block for the
/// animation (or the world has no catalog): the spec's tick counts of actions 141, 142, 41, 49
/// and 123 (`sprite-animations.md`, "Combat, state and stealth ids"; each tick of the timing
/// word is one world tick here, as the animation player already assumes).
pub const NOTICED_TICKS: u32 = 6;
/// See [`NOTICED_TICKS`].
pub const ALARM_TICKS: u32 = 11;
/// See [`NOTICED_TICKS`].
pub const KNOCKED_DOWN_TICKS: u32 = 13;
/// See [`NOTICED_TICKS`].
pub const GET_UP_TICKS: u32 = 16;
/// See [`NOTICED_TICKS`].
pub const PUNCH_TICKS: u32 = 12;
/// Hit points of a human whose profile gives none (player characters, civilians): the profile
/// table's PC and CV records have no field read as hit points yet (`profile.md`). Hypothesis.
pub const DEFAULT_HIT_POINTS: i32 = 100;

/// Sprite action ids the states report (`sprite-animations.md`); [`action_id`] picks them.
pub mod actions {
    /// Standing idle.
    pub const IDLE: u32 = 0;
    /// Walk.
    pub const WALK: u32 = 6;
    /// Run.
    pub const RUN: u32 = 7;
    /// Crouched idle.
    pub const CROUCH_IDLE: u32 = 14;
    /// Sneak.
    pub const SNEAK: u32 = 16;
    /// Knocked down forward.
    pub const KNOCKED_DOWN: u32 = 41;
    /// Knocked down backward.
    pub const KNOCKED_DOWN_BACK: u32 = 44;
    /// Lying face down.
    pub const LYING: u32 = 47;
    /// Lying on the back.
    pub const LYING_BACK: u32 = 48;
    /// Get up.
    pub const GET_UP: u32 = 49;
    /// Knock-out blow.
    pub const PUNCH: u32 = 123;
    /// Alert idle.
    pub const ALERT_IDLE: u32 = 140;
    /// Noticed.
    pub const NOTICED: u32 = 141;
    /// Alarm.
    pub const ALARM: u32 = 142;
    /// Alert walk.
    pub const ALERT_WALK: u32 = 143;
    /// Alert run.
    pub const ALERT_RUN: u32 = 151;
}

/// Unit scale of the sine table.
pub const UNIT: i32 = 4096;
/// `round(4096 * sin(k * 2 pi / 256))` for a quarter turn.
const SIN_Q: [i32; 65] = [
    0, 101, 201, 301, 401, 501, 601, 700, 799, 897, 995, 1092, 1189, 1285, 1380, 1474, 1567, 1660,
    1751, 1842, 1931, 2019, 2106, 2191, 2276, 2359, 2440, 2520, 2598, 2675, 2751, 2824, 2896, 2967,
    3035, 3102, 3166, 3229, 3290, 3349, 3406, 3461, 3513, 3564, 3612, 3659, 3703, 3745, 3784, 3822,
    3857, 3889, 3920, 3948, 3973, 3996, 4017, 4036, 4052, 4065, 4076, 4085, 4091, 4095, 4096,
];

/// Sine of an angle in 1/256 turns, scaled by [`UNIT`]; exact table lookup, any input.
#[must_use]
pub fn sin256(a: i32) -> i32 {
    let a = a.rem_euclid(256) as usize;
    match a / 64 {
        0 => SIN_Q[a],
        1 => SIN_Q[128 - a],
        2 => -SIN_Q[a - 128],
        _ => -SIN_Q[256 - a],
    }
}

/// Cosine of an angle in 1/256 turns, scaled by [`UNIT`].
#[must_use]
pub fn cos256(a: i32) -> i32 {
    sin256(a.wrapping_add(64))
}

/// Whether the point `(px, py)` lies inside the cone of an observer at `(ox, oy)` facing
/// `facing256` (0 = +x, clockwise on screen), with the given range in map pixels and half angle
/// in 1/256 turns. The observer's own position counts as inside. All arithmetic in `i64`.
#[must_use]
pub fn in_view_cone(
    (ox, oy): (Fixed, Fixed),
    facing256: i32,
    (px, py): (Fixed, Fixed),
    range_px: i32,
    half_angle256: i32,
) -> bool {
    let (dx, dy) = (px - ox, py - oy);
    let len = i64::from(Fixed::length(dx, dy).raw());
    if len > i64::from(range_px) * 256 {
        return false;
    }
    if len == 0 {
        return true;
    }
    let dot = i64::from(cos256(facing256)) * i64::from(dx.raw())
        + i64::from(sin256(facing256)) * i64::from(dy.raw());
    dot >= len * i64::from(cos256(half_angle256))
}

/// Whether an attacker at `(ax, ay)` stands behind a victim at `(tx, ty)` facing `facing256`:
/// within [`BACK_ARC_HALF_ANGLE_256`] of the direction opposite the victim's facing. An attacker
/// on the victim's own position is not behind him.
#[must_use]
pub fn is_behind((tx, ty): (Fixed, Fixed), facing256: i32, (ax, ay): (Fixed, Fixed)) -> bool {
    let (dx, dy) = (ax - tx, ay - ty);
    let len = i64::from(Fixed::length(dx, dy).raw());
    if len == 0 {
        return false;
    }
    let dot = i64::from(cos256(facing256)) * i64::from(dx.raw())
        + i64::from(sin256(facing256)) * i64::from(dy.raw());
    dot <= -(len * i64::from(cos256(BACK_ARC_HALF_ANGLE_256)))
}

/// Knock-out duration for a victim with the given resistance (`p4`, 0..100): the base duration
/// scaled by `(100 - resistance) / 100`; at [`KNOCK_OUT_IMMUNE_RESISTANCE`] the blow does not
/// fell the victim (`None`). Hypothesis.
#[must_use]
pub fn knock_out_ticks(resistance: i32) -> Option<u32> {
    if resistance >= KNOCK_OUT_IMMUNE_RESISTANCE {
        return None;
    }
    let r = resistance.clamp(0, 100) as u64;
    Some((u64::from(KNOCK_OUT_BASE_TICKS) * (100 - r) / 100) as u32)
}

/// The sprite action id an entity's state and movement report (script `ActionChange`, the
/// engine's `action` field): the ids of `sprite-animations.md`.
#[must_use]
pub fn action_id(e: &Entity) -> u32 {
    use actions::{
        ALARM, ALERT_IDLE, ALERT_RUN, ALERT_WALK, CROUCH_IDLE, GET_UP, IDLE, KNOCKED_DOWN,
        KNOCKED_DOWN_BACK, LYING, LYING_BACK, NOTICED, PUNCH, RUN, SNEAK, WALK,
    };
    let moving = e.target.is_some();
    match e.ai_state {
        AiState::Patrol => match (e.posture, moving, e.gait) {
            (Posture::Crouched, true, _) => SNEAK,
            (Posture::Crouched, false, _) => CROUCH_IDLE,
            (Posture::Standing, true, Gait::Run) => RUN,
            (Posture::Standing, true, Gait::Walk) => WALK,
            (Posture::Standing, false, _) => IDLE,
        },
        AiState::Noticed => NOTICED,
        AiState::Alarm => ALARM,
        AiState::Alerted | AiState::Returning => match (moving, e.gait) {
            (true, Gait::Run) => ALERT_RUN,
            (true, Gait::Walk) => ALERT_WALK,
            (false, _) => ALERT_IDLE,
        },
        AiState::Punching => PUNCH,
        AiState::KnockedDown => {
            if e.fell_backward {
                KNOCKED_DOWN_BACK
            } else {
                KNOCKED_DOWN
            }
        }
        AiState::Lying | AiState::Dead => {
            if e.fell_backward {
                LYING_BACK
            } else {
                LYING
            }
        }
        AiState::GettingUp => GET_UP,
    }
}

/// The animation of `set` an entity plays in its state, facing its direction.
#[must_use]
pub fn wanted_animation(e: &Entity, set: &AnimSet) -> u32 {
    let dir = direction_of(e.facing256);
    let moving = e.target.is_some();
    match e.ai_state {
        AiState::Patrol => match (e.posture, moving, e.gait) {
            (Posture::Crouched, true, _) => set.crouch_walk[dir],
            (Posture::Crouched, false, _) => set.crouch_idle[dir],
            (Posture::Standing, true, Gait::Run) => set.run[dir],
            (Posture::Standing, true, Gait::Walk) => set.walk[dir],
            (Posture::Standing, false, _) => set.idle[dir],
        },
        AiState::Noticed => set.noticed[dir],
        AiState::Alarm => set.alarm[dir],
        AiState::Alerted | AiState::Returning => match (moving, e.gait) {
            (true, Gait::Run) => set.alert_run[dir],
            (true, Gait::Walk) => set.alert_walk[dir],
            (false, _) => set.alert_idle[dir],
        },
        AiState::Punching => set.punch[dir],
        AiState::KnockedDown => {
            if e.fell_backward {
                set.knocked_down_back[dir]
            } else {
                set.knocked_down[dir]
            }
        }
        AiState::Lying | AiState::Dead => {
            if e.fell_backward {
                set.lying_back[dir]
            } else {
                set.lying[dir]
            }
        }
        AiState::GettingUp => set.get_up[dir],
    }
}

/// A timed state's duration: the length of the animation it plays for this entity (the block of
/// the entity's profile facing its direction), else `fallback`.
fn state_ticks(world: &World, e: &Entity, block: fn(&AnimSet) -> &[u32; 8], fallback: u32) -> u32 {
    let ticks = e
        .anim
        .as_ref()
        .and_then(|a| world.catalog.sets.get(&a.set))
        .and_then(|set| set.length(block(set)[direction_of(e.facing256)]));
    ticks.unwrap_or(fallback).max(1)
}

/// Whether the entity's profile has the knock-out blow: a set without action 123 cannot punch;
/// an entity without a set (synthetic units, no sprite bank) can.
fn can_punch(world: &World, e: &Entity) -> bool {
    e.anim
        .as_ref()
        .and_then(|a| world.catalog.sets.get(&a.set))
        .is_none_or(|set| set.has_punch)
}

/// Stop where the entity is: no order, no path, walking gait.
fn stop(e: &mut Entity) {
    e.target = None;
    e.path.clear();
    e.gait = Gait::Walk;
}

/// Count the state timer down; `true` when it reached zero on this tick.
fn countdown(e: &mut Entity) -> bool {
    e.state_ticks = e.state_ticks.saturating_sub(1);
    e.state_ticks == 0
}

/// A player character a soldier can perceive.
fn perceivable(p: &Entity) -> bool {
    p.kind == EntityKind::Player && p.alive && p.active
}

/// A soldier whose perception runs: a living, active, unlocked enemy on his feet (a locked AI
/// neither perceives nor advances its alert: script natives 134 / 135, `scb.md`).
fn perceives(s: &Entity) -> bool {
    s.kind == EntityKind::Guard
        && s.team == Team::Enemy
        && s.alive
        && s.active
        && !s.ai_locked
        && s.ai_state.standing()
        && s.ai_state != AiState::Punching
}

/// Whether soldier `s` sees or hears player character `p` this tick.
fn stimulus(s: &Entity, p: &Entity) -> bool {
    let range = if p.posture == Posture::Crouched {
        VIEW_RANGE / CROUCH_VIEW_DIVISOR
    } else {
        VIEW_RANGE
    };
    if in_view_cone(
        (s.x, s.y),
        s.facing256,
        (p.x, p.y),
        range,
        VIEW_CONE_HALF_ANGLE_256,
    ) {
        return true;
    }
    let noisy = p.target.is_some() && p.gait == Gait::Run && p.posture == Posture::Standing;
    noisy && Fixed::length(p.x - s.x, p.y - s.y) <= Fixed::from_int(RUN_NOISE_RADIUS)
}

impl World {
    /// One tick of the stealth layer: every soldier's perception and alert state and the timed
    /// states of every human, then the player characters' attack orders (so a state a blow
    /// enters lasts its full duration from the next tick on). Runs before the waypoint programs
    /// (which only Patrol-state guards execute) and before the movement. Returns the work units
    /// spent of [`AI_WORK_PER_TICK`].
    pub(crate) fn ai_tick(&mut self) -> u64 {
        self.ai_tick_with(AI_WORK_PER_TICK)
    }

    /// [`World::ai_tick`] with an explicit budget (tests exercise the exhaustion paths without
    /// a `MAX_ENTITIES` world); returns the units spent.
    pub(crate) fn ai_tick_with(&mut self, budget: u64) -> u64 {
        let mut left = budget;
        let stimuli = self.perception(&mut left);
        for i in 0..self.entities.len() {
            let e = &self.entities[i];
            if !e.alive || !e.active || e.kind == EntityKind::Obstacle || e.ai_locked {
                continue;
            }
            self.advance_state(i, stimuli.get(i).copied().flatten(), &mut left);
        }
        self.attack_orders(&mut left);
        budget - left
    }

    /// The position of the first player character (in slot order) each soldier perceives this
    /// tick. The perceivable player characters are indexed once (one unit per entity), then
    /// the entities are inspected from `ai_cursor` on, round robin (one unit each, plus one
    /// per player character tested for a soldier); when the budget runs out the cursor stays on
    /// the entity not finished, which perceives nothing this tick, and the next tick resumes
    /// there; a completed scan resets the cursor to 0.
    fn perception(&mut self, budget: &mut u64) -> Vec<Option<(Fixed, Fixed)>> {
        let n = self.entities.len();
        let mut out = vec![None; n];
        if n == 0 {
            self.ai_cursor = 0;
            return out;
        }
        if !charge_budget(budget, n as u64) {
            return out;
        }
        let players: Vec<usize> = self
            .entities
            .iter()
            .enumerate()
            .filter(|(_, p)| perceivable(p))
            .map(|(i, _)| i)
            .collect();
        let start = (self.ai_cursor as usize).min(n - 1);
        let mut cursor = 0u32;
        'scan: for k in 0..n {
            let i = (start + k) % n;
            if !charge_budget(budget, 1) {
                cursor = i as u32;
                break 'scan;
            }
            let s = &self.entities[i];
            if !perceives(s) {
                continue;
            }
            for &pi in &players {
                if !charge_budget(budget, 1) {
                    cursor = i as u32;
                    break 'scan;
                }
                let p = &self.entities[pi];
                if stimulus(s, p) {
                    out[i] = Some((p.x, p.y));
                    break;
                }
            }
        }
        self.ai_cursor = cursor;
        out
    }

    /// Order the entity to walk to a point at the given gait; the search draws from the tick's
    /// AI budget, capped at the per-order budget (`ORDER_SEARCH_WORK`); `true` when a path was
    /// found (an exhausted budget leaves the entity standing, like an unreachable target).
    fn walk_to(&mut self, i: usize, to: (Fixed, Fixed), gait: Gait, budget: &mut u64) -> bool {
        let granted = (*budget).min(ORDER_SEARCH_WORK);
        let mut search = granted;
        let _ = self.plan_path_with(i, to, &mut search);
        *budget -= granted - search;
        let e = &mut self.entities[i];
        if e.target.is_some() {
            e.gait = gait;
            true
        } else {
            false
        }
    }

    /// Enter a timed state whose duration is its animation's length (or the fallback).
    fn enter_timed(
        &mut self,
        i: usize,
        state: AiState,
        block: fn(&AnimSet) -> &[u32; 8],
        fallback: u32,
    ) {
        let ticks = state_ticks(self, &self.entities[i], block, fallback);
        let e = &mut self.entities[i];
        e.ai_state = state;
        e.state_ticks = ticks;
    }

    /// Back to the patrol: the waypoint program continues where the alert interrupted it.
    fn resume_patrol(&mut self, i: usize) {
        let e = &mut self.entities[i];
        e.ai_state = AiState::Patrol;
        e.state_ticks = 0;
        e.last_seen = None;
        e.alert_origin = None;
    }

    /// Walk back to the alert origin (`Returning`), or patrol at once when there is none or the
    /// entity already stands there or the way back cannot be found.
    fn go_back(&mut self, i: usize, budget: &mut u64) {
        let e = &self.entities[i];
        let here = (e.x, e.y);
        match e.alert_origin {
            Some(origin) if origin != here => {
                if self.walk_to(i, origin, Gait::Walk, budget) {
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::Returning;
                    e.state_ticks = 0;
                } else {
                    self.resume_patrol(i);
                }
            }
            _ => self.resume_patrol(i),
        }
    }

    /// A stimulus reaches a soldier in a normal state: he notices it (141).
    fn notice(&mut self, i: usize, seen: (Fixed, Fixed)) {
        let e = &mut self.entities[i];
        if e.ai_state == AiState::Patrol {
            e.alert_origin = Some((e.x, e.y));
        }
        e.last_seen = Some(seen);
        stop(e);
        self.enter_timed(i, AiState::Noticed, |s| &s.noticed, NOTICED_TICKS);
    }

    /// The state machine of one human for this tick.
    fn advance_state(&mut self, i: usize, seen: Option<(Fixed, Fixed)>, budget: &mut u64) {
        match self.entities[i].ai_state {
            AiState::Patrol | AiState::Returning => {
                if let Some(p) = seen {
                    self.notice(i, p);
                } else if self.entities[i].ai_state == AiState::Returning
                    && self.entities[i].target.is_none()
                {
                    self.resume_patrol(i);
                }
            }
            AiState::Noticed => {
                if seen.is_some() {
                    self.entities[i].last_seen = seen;
                }
                if countdown(&mut self.entities[i]) {
                    self.enter_timed(i, AiState::Alarm, |s| &s.alarm, ALARM_TICKS);
                }
            }
            AiState::Alarm => {
                if seen.is_some() {
                    self.entities[i].last_seen = seen;
                }
                if countdown(&mut self.entities[i]) {
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::Alerted;
                    e.state_ticks = ALERT_TIMEOUT_TICKS;
                    if let Some(p) = e.last_seen {
                        self.walk_to(i, p, Gait::Run, budget);
                    }
                }
            }
            AiState::Alerted => match seen {
                Some(p) => {
                    let e = &mut self.entities[i];
                    e.last_seen = Some(p);
                    e.state_ticks = ALERT_TIMEOUT_TICKS;
                    let stale = e.target.is_none_or(|(tx, ty)| {
                        Fixed::length(tx - p.0, ty - p.1) > Fixed::from_int(REPLAN_DISTANCE)
                    });
                    if stale && Fixed::length(e.x - p.0, e.y - p.1) > Fixed::from_int(PUNCH_REACH) {
                        self.walk_to(i, p, Gait::Run, budget);
                    }
                }
                None => {
                    if countdown(&mut self.entities[i]) {
                        self.go_back(i, budget);
                    }
                }
            },
            AiState::Punching => {
                if countdown(&mut self.entities[i]) {
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::Patrol;
                }
            }
            AiState::KnockedDown => {
                if countdown(&mut self.entities[i]) {
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::Lying;
                    e.state_ticks = knock_out_ticks(e.knockout_resistance).unwrap_or(1);
                }
            }
            AiState::Lying => {
                if countdown(&mut self.entities[i]) {
                    self.enter_timed(i, AiState::GettingUp, |s| &s.get_up, GET_UP_TICKS);
                }
            }
            AiState::GettingUp => {
                if countdown(&mut self.entities[i]) {
                    self.go_back(i, budget);
                }
            }
            AiState::Dead => {}
        }
    }

    /// The player characters' attack orders (`docs/original/stealth-and-combat.md` 1, tutorial
    /// string 14; the order model is a hypothesis: a left click on an enemy with a character
    /// selected walks into reach, then punches when behind the victim, else stops facing him).
    fn attack_orders(&mut self, budget: &mut u64) {
        for i in 0..self.entities.len() {
            let e = &self.entities[i];
            let Some(target) = e.attack_target else {
                continue;
            };
            if !(e.alive && e.active && e.kind == EntityKind::Player)
                || e.ai_state != AiState::Patrol
            {
                continue;
            }
            let victim = self.entities.iter().position(|v| v.id == target);
            let valid = victim.is_some_and(|t| {
                let v = &self.entities[t];
                v.alive && v.active && v.kind == EntityKind::Guard && v.ai_state.standing()
            });
            let Some(t) = victim.filter(|_| valid) else {
                self.entities[i].attack_target = None;
                continue;
            };
            let (vx, vy) = (self.entities[t].x, self.entities[t].y);
            let (dx, dy) = (vx - self.entities[i].x, vy - self.entities[i].y);
            if Fixed::length(dx, dy) > Fixed::from_int(PUNCH_REACH) {
                if self.entities[i].target.is_none()
                    && !self.walk_to(i, (vx, vy), Gait::Walk, budget)
                {
                    // Unreachable: the order is dropped.
                    self.entities[i].attack_target = None;
                }
                continue;
            }
            // In reach: stop, face the victim, strike when behind him.
            let attacker_pos = (self.entities[i].x, self.entities[i].y);
            let e = &mut self.entities[i];
            stop(e);
            e.attack_target = None;
            if dx.raw() != 0 || dy.raw() != 0 {
                e.facing256 = facing_of(dx, dy);
            }
            let behind = is_behind((vx, vy), self.entities[t].facing256, attacker_pos);
            if !(behind && can_punch(self, &self.entities[i])) {
                continue;
            }
            self.enter_timed(i, AiState::Punching, |s| &s.punch, PUNCH_TICKS);
            self.knock_down(t, attacker_pos);
        }
    }

    /// The blow lands on `t` from `from`: the victim goes down forward when struck from behind,
    /// backward otherwise (41 / 44), unless his resistance makes him immune, in which case the
    /// blow is a stimulus (he notices the attacker). The resistance is the profile's `p4`
    /// (hypothesis): consulting it records `Assumption::ProfileStats` on the script, if any.
    fn knock_down(&mut self, t: usize, from: (Fixed, Fixed)) {
        self.record_assumption(Assumption::ProfileStats);
        let v = &self.entities[t];
        if knock_out_ticks(v.knockout_resistance).is_none() {
            if v.team == Team::Enemy && matches!(v.ai_state, AiState::Patrol | AiState::Returning) {
                self.notice(t, from);
            }
            return;
        }
        let backward = !is_behind((v.x, v.y), v.facing256, from);
        let e = &mut self.entities[t];
        stop(e);
        e.attack_target = None;
        e.fell_backward = backward;
        if e.alert_origin.is_none() {
            e.alert_origin = Some((e.x, e.y));
        }
        if backward {
            self.enter_timed(
                t,
                AiState::KnockedDown,
                |s| &s.knocked_down_back,
                KNOCKED_DOWN_TICKS,
            );
        } else {
            self.enter_timed(
                t,
                AiState::KnockedDown,
                |s| &s.knocked_down,
                KNOCKED_DOWN_TICKS,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Button, InputEvent, Key};
    use crate::world::{Scenario, Snapshot};

    fn f(v: i32) -> Fixed {
        Fixed::from_int(v)
    }

    #[test]
    fn sine_table_symmetries() {
        assert_eq!((sin256(0), cos256(0)), (0, UNIT));
        assert_eq!((sin256(64), cos256(64)), (UNIT, 0));
        assert_eq!((sin256(128), cos256(128)), (0, -UNIT));
        assert_eq!((sin256(192), cos256(192)), (-UNIT, 0));
        assert_eq!(sin256(32), 2896);
        assert_eq!(sin256(-32), -2896);
        assert_eq!(sin256(96), 2896);
        assert_eq!(sin256(160), -2896);
        assert_eq!(sin256(224), -2896);
        assert_eq!(cos256(BACK_ARC_HALF_ANGLE_256), 1567);
        for a in -600..600 {
            let (s, c) = (i64::from(sin256(a)), i64::from(cos256(a)));
            let r = s * s + c * c;
            assert!(
                (r - i64::from(UNIT) * i64::from(UNIT)).abs() < 8000,
                "{a}: {r}"
            );
        }
    }

    #[test]
    fn view_cone_geometry() {
        let o = (f(100), f(100));
        // Facing +x: points ahead inside, behind and sideways outside, range respected.
        assert!(in_view_cone(o, 0, (f(200), f(100)), 200, 32));
        assert!(
            in_view_cone(o, 0, (f(200), f(190)), 200, 32),
            "just inside 45 degrees"
        );
        assert!(
            !in_view_cone(o, 0, (f(200), f(210)), 200, 32),
            "just outside 45 degrees"
        );
        assert!(!in_view_cone(o, 0, (f(100), f(200)), 200, 32), "sideways");
        assert!(!in_view_cone(o, 0, (f(0), f(100)), 200, 32), "behind");
        assert!(
            in_view_cone(o, 0, (f(300), f(100)), 200, 32),
            "at the range"
        );
        assert!(
            !in_view_cone(o, 0, (f(301), f(100)), 200, 32),
            "beyond the range"
        );
        assert!(in_view_cone(o, 0, o, 200, 32), "the observer's own spot");
        // Facing screen-down (+y, facing 64) and the diagonals.
        assert!(in_view_cone(o, 64, (f(100), f(250)), 200, 32));
        assert!(!in_view_cone(o, 64, (f(100), f(0)), 200, 32));
        assert!(in_view_cone(o, 32, (f(200), f(200)), 200, 32));
        assert!(in_view_cone(o, 192, (f(100), f(0)), 200, 32));
        // Extremes never panic.
        let big = (Fixed::MAX, Fixed::MIN);
        let _ = in_view_cone(big, i32::MIN, (Fixed::MIN, Fixed::MAX), i32::MAX, i32::MAX);
        let _ = is_behind(big, i32::MAX, (Fixed::MIN, Fixed::MIN));
    }

    #[test]
    fn behind_arc_geometry() {
        let victim = (f(100), f(100));
        // The victim faces +x: an attacker at -x is behind, at +x in front, sideways not behind.
        assert!(is_behind(victim, 0, (f(70), f(100))));
        assert!(!is_behind(victim, 0, (f(130), f(100))));
        assert!(!is_behind(victim, 0, (f(100), f(130))));
        // 67.5 degrees off straight behind is the limit: 60 degrees counts, 75 does not.
        assert!(is_behind(victim, 0, (f(100 - 50), f(100 + 86))));
        assert!(!is_behind(victim, 0, (f(100 - 26), f(100 + 96))));
        assert!(!is_behind(victim, 0, victim));
        assert!(is_behind(victim, 128, (f(130), f(100))));
    }

    #[test]
    fn knock_out_duration_scales_with_the_resistance() {
        assert_eq!(knock_out_ticks(0), Some(KNOCK_OUT_BASE_TICKS));
        assert_eq!(knock_out_ticks(-5), Some(KNOCK_OUT_BASE_TICKS));
        assert_eq!(knock_out_ticks(50), Some(KNOCK_OUT_BASE_TICKS / 2));
        assert_eq!(knock_out_ticks(75), Some(KNOCK_OUT_BASE_TICKS / 4));
        assert_eq!(knock_out_ticks(99), Some(KNOCK_OUT_BASE_TICKS / 100));
        assert_eq!(knock_out_ticks(100), None);
        assert_eq!(knock_out_ticks(i32::MAX), None);
    }

    /// The corridor with the guard parked at a spot facing `facing`, not patrolling, and the
    /// player selected.
    fn scene(guard: (i32, i32), facing: i32, player: (i32, i32)) -> World {
        let mut w = World::new(Scenario::Synthetic("corridor".into()), 3).unwrap();
        let g = &mut w.entities[1];
        g.patrol.clear();
        g.patrol_index = 0;
        g.x = f(guard.0);
        g.y = f(guard.1);
        g.facing256 = facing;
        let p = &mut w.entities[0];
        p.x = f(player.0);
        p.y = f(player.1);
        w.selected = Some(w.entities[0].id);
        w.validate().unwrap();
        w
    }

    fn click(w: &mut World, x: i32, y: i32, button: Button) {
        w.step(&[
            InputEvent::PointerMove {
                x256: x * 256,
                y256: y * 256,
            },
            InputEvent::PointerDown { button },
            InputEvent::PointerUp { button },
        ]);
    }

    fn states(w: &World) -> (AiState, AiState) {
        (w.entities[0].ai_state, w.entities[1].ai_state)
    }

    #[test]
    fn a_player_in_the_cone_is_noticed_then_the_alarm_then_the_search_then_the_return() {
        // The guard at (400, 240) faces -x; the player stands 150 px in front of him.
        let mut w = scene((400, 240), 128, (250, 240));
        w.step(&[]);
        let g = &w.entities[1];
        assert_eq!(g.ai_state, AiState::Noticed);
        assert_eq!(g.state_ticks, NOTICED_TICKS);
        assert_eq!(g.last_seen, Some((f(250), f(240))));
        assert_eq!(g.alert_origin, Some((f(400), f(240))));
        assert_eq!(g.action, actions::NOTICED);
        for _ in 1..NOTICED_TICKS {
            w.step(&[]);
            assert_eq!(w.entities[1].ai_state, AiState::Noticed);
        }
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::Alarm);
        assert_eq!(w.entities[1].action, actions::ALARM);
        for _ in 1..ALARM_TICKS {
            w.step(&[]);
        }
        w.step(&[]);
        let g = &w.entities[1];
        assert_eq!(g.ai_state, AiState::Alerted);
        assert_eq!(g.gait, Gait::Run);
        assert!(g.target.is_some(), "runs to the last seen position");
        assert_eq!(g.action, actions::ALERT_RUN);
        // Move the player out of sight (behind the guard's back, far away) by hand: the guard
        // reaches the spot, waits out the alert timeout and walks back to his post.
        w.entities[0].x = f(630);
        w.entities[0].y = f(60);
        let mut ticks = 0;
        while w.entities[1].ai_state == AiState::Alerted {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < ALERT_TIMEOUT_TICKS + 500, "never gave up");
        }
        let g = &w.entities[1];
        assert_eq!(g.ai_state, AiState::Returning);
        assert!(g.target.is_some() && g.gait == Gait::Walk);
        assert_eq!(g.action, actions::ALERT_WALK);
        let mut ticks = 0;
        while w.entities[1].ai_state == AiState::Returning {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 1000, "never came back");
        }
        let g = &w.entities[1];
        assert_eq!(g.ai_state, AiState::Patrol);
        assert_eq!((g.x.round(), g.y.round()), (400, 240));
        assert!(g.last_seen.is_none() && g.alert_origin.is_none());
        w.validate().unwrap();
    }

    #[test]
    fn crouching_halves_the_sight_range_and_the_back_is_blind_unless_running() {
        // 150 px in front, crouched: beyond the halved range, unseen.
        let mut w = scene((400, 240), 128, (250, 240));
        w.entities[0].posture = Posture::Crouched;
        for _ in 0..20 {
            w.step(&[]);
        }
        assert_eq!(states(&w).1, AiState::Patrol);
        // 90 px in front, crouched: inside the halved range.
        w.entities[0].x = f(310);
        w.step(&[]);
        assert_eq!(states(&w).1, AiState::Noticed);
        // Behind the guard's back, standing still: not seen.
        let mut w = scene((300, 240), 128, (400, 240));
        for _ in 0..20 {
            w.step(&[]);
        }
        assert_eq!(states(&w).1, AiState::Patrol);
        // Behind the guard's back but running within the noise radius: heard.
        click(&mut w, 500, 240, Button::Left);
        click(&mut w, 500, 240, Button::Left);
        assert_eq!(w.entities[0].gait, Gait::Run);
        assert_eq!(states(&w).1, AiState::Noticed);
        // Walking behind him is silent.
        let mut w = scene((300, 240), 128, (400, 240));
        click(&mut w, 500, 240, Button::Left);
        for _ in 0..10 {
            w.step(&[]);
        }
        assert_eq!(states(&w).1, AiState::Patrol);
        // A locked AI perceives nothing.
        let mut w = scene((400, 240), 128, (250, 240));
        w.entities[1].ai_locked = true;
        for _ in 0..5 {
            w.step(&[]);
        }
        assert_eq!(states(&w).1, AiState::Patrol);
        // Neither does a civilian.
        let mut w = scene((400, 240), 128, (250, 240));
        w.entities[1].team = Team::Civilian;
        for _ in 0..5 {
            w.step(&[]);
        }
        assert_eq!(states(&w).1, AiState::Patrol);
        w.validate().unwrap();
    }

    #[test]
    fn knock_out_from_behind_and_a_stop_from_the_front() {
        // The guard at (400, 240) faces -x (away from the player at (500, 240)).
        let mut w = scene((400, 240), 128, (500, 240));
        click(&mut w, 400, 240, Button::Left);
        let p = &w.entities[0];
        assert_eq!(p.attack_target, Some(w.entities[1].id));
        assert!(p.target.is_some(), "walks into reach");
        assert_eq!(
            w.selected,
            Some(w.entities[0].id),
            "the click is an order, not a selection"
        );
        let mut ticks = 0;
        while w.entities[0].ai_state != AiState::Punching {
            assert_eq!(
                w.entities[1].ai_state,
                AiState::Patrol,
                "unseen: never noticed"
            );
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 200, "never struck: {:?}", states(&w));
        }
        let (p, g) = (&w.entities[0], &w.entities[1]);
        assert!(p.attack_target.is_none() && p.target.is_none());
        assert_eq!(p.action, actions::PUNCH);
        assert_eq!(p.facing256, 128, "faces the victim");
        assert!(Fixed::length(p.x - g.x, p.y - g.y) <= f(PUNCH_REACH));
        assert_eq!(g.ai_state, AiState::KnockedDown);
        assert!(!g.fell_backward);
        assert_eq!(g.action, actions::KNOCKED_DOWN);
        assert_eq!(g.state_ticks, KNOCKED_DOWN_TICKS);
        assert_eq!(g.alert_origin, Some((f(400), f(240))));
        for _ in 0..PUNCH_TICKS {
            w.step(&[]);
        }
        assert_eq!(w.entities[0].ai_state, AiState::Patrol);
        assert_eq!(w.entities[1].ai_state, AiState::KnockedDown);
        for _ in PUNCH_TICKS..KNOCKED_DOWN_TICKS {
            w.step(&[]);
        }
        assert_eq!(w.entities[1].ai_state, AiState::Lying);
        assert_eq!(w.entities[1].state_ticks, KNOCK_OUT_BASE_TICKS);
        assert_eq!(w.entities[1].action, actions::LYING);
        // While he lies there the player stands in his (blind) cone: nothing happens.
        for _ in 0..KNOCK_OUT_BASE_TICKS - 1 {
            w.step(&[]);
            assert_eq!(w.entities[1].ai_state, AiState::Lying);
        }
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::GettingUp);
        assert_eq!(w.entities[1].action, actions::GET_UP);
        for _ in 0..GET_UP_TICKS {
            w.step(&[]);
        }
        assert_eq!(
            w.entities[1].ai_state,
            AiState::Patrol,
            "up and back on duty"
        );
        w.validate().unwrap();

        // From the front: the guard faces the player; the character walks up, stops and faces
        // him, no blow, the guard has noticed him meanwhile.
        let mut w = scene((400, 240), 0, (500, 240));
        click(&mut w, 400, 240, Button::Left);
        let mut ticks = 0;
        while w.entities[0].attack_target.is_some() {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 200);
        }
        let p = &w.entities[0];
        assert_ne!(p.ai_state, AiState::Punching);
        assert_eq!(p.facing256, 128);
        assert!(p.target.is_none());
        assert!(w.entities[1].ai_state.alert());
        assert!(w.entities[1].ai_state.standing());
        // Right click cancels an attack order like a walk.
        let mut w = scene((400, 240), 128, (500, 240));
        click(&mut w, 400, 240, Button::Left);
        assert!(w.entities[0].attack_target.is_some());
        click(&mut w, 500, 240, Button::Right);
        assert!(w.entities[0].attack_target.is_none() && w.entities[0].target.is_none());
        w.validate().unwrap();
    }

    #[test]
    fn immune_victims_notice_the_blow_and_resistance_shortens_the_sleep() {
        let mut w = scene((400, 240), 128, (500, 240));
        w.entities[1].knockout_resistance = 50;
        click(&mut w, 400, 240, Button::Left);
        for _ in 0..200 {
            w.step(&[]);
            if w.entities[1].ai_state == AiState::Lying {
                break;
            }
        }
        assert_eq!(w.entities[1].ai_state, AiState::Lying);
        assert_eq!(w.entities[1].state_ticks, KNOCK_OUT_BASE_TICKS / 2);
        let mut w = scene((400, 240), 128, (500, 240));
        w.entities[1].knockout_resistance = KNOCK_OUT_IMMUNE_RESISTANCE;
        click(&mut w, 400, 240, Button::Left);
        for _ in 0..200 {
            w.step(&[]);
        }
        assert!(
            w.entities[1].ai_state.standing(),
            "never fell: {:?}",
            w.entities[1].ai_state
        );
        assert!(w.entities[0].ai_state != AiState::Punching);
    }

    #[test]
    fn a_profile_without_the_punch_cannot_strike() {
        use crate::anim::{AnimSet, Catalog, FrameSpec};
        let frame = |frame| FrameSpec {
            frame,
            duration: 1,
            offset_x: 0,
            offset_y: 0,
        };
        let mut catalog = Catalog::default();
        let mut set = AnimSet::standing_only(vec![vec![frame(0)], vec![frame(1)]], [0; 8], [1; 8]);
        set.has_punch = false;
        catalog.sets.insert("hero".into(), set.clone());
        let mut w = scene((400, 240), 128, (500, 240));
        w.attach_catalog(catalog, Some("hero"), Some("hero"));
        click(&mut w, 400, 240, Button::Left);
        for _ in 0..200 {
            w.step(&[]);
        }
        assert_eq!(states(&w), (AiState::Patrol, AiState::Patrol));
        assert!(w.entities[0].target.is_none() && w.entities[0].attack_target.is_none());
    }

    #[test]
    fn timed_states_last_their_animation_with_a_catalog() {
        use crate::anim::{AnimSet, Catalog, FrameSpec};
        let frame = |duration| FrameSpec {
            frame: 0,
            duration,
            offset_x: 0,
            offset_y: 0,
        };
        // Animation 2 is the noticed block (3 + 4 ticks), 3 the alarm block (2 ticks).
        let mut set = AnimSet::standing_only(
            vec![
                vec![frame(1)],
                vec![frame(1)],
                vec![frame(3), frame(4)],
                vec![frame(2)],
            ],
            [0; 8],
            [1; 8],
        );
        set.noticed = [2; 8];
        set.alarm = [3; 8];
        let mut catalog = Catalog::default();
        catalog.sets.insert("soldier".into(), set);
        let mut w = scene((400, 240), 128, (250, 240));
        w.attach_catalog(catalog, None, Some("soldier"));
        w.step(&[]);
        let g = &w.entities[1];
        assert_eq!((g.ai_state, g.state_ticks), (AiState::Noticed, 7));
        assert_eq!(g.anim.as_ref().unwrap().animation, 2);
        for _ in 0..6 {
            w.step(&[]);
        }
        assert_eq!(w.entities[1].ai_state, AiState::Noticed);
        assert_eq!(w.entities[1].anim.as_ref().unwrap().frame, 1);
        w.step(&[]);
        let g = &w.entities[1];
        assert_eq!((g.ai_state, g.state_ticks), (AiState::Alarm, 2));
        assert_eq!(g.anim.as_ref().unwrap().animation, 3);
        w.validate().unwrap();
    }

    #[test]
    fn stealth_state_survives_snapshots_and_is_validated() {
        let mut w = scene((400, 240), 128, (500, 240));
        click(&mut w, 400, 240, Button::Left);
        for _ in 0..80 {
            w.step(&[]);
        }
        assert_eq!(w.entities[1].ai_state, AiState::Lying);
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        assert!(json.contains("\"ai_state\":\"lying\""));
        assert!(json.contains("\"team\":\"enemy\""));
        let snap: Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = World::new(Scenario::Synthetic("corridor".into()), 3).unwrap();
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        for _ in 0..700 {
            w.step(&[]);
            w2.step(&[]);
        }
        assert_eq!(w2.hashes(), w.hashes());
        assert_eq!(w.entities[1].ai_state, AiState::Patrol);
        // Rejected: a player on the enemy team, an attack on a missing entity, positions out of
        // range, a timer beyond the bound.
        let reject = |w: &mut World, edit: fn(&mut World), needle: &str| {
            let mut snap = w.snapshot(None);
            edit(&mut snap.world);
            let err = w.restore(&snap).unwrap_err();
            assert!(err.contains(needle), "{err} should mention {needle}");
        };
        reject(&mut w, |s| s.entities[0].team = Team::Enemy, "team");
        reject(&mut w, |s| s.entities[1].team = Team::Player, "team");
        reject(
            &mut w,
            |s| {
                s.entities[0].attack_target = Some(crate::world::EntityId {
                    index: 77,
                    generation: 1,
                });
            },
            "attack target",
        );
        reject(
            &mut w,
            |s| s.entities[1].last_seen = Some((Fixed::MAX, Fixed::ZERO)),
            "last seen",
        );
        reject(
            &mut w,
            |s| s.entities[1].alert_origin = Some((Fixed::ZERO, Fixed::MIN)),
            "alert origin",
        );
        reject(
            &mut w,
            |s| s.entities[1].state_ticks = u32::MAX,
            "state ticks",
        );
        // Semantic invariants: `Dead` and `alive` agree, timed states carry a timer and
        // untimed ones none, the attack order goes from a player character to an enemy soldier,
        // alert states belong to enemy soldiers, the blow to player characters, a returning
        // soldier knows his origin, a last sighting belongs to an alert state.
        reject(&mut w, |s| s.entities[1].ai_state = AiState::Dead, "dead");
        reject(&mut w, |s| s.entities[1].alive = false, "dead");
        reject(&mut w, |s| s.entities[1].state_ticks = 3, "state ticks");
        reject(
            &mut w,
            |s| {
                s.entities[1].ai_state = AiState::Alerted;
                s.entities[1].state_ticks = 0;
            },
            "state ticks",
        );
        reject(
            &mut w,
            |s| s.entities[1].attack_target = Some(s.entities[0].id),
            "attack target",
        );
        reject(
            &mut w,
            |s| {
                s.entities[1].team = Team::Civilian;
                s.entities[0].attack_target = Some(s.entities[1].id);
            },
            "attack target",
        );
        reject(
            &mut w,
            |s| {
                s.entities[0].ai_state = AiState::Noticed;
                s.entities[0].state_ticks = 2;
            },
            "alert state",
        );
        reject(
            &mut w,
            |s| {
                s.entities[1].ai_state = AiState::Punching;
                s.entities[1].state_ticks = 2;
            },
            "punch",
        );
        reject(
            &mut w,
            |s| {
                s.entities[1].ai_state = AiState::Returning;
                s.entities[1].alert_origin = None;
            },
            "origin",
        );
        reject(
            &mut w,
            |s| s.entities[1].last_seen = Some((Fixed::ONE, Fixed::ONE)),
            "last seen",
        );
        // The consistent forms are accepted.
        let mut snap = w.snapshot(None);
        snap.world.entities[1].ai_state = AiState::Dead;
        snap.world.entities[1].alive = false;
        snap.world.entities[1].state_ticks = 0;
        w.restore(&snap).unwrap();
        w.validate().unwrap();
        // Keys still act on a punching character's posture only after the blow: no panic.
        w.step(&[InputEvent::KeyDown {
            key: Key::Letter('c'),
        }]);
        w.validate().unwrap();
    }

    /// The actor status the script predicates share: `Dead` (with `alive` cleared) is dead
    /// and out of action, a knocked-out actor is out of action but not dead, a deactivated one
    /// is absent, and only a standing, present, living actor can act.
    #[test]
    fn actor_status_is_one_reading() {
        let mut w = scene((400, 240), 128, (500, 240));
        let g = |w: &World| ActorStatus::of(&w.entities[1]);
        assert_eq!(
            g(&w),
            ActorStatus {
                dead: false,
                present: true,
                knocked_out: false,
                out_of_action: false,
                can_act: true
            }
        );
        w.entities[1].ai_state = AiState::Lying;
        let s = g(&w);
        assert!(s.knocked_out && s.out_of_action && !s.dead && !s.can_act && s.present);
        w.entities[1].ai_state = AiState::GettingUp;
        let s = g(&w);
        assert!(!s.knocked_out && !s.out_of_action && !s.can_act);
        w.entities[1].ai_state = AiState::Dead;
        w.entities[1].alive = false;
        let s = g(&w);
        assert!(s.dead && s.out_of_action && !s.knocked_out && !s.can_act && s.present);
        w.entities[1].ai_state = AiState::Patrol;
        w.entities[1].alive = true;
        w.entities[1].active = false;
        let s = g(&w);
        assert!(!s.dead && !s.present && !s.can_act);
    }

    /// An open 1000x800 mission of `guards` enemy soldiers at distinct spots facing +x, with
    /// `players` player characters at the far corner.
    fn crowd(guards: usize, players: usize) -> World {
        use crate::geom::Geometry;
        use crate::world::{ActorSpec, MapInfo, MissionSpec, Scenario};
        let mut actors = Vec::with_capacity(guards + players);
        for i in 0..players {
            actors.push(ActorSpec {
                profile: "RobinHood".into(),
                team: Team::Player,
                x: 900 + (i % 50) as i32,
                y: 700 + (i / 50 % 50) as i32,
                facing256: 0,
                patrol: vec![],
                program: vec![],
                active: true,
                hit_points: 100,
                knockout_resistance: 0,
            });
        }
        for i in 0..guards {
            actors.push(ActorSpec {
                profile: "Soldier A00".into(),
                team: Team::Enemy,
                x: 100 + (i % 400) as i32,
                y: 100 + (i / 400 % 400) as i32,
                facing256: 0,
                patrol: vec![],
                program: vec![],
                active: true,
                hit_points: 80,
                knockout_resistance: 0,
            });
        }
        let spec = MissionSpec {
            map: MapInfo {
                width: 1000,
                height: 800,
            },
            geometry: Geometry {
                boundary: vec![(0, 0), (1000, 0), (1000, 800), (0, 800)],
                obstacles: vec![],
                areas: Vec::new(),
            },
            actors,
            script: None,
            rails: Vec::new(),
            lenient_natives: false,
            starting_money: 0,
            assumptions: std::collections::BTreeSet::new(),
        };
        World::new_mission(Scenario::Mission("crowd".into()), 2, &spec).unwrap()
    }

    /// The largest accepted world of soldiers and no player character costs exactly two units
    /// per entity (the pre-index pass and the inspection) and finishes its scan every tick.
    #[test]
    fn perception_charges_every_inspected_entity_with_no_player_to_perceive() {
        let n = crate::world::MAX_ENTITIES;
        let mut w = crowd(n, 0);
        assert_eq!(w.entities.len(), n);
        let spent = w.ai_tick();
        assert_eq!(spent, 2 * n as u64);
        assert!(spent < AI_WORK_PER_TICK);
        assert_eq!(w.ai_cursor, 0, "the scan completed");
        assert!(w.entities.iter().all(|e| e.ai_state == AiState::Patrol));
    }

    /// Soldiers and player characters: every pair is charged, an exhausted budget stops the
    /// scan at a cursor that the next tick resumes from, the cursor is authoritative (snapshot,
    /// hash, validation) and the sweep visits every soldier exactly once per round.
    #[test]
    fn perception_resumes_from_its_cursor_when_the_budget_runs_out() {
        let guards = 40;
        let players = 3;
        let n = guards + players;
        let mut w = crowd(guards, players);
        // Full budget: pre-index n, inspect n, test 3 players per soldier.
        assert_eq!(w.ai_tick(), (2 * n + guards * players) as u64);
        assert_eq!(w.ai_cursor, 0);
        // n (pre-index) + 3 (players inspected) + 10 soldiers at 1 + 3 each = n + 43: the
        // eleventh soldier (entity 13) is where the budget runs out.
        let spent = w.ai_tick_with((n + 43) as u64);
        assert_eq!(spent, (n + 43) as u64);
        assert_eq!(w.ai_cursor, 13);
        w.validate().unwrap();
        let h = w.hashes();
        let mut v = w.clone();
        v.ai_cursor = 0;
        assert_ne!(
            v.hashes().get("world"),
            h.get("world"),
            "the cursor is hashed"
        );
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = crowd(guards, players);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.ai_cursor, 13);
        assert_eq!(w2.hashes(), h);
        let mut bad = w.snapshot(None);
        bad.world.ai_cursor = n as u32;
        assert!(w2.restore(&bad).unwrap_err().contains("cursor"));
        // The next scan starts at 13: with the budget of the rest of the round (the 30
        // soldiers 13..=42 at 4 each) plus the pre-index pass and one more unit, it wraps,
        // inspects the first player character and stops on the second.
        let rest = (n + 30 * 4 + 1) as u64;
        assert_eq!(w.ai_tick_with(rest), rest);
        assert_eq!(w.ai_cursor, 1);
        // A budget too small for the pre-index pass perceives nothing and moves nothing.
        w.ai_cursor = 7;
        assert_eq!(w.ai_tick_with(5), 5);
        assert_eq!(w.ai_cursor, 7);
        // Same inputs from the restored world: same states and hashes.
        w2.ai_tick_with(rest);
        w2.ai_cursor = 7;
        w2.ai_tick_with(5);
        for _ in 0..3 {
            w.step(&[]);
            w2.step(&[]);
        }
        assert_eq!(w.hashes(), w2.hashes());
        assert_eq!(w.ai_cursor, 0, "a full tick completes the scan");
    }

    /// A mass alert: hundreds of soldiers hear the running hero at once and all want a path on
    /// the same tick. The path searches share the tick's budget: with the real budget every one
    /// of them is planned within the bound, with a small budget only as many as it pays for and
    /// the rest stand alerted and re-plan on the following ticks; both are deterministic across
    /// a snapshot.
    #[test]
    fn mass_alerts_share_one_navigation_budget() {
        let guards = 600;
        // Soldiers in a 40 x 15 block just behind the hero (facing away from him), who runs
        // east: all within the noise radius, none with him in their cone.
        let mut w = crowd(guards, 1);
        w.entities[0].x = f(500);
        w.entities[0].y = f(400);
        for (k, e) in w.entities.iter_mut().skip(1).enumerate() {
            e.x = f(420 + (k % 40) as i32 * 2);
            e.y = f(370 + (k / 40) as i32 * 4);
            e.facing256 = 128;
        }
        w.selected = Some(w.entities[0].id);
        w.validate().unwrap();
        click(&mut w, 700, 400, Button::Left);
        click(&mut w, 700, 400, Button::Left);
        assert_eq!(w.entities[0].gait, Gait::Run);
        // Everyone noticed him on the double click's tick; the alarm follows, then the run.
        assert!(
            w.entities
                .iter()
                .skip(1)
                .all(|e| e.ai_state == AiState::Noticed)
        );
        for _ in 0..NOTICED_TICKS + ALARM_TICKS - 1 {
            w.step(&[]);
        }
        assert!(
            w.entities
                .iter()
                .skip(1)
                .all(|e| e.ai_state == AiState::Alarm)
        );
        let snap = w.snapshot(None);
        // The transition tick with the real budget: every soldier is alerted and running, the
        // work stayed within the bound.
        let mut full = w.clone();
        let spent = full.ai_tick();
        assert!(spent <= AI_WORK_PER_TICK);
        assert!(
            full.entities
                .iter()
                .skip(1)
                .all(|e| e.ai_state == AiState::Alerted && e.target.is_some())
        );
        // A budget that covers the perception and a few searches: the rest stand alerted
        // without a path this tick and get theirs on the following ticks.
        let n = w.entities.len() as u64;
        let small = 2 * n + guards as u64 + 3000;
        let spent = w.ai_tick_with(small);
        assert!(spent <= small && spent > 2 * n + guards as u64);
        let planned = w
            .entities
            .iter()
            .skip(1)
            .filter(|e| e.target.is_some())
            .count();
        assert!(
            planned > 0 && planned < guards,
            "{planned} of {guards} planned"
        );
        assert!(
            w.entities
                .iter()
                .skip(1)
                .all(|e| e.ai_state == AiState::Alerted)
        );
        w.validate().unwrap();
        for _ in 0..3 {
            w.step(&[]);
        }
        assert!(
            w.entities.iter().skip(1).all(|e| e.target.is_some()),
            "the starved soldiers re-planned while the hero stays in earshot"
        );
        // Deterministic: the same small budget from the snapshot gives the same world.
        let mut w2 = crowd(0, 0);
        w2.restore(&snap).unwrap();
        w2.ai_tick_with(small);
        for _ in 0..3 {
            w2.step(&[]);
        }
        assert_eq!(w2.hashes(), w.hashes());
    }
}
