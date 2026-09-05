//! NPC perception, alert states, the knock-out and the melee: the stealth and combat layer of
//! `docs/original/stealth-and-combat.md` (section 6, items 1-4) and
//! `docs/original/combat-measurements.md`, as far as the first mission needs it. Everything
//! here is fixed point or integer, part of [`Entity`] (and so of the snapshot, of `validate`
//! and of the `actors` hash) and stepped by [`World::ai_tick`] from `World::simulate`, before
//! the waypoint programs run.
//!
//! Status of the constants below (`docs/original/stealth-and-combat.md` 8 and
//! `combat-measurements.md`, measured 2026-09-05): the noise channel is **measured** (a running
//! hero is heard at 330 px and more by soldiers not facing him, and they charge at once,
//! without the noticed / alarm pause), the timed states' durations come from the profiles'
//! tables on the measured animation clock (`anim.rs`); the melee's hit points, energy, damage,
//! cadence, fighting distance and the powerful blow's timing are **measured**; the view cone
//! (angle, range, the crouch divisor), the alert timeout, the knock-out timer, the punch's
//! arc, the hero's click attacks never landing ([`AttackRule::Block`]), the powerful blow's
//! chance and the soldier's cadence jitter ([`AttackRule::HitChance`]) and the soldier standing
//! his ground when his foe leaves ([`AttackRule::PostBound`]) remain **hypotheses** (section 7 and
//! `combat-measurements.md` 7 list the captures that would settle them). Every value is pinned
//! by tests so that a correction is a deliberate ruleset bump.
//!
//! Not modelled (documented gaps): occluders and walls do not block sight; civilians neither
//! perceive nor raise the alarm; walking makes no noise; soldiers do not start fights (only
//! the player's attack order does: a charging soldier who reaches the hero stands by him),
//! shoot, revive comrades or report to the script through `FilterAIEvent`; a soldier fights
//! one player character at a time (the others wait at reach: [`AttackRule::MultiParty`], the
//! measurements were one-on-one); a knock-out never
//! fails (the manual's chance is not modelled beyond the resistance threshold); knocked-down
//! and dead bodies do not move by the animation's displacement; the soldiers' occasional
//! 25-hp blow, the block and the other eight figures are not modelled; an empty energy bar
//! forbids nothing (its effect is not measured).
//!
//! Work. One simulation budget per tick (`world::SIM_WORK_PER_TICK`) pays for everything the
//! simulation does besides the script: the pre-index pass ([`SimIndex`], one unit per entity,
//! once), then each phase in turn on its own **quota** (`world::SIM_QUOTA_*`, deterministic
//! shares of the tick's budget; Codex review 9, finding 4): perception (one unit per soldier
//! inspected and one per (soldier, player character) pair tested), the state transitions (one
//! per human), the attack orders (one per attacker, the victim found by binary search in the
//! index) and, in `world.rs`, the waypoint programs (one per idle guard), the movement against
//! the obstacles and the geometry, the animation advance and the action-change scan; every
//! path search any phase issues (the alert run, the return, the attack approach, a program's
//! walk) draws from the phase's grant, capped per search at `world::SIM_SEARCH_WORK`, which
//! every quota exceeds: a search that fails with the full cap is unreachable under this
//! budget (a definite answer), one that fails with less changes nothing: the transition that
//! wanted it is not applied, the cursor stays on the entity and the next tick, where he
//! comes first, retries it with the full cap (Codex review 10, finding 4; a fight that ends
//! on an unpaid return enters [`AiState::ReturnPending`] and searches again next tick). A phase
//! never spends more than its quota plus what the phases before it left unused
//! ([`crate::world::SimBudget`]), so no phase can starve another whatever the snapshot holds.
//! Each phase walks its list from its own cursor (`World::cursors`, round robin: when its grant
//! runs out the cursor stays on the entity not finished and the next tick resumes there; when
//! it ran out on the first entity of the walk, the cursor moves past it so that one entity too
//! expensive for a whole quota cannot block the others; a completed walk resets it to 0), so
//! exhaustion is fair across ticks. The cursors are authoritative (snapshot, `validate`, the
//! `world` hash); the budget itself is granted afresh every tick and never stored.
//!
//! Taint. Every hypothesis of this layer records its [`Assumption`] where it first mutates
//! authoritative state, whether or not a script handler exists (Codex review 9, finding 1):
//! [`Assumption::SightCone`] when a sighting the rear radius ([`REAR_SIGHT_RADIUS`]) or the
//! crouch divisor decided changed a soldier's state (a standing character seen inside the
//! measured cone records nothing), [`Assumption::NoiseRadius`]
//! when a run was heard from beyond the measured bound ([`NOISE_MEASURED_RADIUS`]),
//! [`Assumption::AlertPolicy`] for the alert sequence a sighting starts and the re-plan,
//! [`Assumption::AlertTimeout`] for the alert timeout and the return to the post (the charge
//! on a heard run is measured and records nothing of its own; the timeout it stores is not),
//! [`Assumption::AttackPolicy`] for the reach bands, the block, the chances and the cadence
//! jitter, the post-bound soldier and the one-at-a-time rule of a soldier several player
//! characters attack ([`AttackRule::MultiParty`]), [`Assumption::KnockOut`] for the blow's effect and
//! [`Assumption::ProfileStats`] for the resistance it consults.

use serde::{Deserialize, Serialize};

use crate::anim::{AnimSet, direction_of, world_ticks};
use crate::fixed::Fixed;
use crate::vm::{Assumption, AttackRule, charge_budget};
use crate::world::{
    Entity, EntityId, EntityKind, Gait, Posture, SIM_QUOTA_ATTACKS, SIM_QUOTA_PERCEPTION,
    SIM_QUOTA_STATES, SIM_SEARCH_WORK, SimBudget, Team, World, facing_of,
};

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
    /// The fight (or the knock-out, the alert) ended but the search for the way back to the
    /// post was not paid by the tick's budget: stands where he is (140) and searches again
    /// next tick, then `Returning` (Codex review 10, finding 4: an unpaid return is retried,
    /// never turned into a patrol where he stands).
    ReturnPending,
    /// Delivers the knock-out blow (action 123), timed; player characters only.
    Punching,
    /// Knocked down (action 41 forward, 44 backward), timed.
    KnockedDown,
    /// Lying knocked out (47 / 48) until the knock-out timer runs out.
    Lying,
    /// Gets up (action 49), timed.
    GettingUp,
    /// Dead: lies for good (47 / 48) after the fall ([`AiState::Dying`]); the script
    /// predicates 85 / 87 / 90 see it.
    Dead,
    /// In a melee (`combat-measurements.md` 1): stands in the fighting stance (54) facing the
    /// foe (`Entity::foe`), strikes (59 / 75) and flinches (104) by `Entity::pose`; player
    /// characters and enemy soldiers.
    Fighting,
    /// Killed: the fall (44 backward when struck from the front, 41 forward) plays, timed;
    /// already not alive (the predicates report dead from the first tick).
    Dying,
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
            AiState::Fighting => 11,
            AiState::Dying => 12,
            AiState::ReturnPending => 13,
        }
    }

    /// Knocked out (down or lying) or dead (falling or lying for good): what script native 90
    /// reports as "out of action" (a soldier getting up is back in action; hypothesis).
    #[must_use]
    pub fn out_of_action(self) -> bool {
        matches!(
            self,
            AiState::KnockedDown | AiState::Lying | AiState::Dead | AiState::Dying
        )
    }

    /// Dead: the two states of a killed entity (`alive` is false exactly then).
    #[must_use]
    pub fn dead(self) -> bool {
        matches!(self, AiState::Dead | AiState::Dying)
    }

    /// On its feet: can perceive, walk, be ordered, be knocked out (script native 128, "able
    /// to act", also requires `alive` and `active`).
    #[must_use]
    pub fn standing(self) -> bool {
        !matches!(
            self,
            AiState::KnockedDown
                | AiState::Lying
                | AiState::GettingUp
                | AiState::Dead
                | AiState::Dying
        )
    }

    /// One of the alert states a soldier's perception drives.
    #[must_use]
    pub fn alert(self) -> bool {
        matches!(
            self,
            AiState::Noticed
                | AiState::Alarm
                | AiState::Alerted
                | AiState::Returning
                | AiState::ReturnPending
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
                | AiState::Dying
        )
    }
}

/// What a fighter is doing inside the `Fighting` state (`Entity::pose`): the stance, or a
/// timed pose (`Entity::pose_ticks`) whose end resolves the blow it carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FightPose {
    /// The fighting stance (action 54), between blows.
    #[default]
    Idle,
    /// An automatic strike (action 59): a soldier's basic hit (5 hp, two in three land), the
    /// hero's click attack (never lands against a soldier, [`AttackRule::Block`]).
    Strike,
    /// The hero's powerful blow of the forward-stroke figure (action 75): 50 hp when it
    /// lands, resolved [`POWERFUL_BLOW_TICKS`] after the order.
    PowerfulBlow,
    /// Hit in the stance (action 104).
    Flinch,
}

impl FightPose {
    /// Stable tag for canonical encodings (never derived from declaration order).
    #[must_use]
    pub fn tag(self) -> u8 {
        match self {
            FightPose::Idle => 1,
            FightPose::Strike => 2,
            FightPose::PowerfulBlow => 3,
            FightPose::Flinch => 4,
        }
    }
}

/// A drawn figure (the manual's mouse strokes; `combat-measurements.md` 1.4): the pending
/// order of a player character (`Entity::figure`), executed once he fights the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Figure {
    /// The stroke forwards (the pointer dragged right with the button held): the slow,
    /// powerful blow.
    ForwardStroke,
}

impl Figure {
    /// Stable tag for canonical encodings (never derived from declaration order).
    #[must_use]
    pub fn tag(self) -> u8 {
        match self {
            Figure::ForwardStroke => 1,
        }
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
        let dead = !e.alive || e.ai_state.dead();
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

/// Half opening angle of a soldier's view cone in 1/256 turns: 28 = 39.4 degrees, the measured
/// sector of about 80 degrees (half-angle 40) the Alt overlay draws for the archery sergeant
/// (`docs/original/h01-measurements-2.md` 6, one actor, confidence medium). The cone is bound
/// to the actor's facing: the archers facing north never noticed a hero 60..110 px behind
/// them over minutes (section 3).
pub const VIEW_CONE_HALF_ANGLE_256: i32 = 28;
/// Reach of the view cone in map pixels along the screen x axis: measured about 270 px
/// (`h01-measurements-2.md` 6). The boundary is an ellipse, not a circle: along y the reach is
/// [`VIEW_RANGE`] times [`VIEW_Y_COMPRESSION`] (196 px), so a y offset is weighted by 25 / 18
/// before the range test ([`in_view_cone`]).
pub const VIEW_RANGE: i32 = 270;
/// The view cone's y axis compressed to 0.72 of its x axis (18 / 25; measured: the top of the
/// sector at 196 px for a reach of 270 along x, `h01-measurements-2.md` 6). Whether the
/// compression is the game's distance metric or a projected drawing is not separated; the
/// engine applies it as the metric.
pub const VIEW_Y_COMPRESSION: (i32, i32) = (18, 25);
/// A crouched player character is seen at the range over this divisor (manual p. 16: crouching
/// characters are less visible; the factor is a hypothesis: a sighting of a crouched character
/// records [`Assumption::SightCone`]).
pub const CROUCH_VIEW_DIVISOR: i32 = 2;
/// Radius in map pixels within which a soldier notices a standing player character whatever
/// he faces (behind him included). Hypothesis from one event (`h01-measurements-2.md` 3: a
/// walking hero was noticed within 3 s at 40..60 px from behind an archer facing north, while
/// a hero 60..110 px behind the archers was not seen for minutes): either such a radius or
/// the archers' turns while shooting; the engine takes the radius, halved by
/// [`CROUCH_VIEW_DIVISOR`] for a crouched character, and records [`Assumption::SightCone`]
/// when it decides a sighting.
pub const REAR_SIGHT_RADIUS: i32 = 50;
/// Radius in map pixels within which a running player character is heard whatever the
/// soldier faces (manual p. 16: running makes a lot of noise; the console's `NOISE` shows such a
/// radius exists). Measured lower bound: soldiers not facing the hero detected his run from
/// at least 330 px ([`NOISE_MEASURED_RADIUS`]) and charged at once (`stealth-and-combat.md`
/// 8.6); the exact radius is not measured, 350 is the engine's choice above the bound, and a
/// run heard from beyond the bound records [`Assumption::NoiseRadius`]. Walking (nothing at
/// 290 px, measured) and sneaking make no noise.
pub const RUN_NOISE_RADIUS: i32 = 350;
/// The measured bound of the noise radius in map pixels: a run was detected from at least this
/// far (`stealth-and-combat.md` 8.6). Hearing within it is measured; hearing between it and
/// [`RUN_NOISE_RADIUS`] is the engine's hypothesis.
pub const NOISE_MEASURED_RADIUS: i32 = 330;
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
/// Fallback durations in world ticks of the timed states when the profile has no block for
/// the animation (or the world has no catalog): the soldier profiles' actions 141, 142, 41 and
/// 49 and the hero's 123 on the animation clock (`sprite-animations.md`, "Reading rules": a
/// frame lasts its tick half plus one table ticks of 3 clocks at 64 Hz; `anim::world_ticks`
/// converts): 141 = 5 frames of 6 ticks -> 11 table ticks -> 31 world ticks.
pub const NOTICED_TICKS: u32 = world_ticks(11);
/// See [`NOTICED_TICKS`]: 142 = 8 frames of 11 ticks -> 19 table ticks.
pub const ALARM_TICKS: u32 = world_ticks(19);
/// See [`NOTICED_TICKS`]: 41 = 8 frames of 13 ticks -> 21 table ticks.
pub const KNOCKED_DOWN_TICKS: u32 = world_ticks(21);
/// See [`NOTICED_TICKS`]: 49 = 8 frames of 16 ticks -> 24 table ticks.
pub const GET_UP_TICKS: u32 = world_ticks(24);
/// See [`NOTICED_TICKS`]: 123 = 8 frames of 11 ticks (the hero) -> 19 table ticks.
pub const PUNCH_TICKS: u32 = world_ticks(19);
/// Hit points of the hero: 100, measured (`combat-measurements.md` 1.2: a 20 px bar at 5 hp
/// per pixel, 36 hits of one pixel).
pub const HERO_HIT_POINTS: i32 = 100;
/// Hit points of a human whose profile gives none (player characters, civilians): the hero's
/// measured value; the profile table's PC and CV records have no field read as hit points
/// (`profile.md`), so the other heroes and the civilians share it (hypothesis for them). A
/// soldier's hit points are his SD record's `pre[0]` (80 for a blue halberdier, confirmed by
/// the powerful blow's 50 hp taking 13 of his 20 pixels; `combat-measurements.md` 1.2).
pub const DEFAULT_HIT_POINTS: i32 = HERO_HIT_POINTS;
/// Energy of every fighter: 20 units, one per pixel of the blue bar (measured, 1.2).
pub const ENERGY_MAX: i32 = 20;
/// Ticks the hero needs to regain one unit of energy: 0.9 s (measured 0.8-1.0 s per pixel,
/// 1.2), 54 ticks at 60 Hz.
pub const HERO_ENERGY_REGEN_TICKS: u32 = 54;
/// Ticks a soldier needs to regain the unit a landed hit cost him: about 4 s (measured, 1.2).
pub const SOLDIER_ENERGY_REGEN_TICKS: u32 = 240;
/// Energy a soldier's landed hit costs him (measured: one pixel, 1.2).
pub const SOLDIER_HIT_ENERGY: i32 = 1;
/// Energy the hero's powerful blow costs, landed or not (measured: two pixels, 1.2 / 1.4).
pub const POWERFUL_BLOW_ENERGY: i32 = 2;
/// Fighting distance between the feet in map pixels (measured: 52 px, 1.6): the attacker
/// stops here and the fight begins.
pub const FIGHT_RANGE: i32 = 52;
/// Distance beyond which a fight breaks off (a fighter moved away by a script walk; the
/// player's own orders end his fight directly): twice the fighting distance.
pub const FIGHT_BREAK_RANGE: i32 = 2 * FIGHT_RANGE;
/// Ticks between a soldier's swings: about 5.3 s (measured: 12 swings in 64 s, 1.5), 318
/// ticks; the gameplay RNG adds a jitter of up to [`SWING_JITTER_TICKS`] either way.
pub const SOLDIER_SWING_TICKS: u32 = 318;
/// Half width of the uniform jitter on a soldier's swing interval (about half a second; the
/// measured intervals between landed hits spread 5.2..15.4 s around the 7.7 s median, which
/// the cadence with two of three swings landing reproduces). The engine's choice within the
/// measurement, not a rule of the original.
pub const SWING_JITTER_TICKS: u32 = 32;
/// Chance of a soldier's swing landing as `(numerator, denominator)`: two in three
/// (measured: median 7.7 s between landed hits over a swing every 5.3 s, 1.5).
pub const SOLDIER_HIT_CHANCE: (u32, u32) = (2, 3);
/// Damage of a soldier's landed basic hit (measured: one pixel of the hero's bar, 5 hp, 36
/// times; the occasional 25-hp blow is not modelled).
pub const SOLDIER_HIT_DAMAGE: i32 = 5;
/// Damage of the hero's powerful blow when it lands (measured: "50", 13 pixels of 80 hp, 1.4).
pub const POWERFUL_BLOW_DAMAGE: i32 = 50;
/// Ticks from the figure's order to the blow's resolution: 0.9-1.0 s measured (1.4), 57 ticks.
pub const POWERFUL_BLOW_TICKS: u32 = 57;
/// Chance of the hero's powerful blow landing on a soldier as `(numerator, denominator)`: one
/// in three, from 2 of 6 strokes against a halberdier (1.4; small sample). Hypothesis:
/// resolving a blow records [`AttackRule::HitChance`].
pub const POWERFUL_BLOW_CHANCE: (u32, u32) = (1, 3);
/// Ticks between the hero's automatic strikes while fighting (1.5 s): presentation only,
/// since a click attack never lands against a soldier ([`AttackRule::Block`]; observed
/// against a pole arm at 52 px over 225 s, 1.3); the interval itself is not measured.
pub const HERO_SWING_TICKS: u32 = 90;
/// Fallback duration of a quick strike (actions 59..66: 8 tick halves over 8 frames = 16
/// table ticks on `Soldier A00`, `sprite-animations.md`).
pub const STRIKE_TICKS: u32 = world_ticks(16);
/// Fallback duration of the flinch in the stance (action 104; not read: 12 table ticks).
pub const FLINCH_TICKS: u32 = world_ticks(12);
/// Ticks a damage number rises over the victim's head before it vanishes (measured: about
/// 1.5 s for 50 px, 1.2).
pub const DAMAGE_NUMBER_TICKS: u32 = 90;
/// Pixels a damage number rises in [`DAMAGE_NUMBER_TICKS`] (measured, 1.2).
pub const DAMAGE_NUMBER_RISE: i32 = 50;
/// Shortest pointer stroke (map pixels between the press and the release of the left button)
/// read as a drawn figure rather than a click; the measured stroke was 80 px (1.4). The
/// engine's choice.
pub const FIGURE_MIN_STROKE: i32 = 32;

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
    /// Fight idle (the stance).
    pub const FIGHT_IDLE: u32 = 54;
    /// A quick strike.
    pub const STRIKE: u32 = 59;
    /// The powerful blow (the over-the-head finishing blow).
    pub const POWERFUL_BLOW: u32 = 75;
    /// Hit in the stance.
    pub const FLINCH: u32 = 104;
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
/// `facing256` (0 = +x, clockwise on screen), with the given reach along x in map pixels and
/// half angle in 1/256 turns. The reach is elliptical ([`VIEW_Y_COMPRESSION`]: the y offset
/// weighs 25 / 18 of the x offset, so a reach of 270 along x is 196 along y, as measured);
/// the angle is tested on the screen offsets as measured. The observer's own position counts
/// as inside. All arithmetic in `i64`.
#[must_use]
pub fn in_view_cone(
    (ox, oy): (Fixed, Fixed),
    facing256: i32,
    (px, py): (Fixed, Fixed),
    range_px: i32,
    half_angle256: i32,
) -> bool {
    let (dx, dy) = (px - ox, py - oy);
    if !within_elliptical_reach(dx, dy, range_px) {
        return false;
    }
    let len = i64::from(Fixed::length(dx, dy).raw());
    if len == 0 {
        return true;
    }
    let dot = i64::from(cos256(facing256)) * i64::from(dx.raw())
        + i64::from(sin256(facing256)) * i64::from(dy.raw());
    dot >= len * i64::from(cos256(half_angle256))
}

/// Whether the offset `(dx, dy)` lies within the sight's elliptical reach: `range_px` along x,
/// `range_px` times [`VIEW_Y_COMPRESSION`] along y (`dx^2 * 18^2 + dy^2 * 25^2 <= (range *
/// 18)^2` on the raw 24.8 values, in `i128`; an offset beyond either semi-axis is rejected
/// first).
#[must_use]
pub fn within_elliptical_reach(dx: Fixed, dy: Fixed, range_px: i32) -> bool {
    let (num, den) = (
        i64::from(VIEW_Y_COMPRESSION.0),
        i64::from(VIEW_Y_COMPRESSION.1),
    );
    let range = i64::from(range_px.max(0)) * 256;
    let (dx, dy) = (i64::from(dx.raw()).abs(), i64::from(dy.raw()).abs());
    if dx > range || dy * den > range * num {
        return false;
    }
    let (dx, dy, num, den, range) = (
        i128::from(dx),
        i128::from(dy),
        i128::from(num),
        i128::from(den),
        i128::from(range),
    );
    dx * dx * num * num + dy * dy * den * den <= range * range * num * num
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
        ALARM, ALERT_IDLE, ALERT_RUN, ALERT_WALK, CROUCH_IDLE, FIGHT_IDLE, FLINCH, GET_UP, IDLE,
        KNOCKED_DOWN, KNOCKED_DOWN_BACK, LYING, LYING_BACK, NOTICED, POWERFUL_BLOW, PUNCH, RUN,
        SNEAK, STRIKE, WALK,
    };
    let moving = e.target.is_some();
    match e.ai_state {
        AiState::Fighting => match e.pose {
            FightPose::Idle => FIGHT_IDLE,
            FightPose::Strike => STRIKE,
            FightPose::PowerfulBlow => POWERFUL_BLOW,
            FightPose::Flinch => FLINCH,
        },
        AiState::Patrol => match (e.posture, moving, e.gait) {
            (Posture::Crouched, true, _) => SNEAK,
            (Posture::Crouched, false, _) => CROUCH_IDLE,
            (Posture::Standing, true, Gait::Run) => RUN,
            (Posture::Standing, true, Gait::Walk) => WALK,
            (Posture::Standing, false, _) => IDLE,
        },
        AiState::Noticed => NOTICED,
        AiState::Alarm => ALARM,
        AiState::Alerted | AiState::Returning | AiState::ReturnPending => match (moving, e.gait) {
            (true, Gait::Run) => ALERT_RUN,
            (true, Gait::Walk) => ALERT_WALK,
            (false, _) => ALERT_IDLE,
        },
        AiState::Punching => PUNCH,
        AiState::KnockedDown | AiState::Dying => {
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
        AiState::Fighting => match e.pose {
            FightPose::Idle => set.fight_idle[dir],
            FightPose::Strike => set.strike[dir],
            FightPose::PowerfulBlow => set.powerful_blow[dir],
            FightPose::Flinch => set.flinch[dir],
        },
        AiState::Patrol => match (e.posture, moving, e.gait) {
            (Posture::Crouched, true, _) => set.crouch_walk[dir],
            (Posture::Crouched, false, _) => set.crouch_idle[dir],
            (Posture::Standing, true, Gait::Run) => set.run[dir],
            (Posture::Standing, true, Gait::Walk) => set.walk[dir],
            (Posture::Standing, false, _) => set.idle[dir],
        },
        AiState::Noticed => set.noticed[dir],
        AiState::Alarm => set.alarm[dir],
        AiState::Alerted | AiState::Returning | AiState::ReturnPending => match (moving, e.gait) {
            (true, Gait::Run) => set.alert_run[dir],
            (true, Gait::Walk) => set.alert_walk[dir],
            (false, _) => set.alert_idle[dir],
        },
        AiState::Punching => set.punch[dir],
        AiState::KnockedDown | AiState::Dying => {
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

/// A timed state's duration in world ticks: one loop of the animation it plays for this entity
/// (the block of the entity's profile facing its direction) on the animation clock
/// (`AnimSet::world_ticks`), else `fallback`.
fn state_ticks(world: &World, e: &Entity, block: fn(&AnimSet) -> &[u32; 8], fallback: u32) -> u32 {
    let ticks = e
        .anim
        .as_ref()
        .and_then(|a| world.catalog.sets.get(&a.set))
        .and_then(|set| set.world_ticks(block(set)[direction_of(e.facing256)]));
    ticks.unwrap_or(fallback).max(1)
}

/// How a soldier perceived a player character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Channel {
    /// Seen: inside the view cone (measured geometry, `h01-measurements-2.md` 6: records
    /// nothing for a standing character) or within the rear radius ([`REAR_SIGHT_RADIUS`]);
    /// `hypothetical` marks a sighting the engine's hypotheses decided (the rear radius, or
    /// the crouch divisor for a crouched character), recorded as [`Assumption::SightCone`].
    Sight {
        /// Decided by the rear radius or the crouch divisor rather than the measured cone.
        hypothetical: bool,
    },
    /// A running character within the noise radius (measured up to [`NOISE_MEASURED_RADIUS`],
    /// `stealth-and-combat.md` 8.6; `beyond_measured` marks a run heard from farther, the
    /// engine's hypothesis).
    Noise {
        /// Heard from beyond the measured bound.
        beyond_measured: bool,
    },
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
        && !matches!(s.ai_state, AiState::Punching | AiState::Fighting)
}

/// A human whose state machine runs: alive (or falling dead: the fall is timed), active, not
/// an obstacle, not locked.
fn stepped(e: &Entity) -> bool {
    (e.alive || e.ai_state == AiState::Dying)
        && e.active
        && e.kind != EntityKind::Obstacle
        && !e.ai_locked
}

/// An enemy soldier a player character can fight: alive, present, on his feet.
pub(crate) fn fightable(v: &Entity) -> bool {
    v.alive
        && v.active
        && v.kind == EntityKind::Guard
        && v.team == Team::Enemy
        && v.ai_state.standing()
}

/// Whether (and how) soldier `s` sees or hears player character `p` this tick: the view cone
/// (measured for a standing character; the crouch divisor is a hypothesis), the rear radius
/// (a hypothesis), then the noise of a run.
fn stimulus(s: &Entity, p: &Entity) -> Option<Channel> {
    let crouched = p.posture == Posture::Crouched;
    let divisor = if crouched { CROUCH_VIEW_DIVISOR } else { 1 };
    if in_view_cone(
        (s.x, s.y),
        s.facing256,
        (p.x, p.y),
        VIEW_RANGE / divisor,
        VIEW_CONE_HALF_ANGLE_256,
    ) {
        return Some(Channel::Sight {
            hypothetical: crouched,
        });
    }
    let distance = Fixed::length(p.x - s.x, p.y - s.y);
    if distance <= Fixed::from_int(REAR_SIGHT_RADIUS / divisor) {
        return Some(Channel::Sight { hypothetical: true });
    }
    let noisy = p.target.is_some() && p.gait == Gait::Run && p.posture == Posture::Standing;
    (noisy && distance <= Fixed::from_int(RUN_NOISE_RADIUS)).then_some(Channel::Noise {
        beyond_measured: distance > Fixed::from_int(NOISE_MEASURED_RADIUS),
    })
}

/// The entity lists the phases of one tick walk, built once per tick by [`World::sim_index`]
/// (one work unit per entity). Every phase walks its own list from its cursor instead of the
/// whole table, so its work is proportional to the entities it concerns; the conditions are
/// re-checked per entity when the phase runs, since an earlier phase may have changed them.
pub(crate) struct SimIndex {
    /// Player characters a soldier can perceive.
    pub players: Vec<usize>,
    /// The first player character in slot order (the synthetic objective's runner).
    pub first_player: Option<usize>,
    /// Soldiers whose perception runs.
    pub perceivers: Vec<usize>,
    /// Humans whose state machine runs (alive, active, not an obstacle, not locked).
    pub humans: Vec<usize>,
    /// Player characters with an attack order.
    pub attackers: Vec<usize>,
    /// Guards in the normal state without a walk (their program or patrol may issue one).
    pub idle_guards: Vec<usize>,
    /// Living, active non-obstacles: the movement phase's list (a walk issued by an earlier
    /// phase of the same tick is stepped this tick, so the list is not filtered by `target`).
    pub actors: Vec<usize>,
    /// Active entities: the animation phase's list.
    pub active: Vec<usize>,
    /// Active non-obstacles: the action-change scan's list.
    pub present: Vec<usize>,
    /// The obstacles' boxes `(x, y, half width, half height)` in slot order: the key of the
    /// obstacle index (`World::obstacles`), rebuilt when it differs.
    pub obstacles: Vec<(Fixed, Fixed, Fixed, Fixed)>,
    /// `(id, index)` of every entity, sorted by id: the attack target and foe lookup.
    pub by_id: Vec<(EntityId, usize)>,
}

impl SimIndex {
    /// The slot of an entity by id (binary search in `by_id`).
    fn slot(&self, id: EntityId) -> Option<usize> {
        self.by_id
            .binary_search_by_key(&id, |&(id, _)| id)
            .ok()
            .map(|k| self.by_id[k].1)
    }
}

/// What a walk order came to ([`World::walk_to`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Walk {
    /// A path was found; the entity walks.
    Planned,
    /// No path exists (or the grid is missing): a definite answer.
    Unreachable,
    /// The budget could not pay the search: nothing changed (the walk the entity had is kept),
    /// the transition that wanted it is not applied and the next tick retries it first.
    Exhausted,
}

/// `list` (ascending entity indices) rotated so that the walk starts at the first entity at or
/// after `cursor` and wraps around: the round robin of a phase.
pub(crate) fn rotated(list: &[usize], cursor: u32) -> Vec<usize> {
    let start = list.partition_point(|&i| i < cursor as usize);
    list[start..]
        .iter()
        .chain(&list[..start])
        .copied()
        .collect()
}

/// Where a phase's walk over `order` resumes next tick after its grant ran out on the `k`-th
/// entity: on that entity, unless it was the first of the walk and the phase had its whole
/// quota (`full`), in which case the walk moves past it (an entity too expensive for a whole
/// quota must not block every other one; it is served again once the round comes back to
/// it). A first entity that failed on a partial grant is not at fault and keeps its turn.
pub(crate) fn resume_at(order: &[usize], k: usize, full: bool) -> u32 {
    let at = if k == 0 && full {
        order.get(1).copied().unwrap_or(0)
    } else {
        order[k]
    };
    at as u32
}

impl World {
    /// One tick of the stealth layer with the full simulation budget (tests; `simulate` runs
    /// [`World::sim_index`], [`World::stealth_tick`] and the phases of `world.rs` on one
    /// budget). Returns the work units spent of `SIM_WORK_PER_TICK`.
    #[cfg(test)]
    pub(crate) fn ai_tick(&mut self) -> u64 {
        self.ai_tick_with(crate::world::SIM_WORK_PER_TICK)
    }

    /// [`World::ai_tick`] with an explicit budget (tests exercise the exhaustion paths without
    /// a `MAX_ENTITIES` world); returns the units spent.
    #[cfg(test)]
    pub(crate) fn ai_tick_with(&mut self, budget: u64) -> u64 {
        let mut left = budget;
        if let Some(index) = self.sim_index(&mut left) {
            let mut sim = SimBudget::new(left);
            self.stealth_tick(&index, &mut sim);
            left = sim.left();
        }
        budget - left
    }

    /// The pre-index pass: one unit per entity, charged before the pass; `None` (nothing
    /// indexed, the cursors untouched) when it does not fit.
    pub(crate) fn sim_index(&mut self, left: &mut u64) -> Option<SimIndex> {
        let n = self.entities.len();
        if !charge_budget(left, n as u64) {
            return None;
        }
        let mut index = SimIndex {
            players: Vec::new(),
            first_player: None,
            perceivers: Vec::new(),
            humans: Vec::new(),
            attackers: Vec::new(),
            idle_guards: Vec::new(),
            actors: Vec::new(),
            active: Vec::new(),
            present: Vec::new(),
            obstacles: Vec::new(),
            by_id: Vec::with_capacity(n),
        };
        for (i, e) in self.entities.iter().enumerate() {
            index.by_id.push((e.id, i));
            if e.kind == EntityKind::Player && index.first_player.is_none() {
                index.first_player = Some(i);
            }
            if e.kind == EntityKind::Obstacle
                && let Some(&(hw, hh)) = e.patrol.first()
            {
                index.obstacles.push((e.x, e.y, hw, hh));
            }
            if e.active {
                index.active.push(i);
                if e.kind != EntityKind::Obstacle {
                    index.present.push(i);
                    if e.alive {
                        index.actors.push(i);
                    }
                }
            }
            if perceivable(e) {
                index.players.push(i);
            }
            if perceives(e) {
                index.perceivers.push(i);
            }
            if stepped(e) {
                index.humans.push(i);
            }
            if e.attack_target.is_some() && e.kind == EntityKind::Player {
                index.attackers.push(i);
            }
            if e.alive
                && e.active
                && e.kind == EntityKind::Guard
                && e.target.is_none()
                && e.ai_state == AiState::Patrol
            {
                index.idle_guards.push(i);
            }
        }
        index.by_id.sort_unstable();
        Some(index)
    }

    /// The stealth phases of one tick, each on its quota: every soldier's perception and alert
    /// state and the timed states of every human, then the player characters' attack orders
    /// (so a state a blow enters lasts its full duration from the next tick on). Runs before
    /// the waypoint programs (which only Patrol-state guards execute) and before the movement.
    pub(crate) fn stealth_tick(&mut self, index: &SimIndex, sim: &mut SimBudget) {
        let mut grant = sim.grant(SIM_QUOTA_PERCEPTION);
        let stimuli = self.perception(index, &mut grant);
        sim.settle(grant);
        let mut grant = sim.grant(SIM_QUOTA_STATES);
        self.transitions(index, &stimuli, &mut grant);
        sim.settle(grant);
        let mut grant = sim.grant(SIM_QUOTA_ATTACKS);
        self.attack_orders(index, &mut grant);
        sim.settle(grant);
    }

    /// The position of the first player character (in slot order) each soldier perceives this
    /// tick, with the channel. The soldiers are inspected from `cursors.perception` on, round
    /// robin (one unit each, plus one per player character tested); when the grant runs out
    /// the cursor stays on the soldier not finished, who perceives nothing this tick, and the
    /// next tick resumes there ([`resume_at`]); a completed walk resets the cursor to 0. A
    /// soldier's inspection costs at most `MAX_ENTITIES + 1` units, less than the perception
    /// quota, so a soldier the cursor rests on is always finished on the next tick.
    fn perception(
        &mut self,
        index: &SimIndex,
        budget: &mut u64,
    ) -> Vec<Option<((Fixed, Fixed), Channel)>> {
        let mut out = vec![None; self.entities.len()];
        let mut cursor = 0u32;
        let full = *budget >= SIM_QUOTA_PERCEPTION;
        let order = rotated(&index.perceivers, self.cursors.perception);
        'scan: for (k, &i) in order.iter().enumerate() {
            if !charge_budget(budget, 1) {
                cursor = resume_at(&order, k, full);
                break 'scan;
            }
            let s = &self.entities[i];
            if !perceives(s) {
                continue;
            }
            for &pi in &index.players {
                if !charge_budget(budget, 1) {
                    cursor = resume_at(&order, k, full);
                    break 'scan;
                }
                let p = &self.entities[pi];
                if let Some(channel) = stimulus(s, p) {
                    out[i] = Some(((p.x, p.y), channel));
                    break;
                }
            }
        }
        self.cursors.perception = cursor;
        out
    }

    /// The state machine of every human (one unit each) from `cursors.states`, round robin;
    /// a human the grant does not reach keeps its state and timer for this tick, and so does
    /// one whose transition needs a path search the grant could not pay
    /// ([`Walk::Exhausted`]): the walk stops on him with the cursor resting there, so the next
    /// tick, where he comes first, retries the transition with the full search cap (Codex
    /// review 10, finding 4: a transition and the path it plans are one step).
    fn transitions(
        &mut self,
        index: &SimIndex,
        stimuli: &[Option<((Fixed, Fixed), Channel)>],
        budget: &mut u64,
    ) {
        let mut cursor = 0u32;
        let full = *budget >= SIM_QUOTA_STATES;
        let order = rotated(&index.humans, self.cursors.states);
        for (k, &i) in order.iter().enumerate() {
            if !charge_budget(budget, 1) {
                cursor = resume_at(&order, k, full);
                break;
            }
            if !stepped(&self.entities[i]) {
                continue;
            }
            if !self.advance_state(i, index, stimuli.get(i).copied().flatten(), budget) {
                cursor = resume_at(&order, k, full);
                break;
            }
        }
        self.cursors.states = cursor;
    }

    /// Order the entity to walk to a point at the given gait; the search draws from the
    /// phase's grant, capped per search at [`SIM_SEARCH_WORK`]. A search that fails with the
    /// full cap granted is unreachable under this budget, a definite answer
    /// ([`Walk::Unreachable`]: the entity stands); one that fails with less (the grant was
    /// nearly spent) leaves the entity exactly as it was, the walk it had included, so that
    /// the next tick, where it comes first, retries with the full cap ([`Walk::Exhausted`]).
    fn walk_to(&mut self, i: usize, to: (Fixed, Fixed), gait: Gait, budget: &mut u64) -> Walk {
        let granted = (*budget).min(SIM_SEARCH_WORK);
        let mut search = granted;
        let e = &mut self.entities[i];
        let kept = (e.target, std::mem::take(&mut e.path), e.gait);
        let planned = self.plan_path_with(i, to, &mut search);
        *budget -= granted - search;
        if planned == Err(crate::nav::NavError::WorkExhausted) {
            if granted >= SIM_SEARCH_WORK {
                return Walk::Unreachable;
            }
            let e = &mut self.entities[i];
            (e.target, e.path, e.gait) = kept;
            return Walk::Exhausted;
        }
        let e = &mut self.entities[i];
        if e.target.is_some() {
            e.gait = gait;
            Walk::Planned
        } else {
            Walk::Unreachable
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
        e.heard = false;
        e.last_seen = None;
        e.alert_origin = None;
    }

    /// Walk back to the alert origin (`Returning`), or patrol at once when there is none, the
    /// entity already stands there or the way back cannot be found. `false` when the budget
    /// could not pay the search: nothing changed, the caller keeps its state and the
    /// transition is retried next tick.
    fn return_to_post(&mut self, i: usize, budget: &mut u64) -> bool {
        let e = &self.entities[i];
        let here = (e.x, e.y);
        match e.alert_origin {
            Some(origin) if origin != here => match self.walk_to(i, origin, Gait::Walk, budget) {
                Walk::Planned => {
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::Returning;
                    e.state_ticks = 0;
                    true
                }
                Walk::Unreachable => {
                    self.resume_patrol(i);
                    true
                }
                Walk::Exhausted => false,
            },
            _ => {
                self.resume_patrol(i);
                true
            }
        }
    }

    /// A timed state (the alert's timeout, the getting up) ended and the soldier goes back to
    /// his post ([`World::return_to_post`]). The return is part of the alert policy
    /// (hypothesis): it records [`Assumption::AlertTimeout`] before the state changes. When
    /// the budget cannot pay the search, the timed state keeps one more tick and the
    /// transition is retried next tick, first in the walk (`false`).
    fn go_back(&mut self, i: usize, budget: &mut u64) -> bool {
        self.record_assumption(Assumption::AlertTimeout);
        if self.return_to_post(i, budget) {
            true
        } else {
            self.entities[i].state_ticks = 1;
            false
        }
    }

    /// A sighting reaches a soldier in a normal state: he notices it (141), then raises the
    /// alarm (142) and searches. The sequence is a hypothesis (the cone was not measured):
    /// it records [`Assumption::AlertPolicy`] (the caller recorded the channel's source).
    fn notice(&mut self, i: usize, seen: (Fixed, Fixed)) {
        self.record_assumption(Assumption::AlertPolicy);
        let e = &mut self.entities[i];
        if e.ai_state == AiState::Patrol {
            e.alert_origin = Some((e.x, e.y));
        }
        e.last_seen = Some(seen);
        e.heard = false;
        stop(e);
        self.enter_timed(i, AiState::Noticed, |s| &s.noticed, NOTICED_TICKS);
    }

    /// A running character is heard by a soldier in a normal state: he charges at once
    /// (`Alerted`, the alert run 151 to the position of the noise) without the noticed / alarm
    /// pause (measured: `stealth-and-combat.md` 8.6, soldiers converged within 1.5 s of the
    /// first running frames, no reaction animation seen). `heard` marks the alert as the
    /// measured channel so its action ids record no perception assumption. The charge itself
    /// records nothing; the alert timeout and the return destination it stores are the alert
    /// policy ([`Assumption::AlertTimeout`], recorded before the state changes; Codex review
    /// 10, finding 1). The search comes first: when the budget cannot pay it nothing changes
    /// and the transition is retried next tick (`false`); a noise he cannot reach still alerts
    /// him where he stands.
    fn charge(&mut self, i: usize, heard_at: (Fixed, Fixed), budget: &mut u64) -> bool {
        self.record_assumption(Assumption::AlertTimeout);
        let walk = self.walk_to(i, heard_at, Gait::Run, budget);
        if walk == Walk::Exhausted {
            return false;
        }
        let e = &mut self.entities[i];
        if e.ai_state == AiState::Patrol {
            e.alert_origin = Some((e.x, e.y));
        }
        e.last_seen = Some(heard_at);
        e.heard = true;
        e.ai_state = AiState::Alerted;
        e.state_ticks = ALERT_TIMEOUT_TICKS;
        if walk == Walk::Unreachable {
            stop(e);
        }
        true
    }

    /// The state machine of one human for this tick (the energy regains its units first, in
    /// every state). A stimulus a perceiving state consumes records its channel's source
    /// where it changes the state: a sighting the rear radius or the crouch divisor decided
    /// ([`Assumption::SightCone`]) or a run heard from beyond the measured bound
    /// ([`Assumption::NoiseRadius`]); a sighting inside the measured cone of a standing
    /// character and a run heard within the bound record nothing. `false` when the transition needed a path
    /// search the budget could not pay: nothing changed (a timed state that ended keeps one
    /// more tick) and the caller retries next tick with the cursor on this entity.
    fn advance_state(
        &mut self,
        i: usize,
        index: &SimIndex,
        stimulus: Option<((Fixed, Fixed), Channel)>,
        budget: &mut u64,
    ) -> bool {
        let seen = stimulus.map(|(p, _)| p);
        self.regain_energy(i);
        let state = self.entities[i].ai_state;
        if matches!(
            state,
            AiState::Patrol
                | AiState::Returning
                | AiState::ReturnPending
                | AiState::Noticed
                | AiState::Alarm
                | AiState::Alerted
        ) {
            match stimulus {
                Some((_, Channel::Sight { hypothetical: true })) => {
                    self.record_assumption(Assumption::SightCone);
                }
                Some((
                    _,
                    Channel::Noise {
                        beyond_measured: true,
                    },
                )) => self.record_assumption(Assumption::NoiseRadius),
                _ => {}
            }
        }
        match state {
            AiState::Fighting => self.fight_tick(i, index, budget),
            AiState::Dying => {
                if countdown(&mut self.entities[i]) {
                    self.entities[i].ai_state = AiState::Dead;
                }
                true
            }
            AiState::Patrol | AiState::Returning | AiState::ReturnPending => match stimulus {
                Some((p, Channel::Sight { .. })) => {
                    self.notice(i, p);
                    true
                }
                Some((p, Channel::Noise { .. })) => self.charge(i, p, budget),
                None => match state {
                    AiState::Returning if self.entities[i].target.is_none() => {
                        self.resume_patrol(i);
                        true
                    }
                    // The return whose search was not paid: searched again, first in the
                    // walk when it stays unpaid.
                    AiState::ReturnPending => self.return_to_post(i, budget),
                    _ => true,
                },
            },
            AiState::Noticed => {
                if seen.is_some() {
                    self.entities[i].last_seen = seen;
                }
                if countdown(&mut self.entities[i]) {
                    self.record_assumption(Assumption::AlertPolicy);
                    self.enter_timed(i, AiState::Alarm, |s| &s.alarm, ALARM_TICKS);
                }
                true
            }
            AiState::Alarm => {
                if seen.is_some() {
                    self.entities[i].last_seen = seen;
                }
                if countdown(&mut self.entities[i]) {
                    // The search, then the state: the sequence is the alert policy, the
                    // timer it starts the timeout; unpaid, the alarm keeps one more tick.
                    self.record_assumption(Assumption::AlertPolicy);
                    self.record_assumption(Assumption::AlertTimeout);
                    let walk = match self.entities[i].last_seen {
                        Some(p) => self.walk_to(i, p, Gait::Run, budget),
                        None => Walk::Unreachable,
                    };
                    if walk == Walk::Exhausted {
                        self.entities[i].state_ticks = 1;
                        return false;
                    }
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::Alerted;
                    e.state_ticks = ALERT_TIMEOUT_TICKS;
                }
                true
            }
            AiState::Alerted => match seen {
                Some(p) => {
                    let e = &self.entities[i];
                    let stale = e.target.is_none_or(|(tx, ty)| {
                        Fixed::length(tx - p.0, ty - p.1) > Fixed::from_int(REPLAN_DISTANCE)
                    });
                    if stale && Fixed::length(e.x - p.0, e.y - p.1) > Fixed::from_int(PUNCH_REACH) {
                        // The re-plan distance and the search's target are the alert policy;
                        // unpaid, nothing changes and the sighting is consumed again next tick.
                        self.record_assumption(Assumption::AlertPolicy);
                        if self.walk_to(i, p, Gait::Run, budget) == Walk::Exhausted {
                            return false;
                        }
                    }
                    // The sighting restarts the timeout.
                    self.record_assumption(Assumption::AlertTimeout);
                    let e = &mut self.entities[i];
                    e.last_seen = Some(p);
                    e.state_ticks = ALERT_TIMEOUT_TICKS;
                    true
                }
                None => {
                    if countdown(&mut self.entities[i]) {
                        self.go_back(i, budget)
                    } else {
                        true
                    }
                }
            },
            AiState::Punching => {
                if countdown(&mut self.entities[i]) {
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::Patrol;
                }
                true
            }
            AiState::KnockedDown => {
                if countdown(&mut self.entities[i]) {
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::Lying;
                    e.state_ticks = knock_out_ticks(e.knockout_resistance).unwrap_or(1);
                }
                true
            }
            AiState::Lying => {
                if countdown(&mut self.entities[i]) {
                    self.enter_timed(i, AiState::GettingUp, |s| &s.get_up, GET_UP_TICKS);
                }
                true
            }
            AiState::GettingUp => {
                if countdown(&mut self.entities[i]) {
                    self.go_back(i, budget)
                } else {
                    true
                }
            }
            AiState::Dead => true,
        }
    }

    /// The player characters' attack orders (`combat-measurements.md` 1.1, measured: a left
    /// click on the enemy is the attack order, the character walks up, stops at the fighting
    /// distance and the sword fight begins; the knock-out blow from behind of
    /// `stealth-and-combat.md` 1 is kept for an unseen approach, a hypothesis: an order that
    /// resolves with the victim's back to the attacker records
    /// [`AttackRule::Reach`]). One unit per attacker from `cursors.attacks`, round robin; the
    /// victim is looked up in the index, never by a scan.
    fn attack_orders(&mut self, index: &SimIndex, budget: &mut u64) {
        let mut cursor = 0u32;
        let full = *budget >= SIM_QUOTA_ATTACKS;
        let order = rotated(&index.attackers, self.cursors.attacks);
        for (k, &i) in order.iter().enumerate() {
            if !charge_budget(budget, 1) {
                cursor = resume_at(&order, k, full);
                break;
            }
            let e = &self.entities[i];
            let Some(target) = e.attack_target else {
                continue;
            };
            if !(e.alive && e.active && e.kind == EntityKind::Player)
                || e.ai_state != AiState::Patrol
            {
                continue;
            }
            let victim = index.slot(target);
            let Some(t) = victim.filter(|&t| fightable(&self.entities[t])) else {
                let e = &mut self.entities[i];
                e.attack_target = None;
                e.figure = None;
                continue;
            };
            let (vx, vy) = (self.entities[t].x, self.entities[t].y);
            let attacker_pos = (self.entities[i].x, self.entities[i].y);
            let (dx, dy) = (vx - attacker_pos.0, vy - attacker_pos.1);
            // An unseen approach from behind ends in the knock-out blow at its shorter reach;
            // a drawn figure or a victim who faces the attacker means the sword fight.
            let behind = is_behind((vx, vy), self.entities[t].facing256, attacker_pos);
            let punching =
                self.entities[i].figure.is_none() && behind && can_punch(self, &self.entities[i]);
            let reach = if punching { PUNCH_REACH } else { FIGHT_RANGE };
            if Fixed::length(dx, dy) > Fixed::from_int(reach) {
                if self.entities[i].target.is_none() {
                    match self.walk_to(i, (vx, vy), Gait::Walk, budget) {
                        Walk::Planned => {}
                        // Unreachable: the order is dropped.
                        Walk::Unreachable => self.entities[i].attack_target = None,
                        // Unpaid: the order is kept and planned first next tick.
                        Walk::Exhausted => {
                            cursor = resume_at(&order, k, full);
                            break;
                        }
                    }
                }
                continue;
            }
            // In reach. A victim engaged with another player character fights one at a
            // time: this attacker waits at reach with his order until the victim is free
            // (the multi-party policy, a hypothesis: the measurements were one-on-one); a
            // blow from behind on an engaged victim rests on the same policy.
            let attacker_id = self.entities[i].id;
            if self.engaged_with_another(t, attacker_id, index) {
                self.record_assumption(Assumption::AttackPolicy(AttackRule::MultiParty));
                if !punching {
                    stop(&mut self.entities[i]);
                    continue;
                }
            }
            // Stop, face the victim, then the blow from behind or the fight. The frontal
            // fight at the fighting distance is measured; what an approach from behind ends
            // in is the reach-band hypothesis.
            if behind {
                self.record_assumption(Assumption::AttackPolicy(AttackRule::Reach));
            }
            let e = &mut self.entities[i];
            stop(e);
            e.attack_target = None;
            if dx.raw() != 0 || dy.raw() != 0 {
                e.facing256 = facing_of(dx, dy);
            }
            if punching {
                self.enter_timed(i, AiState::Punching, |s| &s.punch, PUNCH_TICKS);
                self.knock_down(t, attacker_pos);
            } else {
                self.start_fight(i, t, index, budget);
            }
        }
        self.cursors.attacks = cursor;
    }

    /// Whether soldier `t` is engaged in a reciprocal fight with a living, present player
    /// character other than `me`: he fights, his foe fights him back.
    fn engaged_with_another(&self, t: usize, me: EntityId, index: &SimIndex) -> bool {
        let v = &self.entities[t];
        v.ai_state == AiState::Fighting
            && v.foe.is_some_and(|f| {
                f != me
                    && index.slot(f).is_some_and(|fi| {
                        let f = &self.entities[fi];
                        f.alive
                            && f.active
                            && f.ai_state == AiState::Fighting
                            && f.foe == Some(v.id)
                    })
            })
    }

    /// The fight begins between the attacker `i` (a player character in reach, facing the
    /// victim) and the victim `t` (measured, `combat-measurements.md` 1.1: both bars appear as
    /// the attacker arrives; the victim's first swing comes at his normal cadence). The victim
    /// turns to the attacker, remembers his post (he returns there afterwards) and drops any
    /// alert. A victim engaged with another player character never gets here (the attacker
    /// waits, [`World::attack_orders`]); a stale pair he still names (a foe deactivated
    /// between ticks) is detached first, so that two living fighters always name each other
    /// (Codex review 10, finding 7). A figure the attacker holds stays pending and starts on
    /// the next tick.
    fn start_fight(&mut self, i: usize, t: usize, index: &SimIndex, budget: &mut u64) {
        let victim_id = self.entities[t].id;
        if let Some(f) = self.entities[t].foe.and_then(|id| index.slot(id))
            && f != i
            && self.entities[f].ai_state == AiState::Fighting
            && self.entities[f].foe == Some(victim_id)
        {
            self.end_fight(f, true, budget);
        }
        let (ax, ay) = (self.entities[i].x, self.entities[i].y);
        let (vx, vy) = (self.entities[t].x, self.entities[t].y);
        let attacker_id = self.entities[i].id;
        let e = &mut self.entities[i];
        e.ai_state = AiState::Fighting;
        e.state_ticks = 0;
        e.foe = Some(victim_id);
        e.pose = FightPose::Idle;
        e.pose_ticks = 0;
        e.swing_ticks = HERO_SWING_TICKS;
        let first_swing = self.next_swing(t);
        let v = &mut self.entities[t];
        stop(v);
        if v.ai_state == AiState::Patrol {
            v.alert_origin = Some((v.x, v.y));
        }
        v.ai_state = AiState::Fighting;
        v.state_ticks = 0;
        v.heard = false;
        v.last_seen = None;
        v.foe = Some(attacker_id);
        v.pose = FightPose::Idle;
        v.pose_ticks = 0;
        v.swing_ticks = first_swing;
        let (dx, dy) = (ax - vx, ay - vy);
        if dx.raw() != 0 || dy.raw() != 0 {
            v.facing256 = facing_of(dx, dy);
        }
    }

    /// Ticks until the next automatic swing: a soldier's measured cadence with the RNG's
    /// jitter (the spread is the engine's: [`AttackRule::HitChance`]), the hero's
    /// presentation interval.
    fn next_swing(&mut self, i: usize) -> u32 {
        if self.entities[i].kind == EntityKind::Player {
            HERO_SWING_TICKS
        } else {
            self.record_assumption(Assumption::AttackPolicy(AttackRule::HitChance));
            let jitter = self.rng.below(2 * SWING_JITTER_TICKS + 1);
            (SOLDIER_SWING_TICKS - SWING_JITTER_TICKS + jitter).max(1)
        }
    }

    /// One tick of a fighter: the fight ends when the foe is gone (dead, absent, no longer
    /// fighting him back or beyond [`FIGHT_BREAK_RANGE`]); otherwise the swing timer runs, a
    /// pose under way counts down and resolves its blow when it ends, a pending figure starts
    /// the powerful blow, and a due swing starts the next automatic strike. Always `true`: a
    /// fight that ends on an unpaid return enters [`AiState::ReturnPending`] rather than
    /// holding the walk.
    fn fight_tick(&mut self, i: usize, index: &SimIndex, budget: &mut u64) -> bool {
        let me = self.entities[i].id;
        let foe = self.entities[i].foe.and_then(|id| index.slot(id));
        let foe_alive = foe.is_some_and(|t| self.entities[t].alive && self.entities[t].active);
        let engaged = foe.is_some_and(|t| {
            let (e, foe) = (&self.entities[i], &self.entities[t]);
            foe.alive
                && foe.active
                && foe.ai_state == AiState::Fighting
                && foe.foe == Some(me)
                && Fixed::length(foe.x - e.x, foe.y - e.y) <= Fixed::from_int(FIGHT_BREAK_RANGE)
        });
        let Some(t) = foe.filter(|_| engaged) else {
            self.end_fight(i, foe_alive, budget);
            return true;
        };
        let e = &mut self.entities[i];
        if e.swing_ticks > 0 {
            e.swing_ticks -= 1;
        }
        if e.pose != FightPose::Idle {
            e.pose_ticks = e.pose_ticks.saturating_sub(1);
            if e.pose_ticks == 0 {
                let pose = std::mem::replace(&mut e.pose, FightPose::Idle);
                self.resolve_blow(i, t, pose);
            }
            return true;
        }
        if e.kind == EntityKind::Player && e.figure.take().is_some() {
            e.pose = FightPose::PowerfulBlow;
            e.pose_ticks = POWERFUL_BLOW_TICKS;
            return true;
        }
        if e.swing_ticks == 0 {
            if self.entities[i].kind == EntityKind::Player {
                // The hero's strike is presentation under the block reading: its start
                // already rests on it.
                self.record_assumption(Assumption::AttackPolicy(AttackRule::Block));
            }
            let ticks = state_ticks(self, &self.entities[i], |s| &s.strike, STRIKE_TICKS);
            let next = self.next_swing(i);
            let e = &mut self.entities[i];
            e.swing_ticks = next;
            e.pose = FightPose::Strike;
            e.pose_ticks = ticks;
        }
        true
    }

    /// A pose of fighter `i` against foe `t` ended: a soldier's strike lands two times in
    /// three for 5 hp and costs him one unit of energy when it does (measured cadence, the
    /// roll is [`AttackRule::HitChance`]); the hero's automatic strike never lands against a
    /// soldier ([`AttackRule::Block`], inferred: the pole arm's reach or a block); the
    /// powerful blow costs two units, lands one time in three ([`AttackRule::HitChance`]) for
    /// 50 hp (measured).
    fn resolve_blow(&mut self, i: usize, t: usize, pose: FightPose) {
        let from = (self.entities[i].x, self.entities[i].y);
        match pose {
            FightPose::Idle | FightPose::Flinch => {}
            FightPose::Strike => {
                if self.entities[i].kind == EntityKind::Player {
                    self.record_assumption(Assumption::AttackPolicy(AttackRule::Block));
                } else {
                    self.record_assumption(Assumption::AttackPolicy(AttackRule::HitChance));
                    if self.rng.below(SOLDIER_HIT_CHANCE.1) < SOLDIER_HIT_CHANCE.0 {
                        self.spend_energy(i, SOLDIER_HIT_ENERGY);
                        self.damage(t, from, SOLDIER_HIT_DAMAGE);
                    }
                }
            }
            FightPose::PowerfulBlow => {
                self.record_assumption(Assumption::AttackPolicy(AttackRule::HitChance));
                self.spend_energy(i, POWERFUL_BLOW_ENERGY);
                if self.rng.below(POWERFUL_BLOW_CHANCE.1) < POWERFUL_BLOW_CHANCE.0 {
                    self.damage(t, from, POWERFUL_BLOW_DAMAGE);
                }
            }
        }
    }

    /// Ticks a fighter needs to regain one unit of energy.
    fn energy_regen_ticks(e: &Entity) -> u32 {
        if e.kind == EntityKind::Player {
            HERO_ENERGY_REGEN_TICKS
        } else {
            SOLDIER_ENERGY_REGEN_TICKS
        }
    }

    /// A blow cost `cost` units of energy (never below 0); the regain timer starts unless it
    /// is already running.
    fn spend_energy(&mut self, i: usize, cost: i32) {
        let regen = Self::energy_regen_ticks(&self.entities[i]);
        let e = &mut self.entities[i];
        e.energy = (e.energy - cost).max(0);
        if e.energy < ENERGY_MAX && e.energy_ticks == 0 {
            e.energy_ticks = regen;
        }
    }

    /// The energy regains one unit every regain interval while below the maximum (health
    /// never regenerates: measured).
    fn regain_energy(&mut self, i: usize) {
        let regen = Self::energy_regen_ticks(&self.entities[i]);
        let e = &mut self.entities[i];
        if e.energy >= ENERGY_MAX {
            e.energy_ticks = 0;
            return;
        }
        if e.energy_ticks > 1 {
            e.energy_ticks -= 1;
            return;
        }
        e.energy += 1;
        e.energy_ticks = if e.energy < ENERGY_MAX { regen } else { 0 };
    }

    /// `amount` hit points are taken from entity `t` by a blow from `from`: a damage number
    /// rises over his head, he flinches in the stance if he stands idle in it, and at 0 he
    /// dies ([`World::kill`]).
    fn damage(&mut self, t: usize, from: (Fixed, Fixed), amount: i32) {
        let v = &mut self.entities[t];
        if !v.alive {
            return;
        }
        v.hp = (v.hp - amount).max(0);
        let hp = v.hp;
        let at = (v.x.round(), v.y.round());
        self.push_damage_number(at, amount);
        if hp == 0 {
            self.kill(t, from);
            return;
        }
        let v = &self.entities[t];
        if v.ai_state == AiState::Fighting && v.pose == FightPose::Idle {
            let ticks = state_ticks(self, v, |s| &s.flinch, FLINCH_TICKS);
            let v = &mut self.entities[t];
            v.pose = FightPose::Flinch;
            v.pose_ticks = ticks;
        }
    }

    /// Entity `t` dies of a blow from `from`: not alive from this tick on (natives 85 / 87 /
    /// 90 report it), falling backward when struck from the front (44, then 48) or forward
    /// (41, then 47) for the animation's length, then `Dead`. Every order and fight of his
    /// ends (his foe notices on the next tick); a player character's death raises
    /// `World::hero_dead` (measured for a lone hero, `combat-measurements.md` 4; with another
    /// player character still alive the loss is a hypothesis, [`Assumption::HeroDeathLoss`]).
    fn kill(&mut self, t: usize, from: (Fixed, Fixed)) {
        let v = &self.entities[t];
        let backward = !is_behind((v.x, v.y), v.facing256, from);
        let block: fn(&AnimSet) -> &[u32; 8] = if backward {
            |s| &s.knocked_down_back
        } else {
            |s| &s.knocked_down
        };
        let v = &mut self.entities[t];
        stop(v);
        v.alive = false;
        v.hp = 0;
        v.fell_backward = backward;
        v.attack_target = None;
        v.figure = None;
        v.foe = None;
        v.pose = FightPose::Idle;
        v.pose_ticks = 0;
        v.swing_ticks = 0;
        v.heard = false;
        v.last_seen = None;
        v.alert_origin = None;
        self.enter_timed(t, AiState::Dying, block, KNOCKED_DOWN_TICKS);
        if self.entities[t].kind == EntityKind::Player {
            self.hero_dead = true;
            let others = self
                .entities
                .iter()
                .any(|e| e.kind == EntityKind::Player && e.alive && e.active);
            if others {
                self.record_assumption(Assumption::HeroDeathLoss);
            }
        }
    }

    /// Fighter `i` leaves the fight: a player character stands where he is (normal state), a
    /// soldier walks back to his post or patrols where he stands. A soldier whose foe is
    /// still alive (`foe_alive`, looked up by the caller: the tick's index, or a scan on the
    /// player's order) stands his ground rather than chasing (measured for the halberdier,
    /// `combat-measurements.md` 3; a hypothesis for every other kind:
    /// [`AttackRule::PostBound`]). A way back the budget could not pay leaves him
    /// [`AiState::ReturnPending`]: searched again next tick, never a patrol where he stands
    /// (Codex review 10, finding 4).
    pub(crate) fn end_fight(&mut self, i: usize, foe_alive: bool, budget: &mut u64) {
        let e = &mut self.entities[i];
        e.foe = None;
        e.pose = FightPose::Idle;
        e.pose_ticks = 0;
        e.swing_ticks = 0;
        e.figure = None;
        if e.kind == EntityKind::Player {
            e.ai_state = AiState::Patrol;
            e.state_ticks = 0;
            e.last_seen = None;
            e.alert_origin = None;
            return;
        }
        let here = (e.x, e.y);
        let origin = e.alert_origin;
        if foe_alive {
            self.record_assumption(Assumption::AttackPolicy(AttackRule::PostBound));
        }
        match origin {
            Some(origin) if origin != here => match self.walk_to(i, origin, Gait::Walk, budget) {
                Walk::Planned => {
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::Returning;
                    e.state_ticks = 0;
                }
                // Unreachable: his post is where he stands.
                Walk::Unreachable => self.resume_patrol(i),
                // Unpaid: the return is pending, its search retried next tick.
                Walk::Exhausted => {
                    let e = &mut self.entities[i];
                    e.ai_state = AiState::ReturnPending;
                    e.state_ticks = 0;
                }
            },
            _ => self.resume_patrol(i),
        }
    }

    /// The nearest enemy soldier a player character at `(x, y)` can fight (slot order breaks
    /// ties), for the figures' lock-on (`combat-measurements.md` 1.4: the figure locks onto
    /// the nearest enemy while the button is held).
    pub(crate) fn nearest_fightable(&self, x: Fixed, y: Fixed) -> Option<usize> {
        self.entities
            .iter()
            .enumerate()
            .filter(|(_, e)| fightable(e))
            .min_by_key(|(_, e)| Fixed::length(e.x - x, e.y - y).raw())
            .map(|(i, _)| i)
    }

    /// The blow lands on `t` from `from`: the victim goes down forward when struck from behind,
    /// backward otherwise (41 / 44), unless his resistance makes him immune, in which case the
    /// blow is a stimulus (he notices the attacker). The resistance is the profile's `p4`
    /// (hypothesis): consulting it records `Assumption::ProfileStats` on the script, if any;
    /// what the blow does (the fall, the timer, the immunity) is the knock-out policy
    /// (`Assumption::KnockOut`), recorded here where it first changes the victim's state.
    fn knock_down(&mut self, t: usize, from: (Fixed, Fixed)) {
        self.record_assumption(Assumption::ProfileStats);
        self.record_assumption(Assumption::KnockOut);
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
        e.heard = false;
        // A victim struck in a fight leaves it (his foe notices on the next tick).
        e.foe = None;
        e.pose = FightPose::Idle;
        e.pose_ticks = 0;
        e.swing_ticks = 0;
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
    use crate::world::{SIM_WORK_PER_TICK, Scenario, Snapshot};

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

    /// The measured cone (`h01-measurements-2.md` 6): a sector of 80 degrees bound to the
    /// facing, reaching 270 px along x and 196 px along y (an ellipse), tested with the
    /// engine's constants.
    #[test]
    fn view_cone_geometry() {
        let (r, h) = (VIEW_RANGE, VIEW_CONE_HALF_ANGLE_256);
        let o = (f(1000), f(1000));
        // Facing +x: a hero 250 px ahead is seen, 250 px behind is not; the reach along x is
        // 270 (at 270 inside, at 271 outside).
        assert!(in_view_cone(o, 0, (f(1250), f(1000)), r, h), "250 px ahead");
        assert!(
            !in_view_cone(o, 0, (f(750), f(1000)), r, h),
            "250 px behind"
        );
        assert!(
            in_view_cone(o, 0, (f(1270), f(1000)), r, h),
            "at the x reach"
        );
        assert!(!in_view_cone(o, 0, (f(1271), f(1000)), r, h), "beyond it");
        // Facing north (-y, facing 192): the reach along y is 196: seen at 180, not at 200.
        assert!(
            in_view_cone(o, 192, (f(1000), f(820)), r, h),
            "180 px north"
        );
        assert!(
            !in_view_cone(o, 192, (f(1000), f(800)), r, h),
            "200 px north"
        );
        assert!(
            in_view_cone(o, 192, (f(1000), f(806)), r, h),
            "194 px north"
        );
        assert!(
            !in_view_cone(o, 192, (f(1000), f(805)), r, h),
            "195 px north"
        );
        assert!(
            !in_view_cone(o, 192, (f(1000), f(1250)), r, h),
            "250 px south"
        );
        // The half angle is about 40 degrees (28 / 256 turns = 39.4): facing +x at 100 px,
        // an offset of 80 px sideways (38.7 degrees) is inside, 86 px (40.7) outside.
        assert!(in_view_cone(o, 0, (f(1100), f(1080)), r, h), "38.7 degrees");
        assert!(
            !in_view_cone(o, 0, (f(1100), f(1086)), r, h),
            "40.7 degrees"
        );
        assert!(!in_view_cone(o, 0, (f(1000), f(1100)), r, h), "sideways");
        assert!(in_view_cone(o, 0, o, r, h), "the observer's own spot");
        // The ellipse: the diagonal (150, 150) weighs sqrt(150^2 + 208^2) = 257 (inside), the
        // diagonal (170, 170) 291 (outside); facing the diagonal so the angle passes.
        assert!(in_view_cone(o, 32, (f(1150), f(1150)), r, h));
        assert!(!in_view_cone(o, 32, (f(1170), f(1170)), r, h));
        assert!(within_elliptical_reach(f(0), f(194), r));
        assert!(!within_elliptical_reach(f(0), f(195), r));
        assert!(within_elliptical_reach(f(-270), f(0), r));
        // The crouch divisor halves both axes.
        assert!(in_view_cone(
            o,
            0,
            (f(1130), f(1000)),
            r / CROUCH_VIEW_DIVISOR,
            h
        ));
        assert!(!in_view_cone(
            o,
            0,
            (f(1140), f(1000)),
            r / CROUCH_VIEW_DIVISOR,
            h
        ));
        // Extremes never panic.
        let big = (Fixed::MAX, Fixed::MIN);
        let _ = in_view_cone(big, i32::MIN, (Fixed::MIN, Fixed::MAX), i32::MAX, i32::MAX);
        let _ = in_view_cone(big, i32::MIN, (Fixed::MIN, Fixed::MAX), i32::MIN, i32::MIN);
        let _ = within_elliptical_reach(Fixed::MIN, Fixed::MIN, i32::MAX);
        let _ = is_behind(big, i32::MAX, (Fixed::MIN, Fixed::MIN));
    }

    /// The rear radius (`h01-measurements-2.md` 3, hypothesis): a standing hero 40 px behind
    /// a soldier is seen whatever the soldier faces and the sighting records `SightCone`; at
    /// 60 px behind him nothing is seen; a crouched hero is seen within half the radius only.
    #[test]
    fn rear_radius_and_crouch_divisor_are_the_hypotheses_the_sighting_records() {
        use crate::vm::tests::{class, mission_world, program};
        // A world with a script so the taint is recorded; the soldier at (300, 300) faces +x.
        let fresh = || {
            let level = class("StartUp", 0, &[]);
            let mut w = mission_world(1, Some(program(vec![level], 1)));
            w.entities[1].facing256 = 0;
            w
        };
        let mut w = fresh();
        let hero_at = |w: &mut World, x: i32, y: i32| {
            w.entities[0].x = Fixed::from_int(x);
            w.entities[0].y = Fixed::from_int(y);
            w.entities[0].posture = Posture::Standing;
        };
        // 60 px behind: nothing.
        hero_at(&mut w, 240, 300);
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::Patrol);
        // 40 px behind: noticed under the rear-radius hypothesis.
        hero_at(&mut w, 260, 300);
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::Noticed);
        assert!(
            w.vm.as_ref()
                .unwrap()
                .assumptions
                .contains(&Assumption::SightCone)
        );
        // A crouched hero 40 px behind: beyond the halved radius, nothing.
        let mut w = fresh();
        hero_at(&mut w, 260, 300);
        w.entities[0].posture = Posture::Crouched;
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::Patrol);
        // A standing hero 200 px ahead: the measured cone, no assumption.
        let mut w = fresh();
        hero_at(&mut w, 500, 300);
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::Noticed);
        assert!(
            !w.vm
                .as_ref()
                .unwrap()
                .assumptions
                .contains(&Assumption::SightCone)
        );
        // A crouched hero 100 px ahead: the crouch divisor decided the sighting (270 / 2 =
        // 135 px reach): a hypothesis.
        let mut w = fresh();
        hero_at(&mut w, 400, 300);
        w.entities[0].posture = Posture::Crouched;
        w.step(&[]);
        assert_eq!(w.entities[1].ai_state, AiState::Noticed);
        assert!(
            w.vm.as_ref()
                .unwrap()
                .assumptions
                .contains(&Assumption::SightCone)
        );
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
        // Behind the guard's back but running within the noise radius: heard, and he charges
        // at once (no noticed / alarm pause: measured), running to where the noise was.
        click(&mut w, 500, 240, Button::Left);
        click(&mut w, 500, 240, Button::Left);
        assert_eq!(w.entities[0].gait, Gait::Run);
        let g = &w.entities[1];
        assert_eq!(g.ai_state, AiState::Alerted);
        assert!(g.heard && g.target.is_some() && g.gait == Gait::Run);
        assert_eq!(g.state_ticks, ALERT_TIMEOUT_TICKS);
        assert_eq!(g.alert_origin, Some((f(300), f(240))));
        assert_eq!(g.action, actions::ALERT_RUN);
        w.validate().unwrap();
        // A run heard from 340 px (beyond the measured 330 px bound) alerts; from 360 px
        // (beyond the radius) it does not.
        let mut w = scene((60, 240), 128, (400, 240));
        click(&mut w, 500, 240, Button::Left);
        click(&mut w, 500, 240, Button::Left);
        assert_eq!(states(&w).1, AiState::Alerted);
        let mut w = scene((40, 240), 128, (400, 240));
        click(&mut w, 500, 240, Button::Left);
        click(&mut w, 500, 240, Button::Left);
        assert_eq!(states(&w).1, AiState::Patrol);
        // A sighting keeps the noticed -> alarm sequence and is not marked as heard.
        let mut w = scene((400, 240), 128, (250, 240));
        w.step(&[]);
        assert_eq!(states(&w).1, AiState::Noticed);
        assert!(!w.entities[1].heard);
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
        // The guard at (400, 240) faces -x (away from the player at (500, 240)); the player
        // sneaks up (crouched: the rear radius halves to 25 px, under the punch's reach; a
        // walking approach would be noticed at 50 px, `REAR_SIGHT_RADIUS`).
        let mut w = scene((400, 240), 128, (500, 240));
        w.entities[0].posture = Posture::Crouched;
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
            assert!(ticks < 400, "never struck: {:?}", states(&w));
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

        // From the front: the guard faces the player; the character walks up, stops at the
        // fighting distance facing him and the sword fight begins, no blow (measured,
        // `combat-measurements.md` 1.1; the guard had noticed him on the way).
        let mut w = scene((400, 240), 0, (500, 240));
        click(&mut w, 400, 240, Button::Left);
        let mut ticks = 0;
        while w.entities[0].attack_target.is_some() {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 200);
        }
        let (p, g) = (&w.entities[0], &w.entities[1]);
        assert_eq!(states(&w), (AiState::Fighting, AiState::Fighting));
        assert_eq!(p.facing256, 128);
        assert!(p.target.is_none() && g.target.is_none());
        assert_eq!((p.foe, g.foe), (Some(g.id), Some(p.id)));
        assert_eq!(g.facing256, 0, "turns to the attacker");
        let d = Fixed::length(p.x - g.x, p.y - g.y);
        assert!(d <= f(FIGHT_RANGE) && d >= f(FIGHT_RANGE - 3), "{d:?}");
        assert_eq!(
            (p.action, g.action),
            (actions::FIGHT_IDLE, actions::FIGHT_IDLE)
        );
        assert!(g.alert_origin.is_some() && g.state_ticks == 0 && !g.heard);
        w.validate().unwrap();
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
    fn a_profile_without_the_punch_fights_instead() {
        use crate::anim::{AnimSet, Catalog, FrameSpec};
        let frame = |frame| FrameSpec {
            frame,
            duration: 1,
            advance: 0,
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
        assert_eq!(states(&w), (AiState::Fighting, AiState::Fighting));
        assert!(w.entities[0].target.is_none() && w.entities[0].attack_target.is_none());
        assert_eq!(w.entities[1].foe, Some(w.entities[0].id));
        w.validate().unwrap();
    }

    #[test]
    fn timed_states_last_their_animation_with_a_catalog() {
        use crate::anim::{AnimSet, Catalog, FrameSpec};
        let frame = |duration| FrameSpec {
            frame: 0,
            duration,
            advance: 0,
            offset_x: 0,
            offset_y: 0,
        };
        // Animation 2 is the noticed block (3 + 4 table ticks = 315 clock units = 20 world
        // ticks), 3 the alarm block (2 table ticks = 90 units = 6 world ticks).
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
        assert_eq!((g.ai_state, g.state_ticks), (AiState::Noticed, 20));
        assert_eq!(g.anim.as_ref().unwrap().animation, 2);
        // The first frame (135 units) changes on the ninth tick of the state.
        for _ in 0..8 {
            w.step(&[]);
        }
        assert_eq!(w.entities[1].anim.as_ref().unwrap().frame, 0);
        w.step(&[]);
        assert_eq!(w.entities[1].anim.as_ref().unwrap().frame, 1);
        for _ in 0..10 {
            w.step(&[]);
        }
        assert_eq!(w.entities[1].ai_state, AiState::Noticed);
        assert_eq!(w.entities[1].anim.as_ref().unwrap().frame, 1);
        // The twentieth tick ends the state as the loop completes.
        w.step(&[]);
        let g = &w.entities[1];
        assert_eq!((g.ai_state, g.state_ticks), (AiState::Alarm, 6));
        assert_eq!(g.anim.as_ref().unwrap().animation, 3);
        w.validate().unwrap();
    }

    #[test]
    fn stealth_state_survives_snapshots_and_is_validated() {
        let mut w = scene((400, 240), 128, (500, 240));
        w.entities[0].posture = Posture::Crouched;
        click(&mut w, 400, 240, Button::Left);
        // 68 px into reach at the sneak's 0.28 px per tick, then the 60-tick fall.
        for _ in 0..400 {
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
        // The melee's invariants: hit points within the maximum and 0 exactly when dead, the
        // energy timer running exactly below full, a foe exactly in a fight and of the other
        // side, a pose with its timer only in a fight, the swing timer only in a fight, a
        // figure only with an order or a fight.
        reject(&mut w, |s| s.entities[1].hp = 0, "hit points");
        reject(&mut w, |s| s.entities[1].hp = 101, "hit points");
        reject(&mut w, |s| s.entities[1].energy = 21, "energy");
        reject(&mut w, |s| s.entities[1].energy = 19, "energy timer");
        reject(&mut w, |s| s.entities[1].energy_ticks = 3, "energy timer");
        reject(
            &mut w,
            |s| s.entities[1].foe = Some(s.entities[0].id),
            "foe",
        );
        reject(
            &mut w,
            |s| {
                s.entities[1].ai_state = AiState::Fighting;
                s.entities[1].state_ticks = 0;
            },
            "foe",
        );
        reject(
            &mut w,
            |s| {
                s.entities[0].ai_state = AiState::Fighting;
                s.entities[0].foe = Some(s.entities[0].id);
            },
            "foe",
        );
        reject(&mut w, |s| s.entities[0].pose = FightPose::Strike, "pose");
        reject(&mut w, |s| s.entities[0].pose_ticks = 3, "pose");
        reject(&mut w, |s| s.entities[0].swing_ticks = 3, "swing");
        reject(
            &mut w,
            |s| s.entities[0].figure = Some(Figure::ForwardStroke),
            "figure",
        );
        reject(
            &mut w,
            |s| {
                s.damage_numbers.push(crate::world::DamageNumber {
                    x: 0,
                    y: 0,
                    amount: 5,
                    age: 90,
                });
            },
            "damage number",
        );
        // The consistent forms are accepted.
        let mut snap = w.snapshot(None);
        snap.world.entities[1].ai_state = AiState::Dead;
        snap.world.entities[1].alive = false;
        snap.world.entities[1].state_ticks = 0;
        snap.world.entities[1].hp = 0;
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
        w.entities[1].ai_state = AiState::Dying;
        let s = g(&w);
        assert!(s.dead && s.out_of_action && !s.knocked_out && !s.can_act && s.present);
        w.entities[1].ai_state = AiState::Fighting;
        w.entities[1].alive = true;
        let s = g(&w);
        assert!(!s.dead && !s.out_of_action && !s.knocked_out && s.can_act && s.present);
        w.entities[1].ai_state = AiState::Patrol;
        w.entities[1].alive = true;
        w.entities[1].active = false;
        let s = g(&w);
        assert!(!s.dead && !s.present && !s.can_act);
    }

    /// The corridor with the guard facing the player, the fight started by a click on him:
    /// returns the world on the first tick of the fight.
    fn fight() -> World {
        let mut w = scene((400, 240), 0, (500, 240));
        click(&mut w, 400, 240, Button::Left);
        let mut ticks = 0;
        while states(&w) != (AiState::Fighting, AiState::Fighting) {
            w.step(&[]);
            ticks += 1;
            assert!(ticks < 200, "{:?}", states(&w));
        }
        w
    }

    /// The forward stroke: the left button pressed on open ground, dragged 80 px right and 20
    /// px up, released (`combat-measurements.md` 1.4), in two ticks.
    fn forward_stroke(w: &mut World, x: i32, y: i32) {
        w.step(&[
            InputEvent::PointerMove {
                x256: x * 256,
                y256: y * 256,
            },
            InputEvent::PointerDown {
                button: Button::Left,
            },
        ]);
        w.step(&[
            InputEvent::PointerMove {
                x256: (x + 80) * 256,
                y256: (y - 20) * 256,
            },
            InputEvent::PointerUp {
                button: Button::Left,
            },
        ]);
    }

    /// The soldier's blows (`combat-measurements.md` 1.5, measured): a swing every ~5.3 s of
    /// which two in three land for 5 hp, so the hero's 100 hp fall by 5 at a time at a mean
    /// interval near 8 s; his click attacks never hurt the soldier (1.3) and cost no energy;
    /// a landed hit costs the soldier one unit of energy, regained in ~4 s; health never
    /// regenerates; at 0 hp the hero dies (his fall, then lying for good; `hero_dead`), the
    /// soldier's fight ends and he stands at his post. A snapshot taken mid-fight restores to
    /// the same run.
    #[test]
    fn soldier_blows_land_at_the_measured_cadence_until_the_hero_dies() {
        let mut w = fight();
        let start = w.tick;
        assert_eq!(w.entities[0].hp, HERO_HIT_POINTS);
        assert_eq!(w.entities[1].hp, DEFAULT_HIT_POINTS);
        let g = &w.entities[1];
        assert!(
            (SOLDIER_SWING_TICKS - SWING_JITTER_TICKS..=SOLDIER_SWING_TICKS + SWING_JITTER_TICKS)
                .contains(&g.swing_ticks),
            "{}",
            g.swing_ticks
        );
        let mut hits: Vec<u64> = Vec::new();
        let mut swings = 0;
        let mut hp = w.entities[0].hp;
        let mut regained_after = None;
        let mut spent_at = None;
        let mut mid = None;
        let mut ticks = 0u64;
        while w.entities[0].alive {
            w.step(&[]);
            ticks += 1;
            assert!(
                ticks < 30_000,
                "the hero never died: {} hp",
                w.entities[0].hp
            );
            if ticks == 1000 {
                mid = Some(w.snapshot(None));
            }
            let (p, g) = (&w.entities[0], &w.entities[1]);
            if g.pose == FightPose::Strike && g.pose_ticks == STRIKE_TICKS {
                swings += 1;
            }
            if p.hp != hp {
                assert_eq!(hp - p.hp, SOLDIER_HIT_DAMAGE, "one basic hit at a time");
                hp = p.hp;
                hits.push(w.tick);
                assert_eq!(g.energy, ENERGY_MAX - SOLDIER_HIT_ENERGY);
                assert_eq!(g.energy_ticks, SOLDIER_ENERGY_REGEN_TICKS);
                spent_at = Some(w.tick);
            } else if let Some(t) = spent_at
                && g.energy == ENERGY_MAX
            {
                regained_after = Some(w.tick - t);
                spent_at = None;
            }
            assert_eq!(p.energy, ENERGY_MAX, "click attacks cost nothing");
            assert_eq!(g.hp, DEFAULT_HIT_POINTS, "click attacks never land");
            if ticks.is_multiple_of(500) {
                w.validate().unwrap();
            }
        }
        assert_eq!(hits.len(), (HERO_HIT_POINTS / SOLDIER_HIT_DAMAGE) as usize);
        assert_eq!(regained_after, Some(u64::from(SOLDIER_ENERGY_REGEN_TICKS)));
        let span = hits.last().unwrap() - start;
        let mean = span / (hits.len() as u64 - 1);
        assert!(
            (380..=600).contains(&mean),
            "mean interval between landed hits {mean} ticks (measured median 462)"
        );
        // Two of three swings land, within the sample's spread.
        assert!(
            swings >= hits.len() + 3 && swings <= hits.len() * 2,
            "{swings} swings for {} hits",
            hits.len()
        );
        // The death: not alive from the tick of the blow, falling backward (struck from the
        // front), every order gone, the loss raised; the soldier notices next tick.
        let p = &w.entities[0];
        assert_eq!(p.ai_state, AiState::Dying);
        assert_eq!(p.state_ticks, KNOCKED_DOWN_TICKS);
        assert!(p.fell_backward && p.foe.is_none() && p.target.is_none() && p.hp == 0);
        assert_eq!(p.action, actions::KNOCKED_DOWN_BACK);
        assert!(w.hero_dead);
        let s = ActorStatus::of(p);
        assert!(s.dead && s.out_of_action && !s.can_act && !s.knocked_out);
        assert_eq!(w.entities[1].ai_state, AiState::Fighting);
        w.step(&[]);
        let g = &w.entities[1];
        assert_eq!(g.ai_state, AiState::Patrol, "his post is where he stands");
        assert!(g.foe.is_none() && g.target.is_none() && g.alert_origin.is_none());
        for _ in 1..KNOCKED_DOWN_TICKS {
            w.step(&[]);
        }
        let p = &w.entities[0];
        assert_eq!(p.ai_state, AiState::Dead);
        assert_eq!(p.action, actions::LYING_BACK);
        assert!(!p.alive && p.state_ticks == 0);
        w.validate().unwrap();
        // The damage numbers rose and vanished.
        assert!(w.damage_numbers.iter().all(|d| d.age < DAMAGE_NUMBER_TICKS));
        // Determinism across a mid-fight snapshot.
        let mid = mid.unwrap();
        let json = serde_json::to_string(&mid).unwrap();
        let snap: Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = World::new(Scenario::Synthetic("corridor".into()), 3).unwrap();
        w2.restore(&snap).unwrap();
        assert_eq!(w2.entities[0].ai_state, AiState::Fighting);
        let mut w1 = World::new(Scenario::Synthetic("corridor".into()), 3).unwrap();
        w1.restore(&mid).unwrap();
        assert_eq!(w1.hashes(), w2.hashes());
        while w2.tick < w.tick {
            w1.step(&[]);
            w2.step(&[]);
        }
        assert_eq!(w1.hashes(), w2.hashes());
        assert_eq!(w2.hashes(), w.hashes());
    }

    /// The forward stroke (`combat-measurements.md` 1.4): the figure locks onto the nearest
    /// soldier, the hero walks up and fights him, the powerful blow resolves
    /// `POWERFUL_BLOW_TICKS` after it starts, costs two units of energy (regained one per
    /// `HERO_ENERGY_REGEN_TICKS`) and does 50 hp when it lands (one time in three across
    /// seeds); two landed blows kill an 80 hp soldier, who falls and lies for good, out of
    /// action for native 90 and dead for 87.
    #[test]
    fn the_forward_stroke_is_the_powerful_blow_and_two_kill_a_soldier() {
        let mut landed = 0;
        let mut missed = 0;
        let mut killed = false;
        for seed in 1..=12u64 {
            let mut w = scene((400, 240), 0, (500, 240));
            w.seed = seed;
            w.rng = crate::rng::Rng::new(seed, crate::world::GAMEPLAY_RNG_STREAM);
            // A blue halberdier's 80 hp.
            w.entities[1].hp = 80;
            w.entities[1].hp_max = 80;
            forward_stroke(&mut w, 520, 260);
            let p = &w.entities[0];
            assert_eq!(p.attack_target, Some(w.entities[1].id));
            assert_eq!(p.figure, Some(Figure::ForwardStroke));
            assert!(p.target.is_some(), "walks up to him");
            let mut ticks = 0;
            while w.entities[0].pose != FightPose::PowerfulBlow {
                w.step(&[]);
                ticks += 1;
                assert!(ticks < 200, "{:?}", states(&w));
            }
            let p = &w.entities[0];
            assert_eq!(p.ai_state, AiState::Fighting);
            assert_eq!(p.pose_ticks, POWERFUL_BLOW_TICKS);
            assert_eq!(p.action, actions::POWERFUL_BLOW);
            assert!(p.figure.is_none() && p.energy == ENERGY_MAX);
            for _ in 0..POWERFUL_BLOW_TICKS {
                w.step(&[]);
            }
            let (p, g) = (&w.entities[0], &w.entities[1]);
            assert_eq!(p.pose, FightPose::Idle);
            assert_eq!(p.energy, ENERGY_MAX - POWERFUL_BLOW_ENERGY);
            assert_eq!(p.energy_ticks, HERO_ENERGY_REGEN_TICKS);
            match g.hp {
                30 => {
                    landed += 1;
                    assert_eq!(w.damage_numbers.last().map(|d| d.amount), Some(50));
                    assert!(g.pose == FightPose::Flinch || g.pose == FightPose::Strike);
                }
                80 => missed += 1,
                hp => panic!("{hp} hp after the blow"),
            }
            // The energy comes back one unit per interval.
            for _ in 0..HERO_ENERGY_REGEN_TICKS {
                w.step(&[]);
            }
            assert_eq!(w.entities[0].energy, ENERGY_MAX - 1);
            for _ in 0..HERO_ENERGY_REGEN_TICKS {
                w.step(&[]);
            }
            assert_eq!(w.entities[0].energy, ENERGY_MAX);
            assert_eq!(w.entities[0].energy_ticks, 0);
            w.validate().unwrap();
            if killed {
                continue;
            }
            // Strokes until he dies (each one in the fight: the figure is delivered in the
            // next idle pose).
            'strokes: for _ in 0..40 {
                forward_stroke(&mut w, 520, 260);
                assert!(w.entities[0].ai_state == AiState::Fighting);
                for _ in 0..(POWERFUL_BLOW_TICKS + STRIKE_TICKS + FLINCH_TICKS + 4) {
                    w.step(&[]);
                    if !w.entities[1].alive {
                        break 'strokes;
                    }
                }
            }
            let g = &w.entities[1];
            assert!(!g.alive && g.hp == 0, "{} hp", g.hp);
            assert_eq!(g.ai_state, AiState::Dying);
            assert!(g.fell_backward, "struck from the front");
            assert_eq!(g.action, actions::KNOCKED_DOWN_BACK);
            let s = ActorStatus::of(g);
            assert!(s.dead && s.out_of_action && !s.knocked_out && !s.can_act);
            assert!(!w.hero_dead);
            w.step(&[]);
            assert_eq!(w.entities[0].ai_state, AiState::Patrol, "the fight is over");
            assert!(w.entities[0].foe.is_none());
            for _ in 0..KNOCKED_DOWN_TICKS {
                w.step(&[]);
            }
            let g = &w.entities[1];
            assert_eq!(g.ai_state, AiState::Dead);
            assert_eq!(g.action, actions::LYING_BACK);
            w.validate().unwrap();
            // A dead soldier takes no orders and is not picked.
            click(&mut w, 400, 240, Button::Left);
            assert!(w.entities[0].attack_target.is_none());
            killed = true;
        }
        assert!(
            landed >= 1 && missed >= 1,
            "{landed} landed, {missed} missed of 12"
        );
        assert!(killed);
    }

    /// A ground order while fighting leaves the fight: the hero walks off and the soldier
    /// stands his ground (post-bound: measured for the halberdier); a right click on the
    /// hero leaves it too; the click on an enemy acts on the button's release, so a press
    /// held still is still a click, and a stroke to the left orders nothing.
    #[test]
    fn leaving_the_fight_and_the_release_rule() {
        let mut w = fight();
        click(&mut w, 600, 240, Button::Left);
        let (p, g) = (&w.entities[0], &w.entities[1]);
        assert_eq!(p.ai_state, AiState::Patrol);
        assert!(p.foe.is_none() && p.target.is_some() && p.pose == FightPose::Idle);
        // Stands his ground (his post is where he fought); the hero in his cone is a fresh
        // sighting on the same tick.
        assert!(
            matches!(g.ai_state, AiState::Patrol | AiState::Noticed),
            "{:?}",
            g.ai_state
        );
        assert!(g.foe.is_none() && g.target.is_none() && g.swing_ticks == 0);
        w.validate().unwrap();
        let mut w = fight();
        let (px, py) = (w.entities[0].x.round(), w.entities[0].y.round());
        click(&mut w, px, py, Button::Right);
        assert_eq!(states(&w).0, AiState::Patrol);
        assert!(w.entities[1].foe.is_none() && w.entities[1].target.is_none());
        // A press without a release orders nothing yet; the release is the click.
        let mut w = scene((400, 240), 0, (500, 240));
        w.step(&[
            InputEvent::PointerMove {
                x256: 400 * 256,
                y256: 240 * 256,
            },
            InputEvent::PointerDown {
                button: Button::Left,
            },
        ]);
        assert!(w.entities[0].attack_target.is_none() && w.press.is_some());
        for _ in 0..5 {
            w.step(&[]);
        }
        w.step(&[InputEvent::PointerUp {
            button: Button::Left,
        }]);
        assert!(w.entities[0].attack_target.is_some() && w.press.is_none());
        // A stroke to the left is not a figure the engine reads: nothing happens.
        let mut w = scene((400, 240), 0, (500, 240));
        w.step(&[
            InputEvent::PointerMove {
                x256: 600 * 256,
                y256: 260 * 256,
            },
            InputEvent::PointerDown {
                button: Button::Left,
            },
        ]);
        w.step(&[
            InputEvent::PointerMove {
                x256: 520 * 256,
                y256: 240 * 256,
            },
            InputEvent::PointerUp {
                button: Button::Left,
            },
        ]);
        let p = &w.entities[0];
        assert!(p.attack_target.is_none() && p.target.is_none() && p.figure.is_none());
        w.validate().unwrap();
    }

    /// Finding 4 of Codex review 10: a transition and the path it plans are one step. A
    /// soldier late in the states walk whose search the grant cannot pay keeps his state (the
    /// alarm's last tick, the patrol before a charge) with the cursor resting on him, and the
    /// next tick, where he comes first with the full cap, applies the transition; a fight that
    /// ends on an unpaid return leaves him `ReturnPending`, searched again next tick, never
    /// patrolling where he stands. All of it survives a snapshot.
    #[test]
    fn an_unpaid_transition_search_is_retried_first_next_tick() {
        use crate::world::Snapshot;
        // Alarm -> Alerted: guards 1 and 2 locked (not stepped), guard 3 on the last tick of
        // his alarm with the hero's distant position to run to; the budget covers the pre-index
        // (4), the perception (guard 3 alone: 1 + 1 pair, nothing seen at 1000 px), the hero's
        // and the guard's units and 50 units of a search that needs thousands.
        let mut w = crowd(3, 1);
        w.entities[1].ai_locked = true;
        w.entities[2].ai_locked = true;
        let g = &mut w.entities[3];
        g.ai_state = AiState::Alarm;
        g.state_ticks = 1;
        g.last_seen = Some((f(900), f(700)));
        g.alert_origin = Some((g.x, g.y));
        w.validate().unwrap();
        let snap = w.snapshot(None);
        let spent = w.ai_tick_with(4 + 2 + 2 + 50);
        assert_eq!(spent, 58, "the partial search spent its grant");
        let g = &w.entities[3];
        assert_eq!(
            (g.ai_state, g.state_ticks),
            (AiState::Alarm, 1),
            "unchanged"
        );
        assert!(g.target.is_none());
        assert_eq!(w.cursors.states, 3, "the cursor rests on him");
        w.validate().unwrap();
        // Deterministic across the snapshot; the next tick with the whole budget serves him
        // first and the transition is applied with its path.
        let mut w2 = crowd(0, 0);
        w2.restore(&snap).unwrap();
        w2.ai_tick_with(58);
        assert_eq!(w2.hashes(), w.hashes());
        w.ai_tick();
        let g = &w.entities[3];
        assert_eq!(
            (g.ai_state, g.state_ticks),
            (AiState::Alerted, ALERT_TIMEOUT_TICKS)
        );
        assert!(g.target.is_some() && g.gait == Gait::Run);
        assert_eq!(w.cursors.states, 0);
        w.validate().unwrap();

        // The charge on a heard run: the same budget rule leaves the patrolling soldier
        // untouched (no origin, no timer, no target) until the search is paid.
        let mut w = crowd(1, 1);
        let h = &mut w.entities[0];
        h.x = f(520);
        h.y = f(400);
        h.target = Some((f(700), f(400)));
        h.gait = Gait::Run;
        let g = &mut w.entities[1];
        g.x = f(450);
        g.y = f(400);
        g.facing256 = 128;
        w.validate().unwrap();
        let spent = w.ai_tick_with(2 + 2 + 2 + 50);
        assert_eq!(spent, 56);
        let g = &w.entities[1];
        assert_eq!(g.ai_state, AiState::Patrol);
        assert!(g.target.is_none() && g.alert_origin.is_none() && !g.heard);
        assert_eq!(g.state_ticks, 0);
        assert_eq!(w.cursors.states, 1);
        w.validate().unwrap();
        w.ai_tick();
        let g = &w.entities[1];
        assert_eq!(g.ai_state, AiState::Alerted);
        assert!(g.heard && g.target.is_some() && g.alert_origin == Some((f(450), f(400))));
        assert_eq!(w.cursors.states, 0);

        // The fight that ends on an unpaid return: the hero is moved beyond the break range
        // by hand and the soldier's post set far away; a small budget ends both fights (the
        // hero's first) but cannot pay the soldier's way back: he is `ReturnPending`, keeps
        // his post and searches again next tick.
        let mut w = fight();
        w.entities[0].x = f(700);
        w.entities[1].alert_origin = Some((f(100), f(100)));
        w.validate().unwrap();
        let spent = w.ai_tick_with(3 + 1 + 1 + 50);
        assert_eq!(spent, 55);
        let (p, g) = (&w.entities[0], &w.entities[1]);
        assert_eq!(p.ai_state, AiState::Patrol);
        assert_eq!(g.ai_state, AiState::ReturnPending);
        assert!(g.foe.is_none() && g.target.is_none() && g.state_ticks == 0);
        assert_eq!(g.alert_origin, Some((f(100), f(100))));
        assert_eq!(
            g.action,
            actions::FIGHT_IDLE,
            "the action follows next scan"
        );
        w.validate().unwrap();
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        assert!(json.contains("\"ai_state\":\"return_pending\""));
        let snap: Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = World::new(Scenario::Synthetic("corridor".into()), 3).unwrap();
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        // Still unpaid: unchanged, the cursor on him; paid: the return walk begins.
        let spent = w.ai_tick_with(3 + 1 + 1 + 50);
        assert_eq!(spent, 55);
        assert_eq!(w.entities[1].ai_state, AiState::ReturnPending);
        assert_eq!(w.cursors.states, 1);
        w2.ai_tick_with(55);
        assert_eq!(w2.hashes(), w.hashes());
        for world in [&mut w, &mut w2] {
            world.step(&[]);
            let g = &world.entities[1];
            assert_eq!(g.ai_state, AiState::Returning);
            assert!(g.target.is_some() && g.gait == Gait::Walk);
            assert_eq!(g.action, actions::ALERT_WALK);
            world.validate().unwrap();
        }
        assert_eq!(w.hashes(), w2.hashes());
        // A pending return without a post is refused.
        let mut bad = w.snapshot(None);
        bad.world.entities[1].ai_state = AiState::ReturnPending;
        bad.world.entities[1].target = None;
        bad.world.entities[1].alert_origin = None;
        assert!(w2.restore(&bad).unwrap_err().contains("origin"));
    }

    /// An open 1000x800 mission with two player characters at the west edge and one soldier
    /// east of them, facing them, running an empty level script (so the assumptions are
    /// recorded).
    fn two_heroes_one_guard() -> World {
        use crate::geom::Geometry;
        use crate::vm::tests::{class, program};
        use crate::world::{ActorSpec, MapInfo, MissionSpec, Scenario};
        let hero = |y: i32| ActorSpec {
            profile: "RobinHood".into(),
            team: Team::Player,
            x: 100,
            y,
            facing256: 0,
            patrol: vec![],
            program: vec![],
            active: true,
            hit_points: 100,
            knockout_resistance: 0,
        };
        let actors = vec![
            hero(100),
            hero(200),
            ActorSpec {
                profile: "Soldier A00".into(),
                team: Team::Enemy,
                x: 300,
                y: 150,
                facing256: 128,
                patrol: vec![],
                program: vec![],
                active: true,
                hit_points: 80,
                knockout_resistance: 0,
            },
        ];
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
            script: Some(program(vec![class("StartUp", 0, &[])], 2)),
            rails: Vec::new(),
            lenient_natives: false,
            starting_money: 0,
            assumptions: std::collections::BTreeSet::new(),
        };
        World::new_mission(Scenario::Mission("pair".into()), 4, &spec).unwrap()
    }

    /// Finding 7 of Codex review 10: two player characters ordered onto one soldier. The
    /// first to arrive fights him; the second waits at reach with his order (the multi-party
    /// policy, `attack_policy: multi_party`, since the measurements were one-on-one) and never
    /// damages a soldier who does not fight him back; when the first leaves, the soldier is
    /// free and the second takes him on. Every living pair names each other (`validate`
    /// refuses a soldier fought by two), through a snapshot and a replay of the recorded
    /// input events into a fresh world.
    #[test]
    fn two_heroes_on_one_soldier_fight_him_one_at_a_time() {
        let mut w = two_heroes_one_guard();
        let (a, b, g) = (w.entities[0].id, w.entities[1].id, w.entities[2].id);
        let mut events: Vec<Vec<InputEvent>> = Vec::new();
        let mut play = |w: &mut World, tick: Vec<InputEvent>| {
            w.step(&tick);
            events.push(tick);
        };
        let click_at = |x: i32, y: i32| {
            vec![
                InputEvent::PointerMove {
                    x256: x * 256,
                    y256: y * 256,
                },
                InputEvent::PointerDown {
                    button: Button::Left,
                },
                InputEvent::PointerUp {
                    button: Button::Left,
                },
            ]
        };
        // Select A, order the attack; select B, order the attack: both walk up.
        play(&mut w, click_at(100, 100));
        assert_eq!(w.selected, Some(a));
        play(&mut w, click_at(300, 150));
        play(&mut w, click_at(100, 200));
        assert_eq!(w.selected, Some(b));
        play(&mut w, click_at(300, 150));
        assert_eq!(w.entities[0].attack_target, Some(g));
        assert_eq!(w.entities[1].attack_target, Some(g));
        let mut ticks = 0;
        while w.entities[0].ai_state != AiState::Fighting {
            play(&mut w, vec![]);
            ticks += 1;
            assert!(ticks < 400, "{:?}", w.entities[0].ai_state);
        }
        for _ in 0..5 {
            play(&mut w, vec![]);
        }
        let (ea, eb, eg) = (&w.entities[0], &w.entities[1], &w.entities[2]);
        assert_eq!((ea.ai_state, ea.foe), (AiState::Fighting, Some(g)));
        assert_eq!((eg.ai_state, eg.foe), (AiState::Fighting, Some(a)));
        assert_eq!(eb.ai_state, AiState::Patrol, "waits");
        assert_eq!(eb.attack_target, Some(g), "with his order");
        assert!(eb.target.is_none() && eb.foe.is_none());
        assert!(
            Fixed::length(eb.x - eg.x, eb.y - eg.y) <= f(FIGHT_RANGE),
            "at reach"
        );
        let vm = w.vm.as_ref().unwrap();
        assert!(
            vm.assumptions
                .contains(&Assumption::AttackPolicy(AttackRule::MultiParty)),
            "{:?}",
            vm.assumptions
        );
        w.validate().unwrap();
        // The snapshot mid-fight, and the hostile form where both fight him.
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: crate::world::Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = two_heroes_one_guard();
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), w.hashes());
        let mut bad = w.snapshot(None);
        bad.world.entities[1].ai_state = AiState::Fighting;
        bad.world.entities[1].foe = Some(g);
        bad.world.entities[1].attack_target = None;
        let err = w2.restore(&bad).unwrap_err();
        assert!(err.contains("reciprocal"), "{err}");
        // B never hurts a soldier who does not fight him: over a swing's worth of ticks the
        // soldier's hit points only change by A's blows (none: click attacks never land).
        for _ in 0..(SOLDIER_SWING_TICKS + SWING_JITTER_TICKS + 2) {
            play(&mut w, vec![]);
            w2.step(&[]);
        }
        assert_eq!(w.entities[2].hp, 80);
        assert_eq!(w.entities[1].ai_state, AiState::Patrol);
        assert_eq!(w.hashes(), w2.hashes());
        // A leaves (selected by a click on him, then a right click on him): the soldier is
        // free and B, at reach with his order, takes him on the next tick.
        let (ax, ay) = (w.entities[0].x.round(), w.entities[0].y.round());
        play(&mut w, click_at(ax, ay));
        assert_eq!(w.selected, Some(a));
        play(
            &mut w,
            vec![
                InputEvent::PointerDown {
                    button: Button::Right,
                },
                InputEvent::PointerUp {
                    button: Button::Right,
                },
            ],
        );
        for _ in 0..3 {
            play(&mut w, vec![]);
        }
        let (ea, eb, eg) = (&w.entities[0], &w.entities[1], &w.entities[2]);
        assert_eq!(ea.ai_state, AiState::Patrol);
        assert!(ea.foe.is_none());
        assert_eq!((eb.ai_state, eb.foe), (AiState::Fighting, Some(g)));
        assert_eq!((eg.ai_state, eg.foe), (AiState::Fighting, Some(b)));
        assert!(eb.attack_target.is_none());
        w.validate().unwrap();
        // The replay: the recorded events into a fresh world reach the same state.
        let mut r = two_heroes_one_guard();
        for tick in &events {
            r.step(tick);
        }
        assert_eq!(r.hashes(), w.hashes());
        assert_eq!(
            r.vm.as_ref().unwrap().assumptions,
            w.vm.as_ref().unwrap().assumptions
        );
    }

    /// Finding 8 of Codex review 10: the drawn figure locks the nearest enemy soldier at the
    /// press (`World::figure_target`, kept while the button is held, snapshotted and hashed)
    /// and the release strikes that soldier even when another has come nearer meanwhile; a
    /// locked soldier who died before the release orders nothing.
    #[test]
    fn the_figure_locks_its_target_at_the_press() {
        use crate::world::Snapshot;
        let mut w = crowd(2, 1);
        let h = &mut w.entities[0];
        h.x = f(500);
        h.y = f(400);
        let near = &mut w.entities[1];
        near.x = f(560);
        near.y = f(400);
        near.ai_locked = true;
        let far = &mut w.entities[2];
        far.x = f(700);
        far.y = f(400);
        far.ai_locked = true;
        w.selected = Some(w.entities[0].id);
        w.validate().unwrap();
        let (near_id, far_id) = (w.entities[1].id, w.entities[2].id);
        let press = |w: &mut World| {
            w.step(&[
                InputEvent::PointerMove {
                    x256: 520 * 256,
                    y256: 420 * 256,
                },
                InputEvent::PointerDown {
                    button: Button::Left,
                },
            ]);
        };
        let release = |w: &mut World| {
            w.step(&[
                InputEvent::PointerMove {
                    x256: 600 * 256,
                    y256: 400 * 256,
                },
                InputEvent::PointerUp {
                    button: Button::Left,
                },
            ]);
        };
        press(&mut w);
        assert_eq!(w.figure_target, Some(near_id));
        assert!(w.press.is_some());
        assert_eq!(w.observe(false).figure_target, Some(near_id));
        w.validate().unwrap();
        // The lock is state: hashed, restored.
        let h = w.hashes();
        let mut v = w.clone();
        v.figure_target = Some(far_id);
        assert_ne!(v.hashes().get("world"), h.get("world"));
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = crowd(0, 0);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.hashes(), h);
        // The far soldier steps nearer during the gesture (by hand, in both worlds).
        for world in [&mut w, &mut w2] {
            world.entities[2].x = f(530);
            world.step(&[]);
            assert_eq!(world.figure_target, Some(near_id), "kept while held");
            release(world);
            let p = &world.entities[0];
            assert_eq!(
                p.attack_target,
                Some(near_id),
                "the locked one, not the nearest"
            );
            assert_eq!(p.figure, Some(Figure::ForwardStroke));
            assert!(world.figure_target.is_none() && world.press.is_none());
            world.validate().unwrap();
        }
        assert_eq!(w.hashes(), w2.hashes());
        // A locked soldier who dies before the release: the figure orders nothing.
        let mut w = crowd(2, 1);
        w.entities[0].x = f(500);
        w.entities[0].y = f(400);
        w.entities[1].x = f(560);
        w.entities[1].y = f(400);
        w.entities[1].ai_locked = true;
        w.entities[2].x = f(700);
        w.entities[2].y = f(400);
        w.entities[2].ai_locked = true;
        w.selected = Some(w.entities[0].id);
        press(&mut w);
        assert_eq!(w.figure_target, Some(w.entities[1].id));
        let d = &mut w.entities[1];
        d.alive = false;
        d.hp = 0;
        d.ai_state = AiState::Dead;
        d.ai_locked = false;
        w.validate().unwrap();
        release(&mut w);
        let p = &w.entities[0];
        assert!(p.attack_target.is_none() && p.figure.is_none() && p.target.is_none());
        // The lock needs a held button and an enemy soldier.
        let mut bad = w.snapshot(None);
        bad.world.figure_target = Some(w.entities[2].id);
        assert!(w.restore(&bad).unwrap_err().contains("figure target"));
        let mut bad = w.snapshot(None);
        bad.world.press = Some((f(1), f(1)));
        bad.world.figure_target = Some(w.entities[0].id);
        assert!(w.restore(&bad).unwrap_err().contains("figure target"));
        let mut ok = w.snapshot(None);
        ok.world.press = Some((f(1), f(1)));
        ok.world.figure_target = Some(w.entities[2].id);
        w.restore(&ok).unwrap();
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

    /// The largest accepted world of soldiers and no player character costs exactly three
    /// units per entity (the pre-index pass, the inspection and the state transition) and
    /// finishes its walks every tick.
    #[test]
    fn perception_charges_every_inspected_entity_with_no_player_to_perceive() {
        let n = crate::world::MAX_ENTITIES;
        let mut w = crowd(n, 0);
        assert_eq!(w.entities.len(), n);
        let spent = w.ai_tick();
        assert_eq!(spent, 3 * n as u64);
        assert!(spent < SIM_WORK_PER_TICK);
        assert_eq!(
            w.cursors,
            crate::world::SimCursors::default(),
            "the walks completed"
        );
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
        // Full budget: pre-index n, inspect every soldier and test 3 players each, then one
        // transition per human.
        assert_eq!(w.ai_tick(), (2 * n + guards * (1 + players)) as u64);
        assert_eq!(w.cursors.perception, 0);
        assert_eq!(w.cursors.states, 0);
        // n (pre-index) + 10 soldiers at 1 + 3 each + 3 = n + 43: the eleventh soldier (entity
        // 13) is where the budget runs out, on his third player; the transitions get nothing
        // and their cursor stays at the first human.
        let spent = w.ai_tick_with((n + 43) as u64);
        assert_eq!(spent, (n + 43) as u64);
        assert_eq!(w.cursors.perception, 13);
        assert_eq!(w.cursors.states, 0);
        w.validate().unwrap();
        let h = w.hashes();
        let mut v = w.clone();
        v.cursors.perception = 0;
        assert_ne!(
            v.hashes().get("world"),
            h.get("world"),
            "the cursor is hashed"
        );
        let json = serde_json::to_string(&w.snapshot(None)).unwrap();
        let snap: Snapshot = serde_json::from_str(&json).unwrap();
        let mut w2 = crowd(guards, players);
        w2.restore(&snap).unwrap();
        assert_eq!(w2.cursors.perception, 13);
        assert_eq!(w2.hashes(), h);
        let mut bad = w.snapshot(None);
        bad.world.cursors.perception = n as u32;
        assert!(w2.restore(&bad).unwrap_err().contains("cursor"));
        // The next scan starts at 13: with the budget of the rest of the round (the 30
        // soldiers 13..=42 at 4 each) plus the pre-index pass and one more unit, it wraps,
        // inspects the first soldier (entity 3) and stops on his first player.
        let rest = (n + 30 * 4 + 1) as u64;
        assert_eq!(w.ai_tick_with(rest), rest);
        assert_eq!(w.cursors.perception, 3);
        // A budget too small for the pre-index pass perceives nothing and moves nothing.
        w.cursors.perception = 7;
        assert_eq!(w.ai_tick_with(5), 5);
        assert_eq!(w.cursors.perception, 7);
        // Same inputs from the restored world: same states and hashes.
        w2.ai_tick_with(rest);
        w2.cursors.perception = 7;
        w2.ai_tick_with(5);
        for _ in 0..3 {
            w.step(&[]);
            w2.step(&[]);
        }
        assert_eq!(w.hashes(), w2.hashes());
        assert_eq!(w.cursors.perception, 0, "a full tick completes the scan");
    }

    /// A mass alert: hundreds of soldiers hear the running hero at once and all want a path on
    /// the same tick. The path searches share the tick's budget: with the real budget every one
    /// of them is planned within the bound, with a small budget only as many as it pays for and
    /// the rest stand alerted and re-plan on the following ticks; both are deterministic across
    /// a snapshot.
    #[test]
    fn mass_alerts_share_one_navigation_budget() {
        let guards = 600;
        // Soldiers in a 40 x 15 block behind the hero (facing away from him, beyond the rear
        // radius), who runs east: all within the noise radius, none with him in their cone.
        let mut w = crowd(guards, 1);
        w.entities[0].x = f(560);
        w.entities[0].y = f(400);
        for (k, e) in w.entities.iter_mut().skip(1).enumerate() {
            e.x = f(420 + (k % 40) as i32 * 2);
            e.y = f(370 + (k / 40) as i32 * 4);
            e.facing256 = 128;
        }
        w.selected = Some(w.entities[0].id);
        w.validate().unwrap();
        // The hero runs east (the order set by hand so that the tick of the mass alert can be
        // stepped with an explicit budget below).
        let hero = &mut w.entities[0];
        hero.target = Some((f(760), f(400)));
        hero.gait = Gait::Run;
        assert!(
            w.entities
                .iter()
                .skip(1)
                .all(|e| e.ai_state == AiState::Patrol)
        );
        let snap = w.snapshot(None);
        // The tick of the noise with the real budget: every soldier hears him and charges at
        // once, alerted and running, the work stayed within the bound.
        let mut full = w.clone();
        let spent = full.ai_tick();
        assert!(spent <= SIM_WORK_PER_TICK);
        assert!(
            full.entities
                .iter()
                .skip(1)
                .all(|e| e.ai_state == AiState::Alerted && e.heard && e.target.is_some())
        );
        // A budget that covers the perception and a few transitions with their searches: the
        // soldiers the transition walk did not reach stay in their normal state this tick
        // (they hear him again next tick, and the cursor gives them the first turn), the
        // ones it reached are alerted, with or without a path.
        let n = w.entities.len() as u64;
        let small = 2 * n + guards as u64 + 3000;
        let spent = w.ai_tick_with(small);
        assert!(spent <= small && spent > 2 * n + guards as u64);
        let alerted = w
            .entities
            .iter()
            .skip(1)
            .filter(|e| e.ai_state == AiState::Alerted)
            .count();
        assert!(
            alerted > 0 && alerted < guards,
            "{alerted} of {guards} alerted"
        );
        assert!(
            w.entities
                .iter()
                .skip(1)
                .all(|e| matches!(e.ai_state, AiState::Alerted | AiState::Patrol))
        );
        assert_ne!(
            w.cursors.states, 0,
            "the transition walk resumes where it stopped"
        );
        w.validate().unwrap();
        // Thirty ticks later the hero has run 50 px east, out of everyone's reach, and every
        // starved soldier re-planned (an alerted soldier within punch reach of the noise
        // stands where he is).
        for _ in 0..30 {
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
        for _ in 0..30 {
            w2.step(&[]);
        }
        assert_eq!(w2.hashes(), w.hashes());
    }
    /// An open 1000x800 mission of `guards` idle soldiers, each with a waypoint program that
    /// walks to the far corner and stops, at distinct spots; no player character.
    fn idle_patrol_world(guards: usize) -> World {
        use crate::geom::Geometry;
        use crate::world::{ActorSpec, Instruction, MapInfo, MissionSpec, Scenario};
        let actors = (0..guards)
            .map(|i| ActorSpec {
                profile: "Soldier A00".into(),
                team: Team::Enemy,
                x: 100 + (i % 400) as i32,
                y: 100 + (i / 400 % 400) as i32,
                facing256: 0,
                patrol: vec![],
                program: vec![Instruction::GoTo { x: 900, y: 700 }, Instruction::Stop],
                active: true,
                hit_points: 80,
                knockout_resistance: 0,
            })
            .collect();
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
        World::new_mission(Scenario::Mission("patrol".into()), 2, &spec).unwrap()
    }

    /// Finding 4 of Codex review 8: thousands of idle guards whose programs all want a path
    /// on the same tick draw their searches from the one simulation budget: the tick's work
    /// stays within `SIM_WORK_PER_TICK`; with a small budget only as many are planned as it
    /// pays for, the program cursor marks the next one, the rest keep their instruction and
    /// are planned on the following ticks from the cursor on; all of it deterministic across
    /// a snapshot.
    #[test]
    fn mass_idle_patrols_share_the_simulation_budget() {
        let guards = 3000;
        let mut w = idle_patrol_world(guards);
        let snap = w.snapshot(None);
        let mut full = w.clone();
        let spent = full.sim_tick_with(SIM_WORK_PER_TICK);
        assert!(spent <= SIM_WORK_PER_TICK);
        let planned = full.entities.iter().filter(|e| e.target.is_some()).count();
        assert!(planned > 0);
        // A small budget: the pre-index, the (empty) stealth walks, then a few searches.
        let small = 3 * guards as u64 + 20_000;
        let spent = w.sim_tick_with(small);
        assert!(spent <= small);
        let planned = w.entities.iter().filter(|e| e.target.is_some()).count();
        assert!(
            planned > 0 && planned < guards,
            "{planned} of {guards} planned"
        );
        assert_ne!(
            w.cursors.programs, 0,
            "the program walk resumes where it stopped"
        );
        assert!(
            w.entities.iter().all(|e| e.pc == 0 && e.wait_ticks == 0),
            "an unpaid search skips no instruction"
        );
        w.validate().unwrap();
        // The cursor is authoritative.
        let h = w.hashes();
        let mut v = w.clone();
        v.cursors.programs = 0;
        assert_ne!(v.hashes().get("world"), h.get("world"));
        // Over the following ticks every guard is planned (in cursor order, none twice
        // before the round completes) and the cursor rests at 0 once the walk fits.
        let mut w2 = idle_patrol_world(0);
        w2.restore(&snap).unwrap();
        w2.sim_tick_with(small);
        assert_eq!(w2.hashes(), h);
        let mut ticks = 0;
        while w.entities.iter().any(|e| e.target.is_none() && e.pc == 0) {
            w.step(&[]);
            w2.step(&[]);
            ticks += 1;
            assert!(ticks < 50, "starved guards");
        }
        assert_eq!(w.hashes(), w2.hashes());
        assert_eq!(w.cursors.programs, 0);
    }

    /// Player characters attacking as many soldiers: the victim lookup goes through the
    /// tick's index (no scan per attacker), one unit per attacker, the searches from the
    /// shared budget; a small budget serves a prefix from the attack cursor and the rest
    /// on the following ticks, deterministically across a snapshot.
    #[test]
    fn mass_attack_orders_are_indexed_and_cursor_resumed() {
        let n = 300;
        let mut w = crowd(n, n);
        // The soldiers do not perceive (locked); every player character attacks his own.
        for i in 0..n {
            w.entities[n + i].ai_locked = true;
            w.entities[i].attack_target = Some(w.entities[n + i].id);
        }
        w.validate().unwrap();
        let snap = w.snapshot(None);
        let total = w.entities.len() as u64;
        let mut full = w.clone();
        let spent = full.ai_tick();
        assert!(spent <= SIM_WORK_PER_TICK);
        assert!(
            full.entities[..n].iter().all(|e| e.target.is_some()),
            "every attacker walks into reach"
        );
        assert_eq!(full.cursors, crate::world::SimCursors::default());
        // Pre-index, no perceivers, the humans' transitions (the locked soldiers are not
        // among them), then the attackers: a budget for a handful of searches.
        let small = 2 * total + 5000;
        let spent = w.ai_tick_with(small);
        assert!(spent <= small);
        let walking = w.entities[..n]
            .iter()
            .filter(|e| e.target.is_some())
            .count();
        assert!(walking > 0 && walking < n, "{walking} of {n} planned");
        assert_ne!(w.cursors.attacks, 0);
        assert!(
            w.entities[..n].iter().all(|e| e.attack_target.is_some()),
            "an unpaid order is kept"
        );
        w.validate().unwrap();
        let mut w2 = crowd(0, 0);
        w2.restore(&snap).unwrap();
        w2.ai_tick_with(small);
        assert_eq!(w2.hashes(), w.hashes());
        for _ in 0..3 {
            w.step(&[]);
            w2.step(&[]);
        }
        assert_eq!(w.hashes(), w2.hashes());
        assert!(w.entities[..n].iter().all(|e| e.target.is_some()));
        assert_eq!(w.cursors.attacks, 0);
    }

    /// Finding 4 of Codex review 9: the largest accepted snapshot split between player
    /// characters and soldiers (2^15 each, every pair tested: 2^30 perception pairs a round
    /// against a 2^24 budget) cannot starve the later phases. Perception spends its quota and
    /// stops on a cursor that advances every tick; the state transitions still run over every
    /// human every tick (every energy timer counts down), the attack orders and the waypoint
    /// programs plan a bounded, growing number of walks per tick from their cursors, and the
    /// movement, animation and action scans complete every tick; all of it deterministic
    /// across a clone stepped alongside.
    #[test]
    fn every_phase_keeps_its_quota_in_the_largest_hostile_snapshot() {
        use crate::world::{
            Instruction, MAX_ENTITIES, SIM_QUOTA_PERCEPTION, SIM_WORK_PER_TICK, SimCursors,
        };
        let half = MAX_ENTITIES / 2;
        let mut w = crowd(half, half);
        assert_eq!(w.entities.len(), MAX_ENTITIES);
        w.programs.push(vec![
            Instruction::GoTo { x: 900, y: 700 },
            Instruction::Stop,
        ]);
        for i in 0..MAX_ENTITIES {
            let victim = w.entities[(half + i) % MAX_ENTITIES].id;
            let e = &mut w.entities[i];
            e.energy = ENERGY_MAX - 1;
            e.energy_ticks = 5;
            if i < half {
                e.attack_target = Some(victim);
            } else {
                e.program = Some(0);
            }
        }
        w.validate().unwrap();
        let start: Vec<(Fixed, Fixed)> = w.entities.iter().map(|e| (e.x, e.y)).collect();
        let mut twin = w.clone();
        let planned = |w: &World, range: std::ops::Range<usize>| {
            w.entities[range]
                .iter()
                .filter(|e| e.target.is_some())
                .count()
        };
        let mut attacks_before = 0;
        let mut programs_before = 0;
        let mut perception_before = 0;
        for tick in 1..=3u32 {
            let spent = w.sim_tick_with(SIM_WORK_PER_TICK);
            twin.sim_tick_with(SIM_WORK_PER_TICK);
            assert!(spent <= SIM_WORK_PER_TICK);
            assert!(
                spent > SIM_QUOTA_PERCEPTION,
                "perception spent its whole quota"
            );
            let c: SimCursors = w.cursors;
            // Perception: cut short, the cursor moves through the soldiers (entities
            // half..) by a bounded number each tick.
            assert!(c.perception as usize >= half, "{c:?}");
            assert!(
                c.perception > perception_before,
                "no progress at tick {tick}: {c:?}"
            );
            assert!(
                (c.perception as usize - half.max(perception_before as usize)) as u64
                    <= SIM_QUOTA_PERCEPTION / half as u64 + 2,
                "{c:?}"
            );
            perception_before = c.perception;
            // The state transitions ran over every human: every timer counted down.
            assert_eq!(c.states, 0, "{c:?}");
            assert!(
                w.entities.iter().all(|e| e.energy_ticks == 5 - tick),
                "a human's timer was starved at tick {tick}"
            );
            // Attack orders and program walks: bounded progress, growing every tick.
            let attacks = planned(&w, 0..half);
            let programs = planned(&w, half..MAX_ENTITIES);
            assert!(attacks > attacks_before, "attacks {attacks} at tick {tick}");
            assert!(
                programs > programs_before,
                "programs {programs} at tick {tick}"
            );
            attacks_before = attacks;
            programs_before = programs;
            // Movement, animation and the action scan complete every tick.
            assert_eq!((c.movement, c.animation, c.actions), (0, 0, 0), "{c:?}");
            assert_eq!(w.hashes(), twin.hashes(), "tick {tick}");
        }
        assert!(
            attacks_before < half && programs_before < half,
            "bounded per tick"
        );
        // The movers moved: every planned walk advanced its entity (the movement phase runs
        // after the phases that plan, in the same tick).
        assert!(
            w.entities
                .iter()
                .zip(&start)
                .filter(|(e, _)| e.target.is_some())
                .all(|(e, &p)| (e.x, e.y) != p)
        );
        w.validate().unwrap();
    }

    /// Where a phase resumes after its grant ran out on the `k`-th entity of its walk: on it,
    /// unless it was the first one served with the phase's whole quota, which the walk then
    /// moves past (one entity too expensive for a quota blocks nobody); a lone entity keeps
    /// the cursor at 0.
    #[test]
    fn a_phase_resumes_on_the_starved_entity_unless_it_alone_exhausted_a_whole_quota() {
        let order = [3usize, 5, 7];
        assert_eq!(resume_at(&order, 2, true), 7);
        assert_eq!(resume_at(&order, 1, false), 5);
        assert_eq!(resume_at(&order, 0, false), 3);
        assert_eq!(resume_at(&order, 0, true), 5);
        assert_eq!(resume_at(&[3], 0, true), 0);
        assert_eq!(rotated(&order, 6), vec![7, 3, 5]);
        assert_eq!(rotated(&order, 0), vec![3, 5, 7]);
    }
}
