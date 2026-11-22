//! Game simulation. Nothing in here touches the window, GPU or audio device:
//! the world is advanced with `World::update` and reports what happened
//! through `World::events`, which keeps it testable headless.

mod rng;
#[cfg(test)]
mod tests;

use std::f32::consts::{FRAC_PI_2, TAU};

use macroquad::math::{Vec2, vec2};

use crate::palette::{self, Rgb};
pub use rng::Rng;

/// Logical screen size; the window is scaled to fit.
pub const W: f32 = 1600.0;
pub const H: f32 = 900.0;

/// Playfield frame.
pub const LEFT: f32 = 32.0;
pub const RIGHT: f32 = 1568.0;
pub const TOP: f32 = 60.0;
pub const BOTTOM: f32 = 868.0;

pub const PLAYER_Y: f32 = 814.0;
pub const PLAYER_R: f32 = 18.0;
pub const ENEMY_R: f32 = 20.0;
const SHOT_R: f32 = 5.0;
const BOMB_R: f32 = 7.0;
const COURIER_R: f32 = 26.0;
const POWERUP_R: f32 = 16.0;

pub const COLS: usize = 10;
pub const ROWS: usize = 4;
const SLOT_DX: f32 = 92.0;
const SLOT_DY: f32 = 62.0;
const EDGE_MARGIN: f32 = 44.0;
const DESCENT: f32 = 22.0;

const START_LIVES: u32 = 3;
const MAX_LIVES: u32 = 5;
const CHARGE_TIME: f32 = 1.1;
const OVERCHARGE_TIME: f32 = 0.38;
const MIN_CHARGE: f32 = 0.09;
const LANCE_PIERCE: u8 = 3;
const MAX_SHOTS: usize = 8;
const MAX_BOMBS: usize = 16;
const MAX_PARTICLES: usize = 4000;
const COMBO_WINDOW: f32 = 2.2;
const MAX_MULTIPLIER: u32 = 8;
const DIVE_WINDUP: f32 = 0.4;
const TRIPLE_TIME: f32 = 12.0;
const OVERCHARGE_DURATION: f32 = 10.0;

#[derive(Clone, Copy, Default)]
pub struct Input {
    pub left: bool,
    pub right: bool,
    /// Fire button is held.
    pub fire: bool,
    /// Fire button went down since the last update.
    pub fire_pressed: bool,
}

/// Things the outside world may want to react to (sound, mostly).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shoot { charge: f32 },
    Explosion,
    PlayerHit,
    BombShot,
    Pickup,
    ShieldBreak,
    WaveClear,
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Green,
    Red,
    Yellow,
    Cyan,
}

impl Kind {
    /// Rows from the top of the formation down, as in the original.
    fn for_row(row: usize) -> Kind {
        [Kind::Green, Kind::Red, Kind::Yellow, Kind::Cyan][row % ROWS]
    }

    pub fn points(self) -> u32 {
        match self {
            Kind::Green => 50,
            Kind::Red => 30,
            Kind::Yellow => 20,
            Kind::Cyan => 10,
        }
    }

    pub fn color(self) -> Rgb {
        match self {
            Kind::Green => palette::GREEN,
            Kind::Red => palette::RED,
            Kind::Yellow => palette::YELLOW,
            Kind::Cyan => palette::CYAN,
        }
    }

    /// The upper rows grow armour in later waves.
    fn hp(self, level: u32) -> u8 {
        match self {
            Kind::Green if level >= 3 => 2,
            Kind::Red if level >= 5 => 2,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Formation,
    /// Broke formation and is swooping at the player.
    Diving { t: f32, bombed: bool },
    /// Flew off the bottom and is heading back to its slot from the top.
    Returning,
}

pub struct Enemy {
    pub kind: Kind,
    pub col: usize,
    pub row: usize,
    pub pos: Vec2,
    pub hp: u8,
    pub mode: Mode,
    pub phase: f32,
    pub flash: f32,
    /// Recent positions, newest first; only filled while out of formation.
    pub trail: Vec<Vec2>,
    trail_tick: u8,
}

pub struct Fleet {
    pub enemies: Vec<Enemy>,
    /// Position of the top-left formation slot.
    pub origin: Vec2,
    pub dir: f32,
    pub speed: f32,
    drop_left: f32,
    dive_timer: f32,
}

impl Fleet {
    fn new(level: u32, rng: &mut Rng) -> Fleet {
        let origin = vec2(
            (W - (COLS - 1) as f32 * SLOT_DX) / 2.0,
            128.0 + (level.min(5) - 1) as f32 * 10.0,
        );
        let mut enemies = Vec::with_capacity(COLS * ROWS);
        for row in 0..ROWS {
            for col in 0..COLS {
                let kind = Kind::for_row(row);
                enemies.push(Enemy {
                    kind,
                    col,
                    row,
                    pos: origin + vec2(col as f32 * SLOT_DX, row as f32 * SLOT_DY),
                    hp: kind.hp(level),
                    mode: Mode::Formation,
                    phase: rng.range(0.0, TAU),
                    flash: 0.0,
                    trail: Vec::new(),
                    trail_tick: 0,
                });
            }
        }
        Fleet {
            enemies,
            origin,
            dir: 1.0,
            speed: 0.0,
            drop_left: 0.0,
            dive_timer: rng.range(4.0, 6.0),
        }
    }
}

pub struct Player {
    pub x: f32,
    pub vx: f32,
    /// 0..1; decides shot speed, and a full charge fires a piercing lance.
    pub charge: f32,
    pub alive: bool,
    pub invulnerable: f32,
    pub shield: bool,
    pub triple: f32,
    pub overcharge: f32,
}

#[derive(Clone, Copy)]
pub struct Shot {
    pub pos: Vec2,
    pub vel: Vec2,
    pub charge: f32,
    pub lance: bool,
    pierce: u8,
}

pub struct Bomb {
    pub pos: Vec2,
    pub vel: Vec2,
    pub zigzag: bool,
    pub t: f32,
    base_x: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Power {
    Triple,
    Shield,
    Overcharge,
}

pub struct PowerUp {
    pub pos: Vec2,
    pub power: Power,
}

/// Bonus ship that crosses above the formation.
pub struct Courier {
    pub pos: Vec2,
    pub dir: f32,
}

pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub max_life: f32,
    pub color: Rgb,
    pub size: f32,
}

pub struct Popup {
    pub pos: Vec2,
    pub text: String,
    pub age: f32,
    pub color: Rgb,
}

pub const POPUP_LIFE: f32 = 0.9;

/// Expanding ring that pushes the background grid around.
pub struct Shock {
    pub pos: Vec2,
    pub age: f32,
    pub strength: f32,
}

pub const SHOCK_LIFE: f32 = 1.3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// "WAVE N" banner; the fleet holds still, the player may move.
    WaveIntro(f32),
    Playing,
    /// The player's ship was destroyed and the next one is on its way.
    Respawning(f32),
    GameOver(f32),
}

pub struct World {
    rng: Rng,
    pub time: f32,
    pub phase: Phase,
    /// Set once the game-over sequence has played out.
    pub finished: bool,
    pub level: u32,
    pub lives: u32,
    pub score: u32,
    pub combo: u32,
    pub combo_timer: f32,
    pub shake: f32,
    pub player: Player,
    pub fleet: Fleet,
    pub shots: Vec<Shot>,
    pub bombs: Vec<Bomb>,
    pub powerups: Vec<PowerUp>,
    pub courier: Option<Courier>,
    courier_timer: f32,
    pub particles: Vec<Particle>,
    pub popups: Vec<Popup>,
    pub shocks: Vec<Shock>,
    /// Drained by the caller every frame.
    pub events: Vec<Event>,
}

pub fn circles_overlap(a: Vec2, ar: f32, b: Vec2, br: f32) -> bool {
    a.distance_squared(b) < (ar + br) * (ar + br)
}

impl World {
    pub fn new(seed: u64) -> World {
        let mut rng = Rng::new(seed);
        let fleet = Fleet::new(1, &mut rng);
        let courier_timer = rng.range(14.0, 22.0);
        World {
            rng,
            time: 0.0,
            phase: Phase::WaveIntro(1.8),
            finished: false,
            level: 1,
            lives: START_LIVES,
            score: 0,
            combo: 0,
            combo_timer: 0.0,
            shake: 0.0,
            player: Player {
                x: W / 2.0,
                vx: 0.0,
                charge: 0.0,
                alive: true,
                invulnerable: 0.0,
                shield: false,
                triple: 0.0,
                overcharge: 0.0,
            },
            fleet,
            shots: Vec::new(),
            bombs: Vec::new(),
            powerups: Vec::new(),
            courier: None,
            courier_timer,
            particles: Vec::new(),
            popups: Vec::new(),
            shocks: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn player_pos(&self) -> Vec2 {
        vec2(self.player.x, PLAYER_Y)
    }

    /// Score multiplier earned by chaining kills.
    pub fn multiplier(&self) -> u32 {
        (1 + self.combo / 4).min(MAX_MULTIPLIER)
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        self.time += dt;
        self.update_effects(dt);

        match self.phase {
            Phase::GameOver(left) => {
                self.phase = Phase::GameOver(left - dt);
                self.finished = left - dt <= 0.0;
            }
            Phase::Respawning(left) => {
                if left - dt <= 0.0 {
                    self.player.alive = true;
                    self.player.x = W / 2.0;
                    self.player.vx = 0.0;
                    self.player.charge = 0.0;
                    self.player.invulnerable = 2.0;
                    self.phase = Phase::Playing;
                } else {
                    self.phase = Phase::Respawning(left - dt);
                }
            }
            Phase::WaveIntro(left) => {
                self.update_player(dt, input, false);
                self.phase = if left - dt <= 0.0 {
                    Phase::Playing
                } else {
                    Phase::WaveIntro(left - dt)
                };
            }
            Phase::Playing => {
                self.update_player(dt, input, true);
                self.update_fleet(dt);
                self.update_courier(dt);
                self.update_projectiles(dt);
                self.collide_shots();
                self.collide_player();
                self.check_invasion();
                self.check_wave_clear();
                if self.combo_timer > 0.0 {
                    self.combo_timer -= dt;
                    if self.combo_timer <= 0.0 {
                        self.combo = 0;
                    }
                }
            }
        }
    }

    fn update_effects(&mut self, dt: f32) {
        let drag = (1.0 - 1.9 * dt).max(0.0);
        for p in &mut self.particles {
            p.pos += p.vel * dt;
            p.vel *= drag;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
        for p in &mut self.popups {
            p.age += dt;
            p.pos.y -= 46.0 * dt;
        }
        self.popups.retain(|p| p.age < POPUP_LIFE);
        for s in &mut self.shocks {
            s.age += dt;
        }
        self.shocks.retain(|s| s.age < SHOCK_LIFE);
        self.shake = (self.shake - 2.6 * dt).max(0.0);
    }

    fn update_player(&mut self, dt: f32, input: &Input, can_fire: bool) {
        let p = &mut self.player;
        if !p.alive {
            return;
        }
        let dir = (input.right as i8 - input.left as i8) as f32;
        let accel = 5200.0 * dt;
        p.vx += (dir * 540.0 - p.vx).clamp(-accel, accel);
        p.x += p.vx * dt;
        let (lo, hi) = (LEFT + 34.0, RIGHT - 34.0);
        if p.x < lo || p.x > hi {
            p.x = p.x.clamp(lo, hi);
            p.vx = 0.0;
        }
        let charge_time = if p.overcharge > 0.0 {
            OVERCHARGE_TIME
        } else {
            CHARGE_TIME
        };
        p.charge = (p.charge + dt / charge_time).min(1.0);
        p.invulnerable = (p.invulnerable - dt).max(0.0);
        p.triple = (p.triple - dt).max(0.0);
        p.overcharge = (p.overcharge - dt).max(0.0);

        // Tap to fire whatever charge there is; hold to release full lances.
        let wants_fire = input.fire_pressed || (input.fire && p.charge >= 1.0);
        if can_fire && wants_fire && p.charge >= MIN_CHARGE && self.shots.len() < MAX_SHOTS {
            self.fire();
        }
    }

    fn fire(&mut self) {
        let charge = self.player.charge;
        let lance = charge >= 1.0;
        let speed = 420.0 + 1100.0 * charge;
        let muzzle = vec2(self.player.x, PLAYER_Y - 26.0);
        let angles: &[f32] = if self.player.triple > 0.0 {
            &[-0.2, 0.0, 0.2]
        } else {
            &[0.0]
        };
        for &a in angles {
            self.shots.push(Shot {
                pos: muzzle,
                vel: vec2(a.sin(), -a.cos()) * speed,
                charge,
                lance,
                pierce: if lance { LANCE_PIERCE } else { 0 },
            });
        }
        self.player.charge = 0.0;
        let color = if lance { palette::CYAN } else { palette::AMBER };
        self.burst(muzzle, color, 4 + (charge * 10.0) as usize, 60.0, 260.0, Some((0.0, 0.5)));
        if lance {
            self.shake = self.shake.max(0.18);
        }
        self.events.push(Event::Shoot { charge });
    }

    fn update_fleet(&mut self, dt: f32) {
        let level = self.level as f32;
        let thinned = 1.0 - self.fleet.enemies.len() as f32 / (COLS * ROWS) as f32;
        // The fewer that remain, the faster they march.
        let speed = (46.0 + 16.0 * level + 380.0 * thinned.powf(1.6)).min(620.0);
        let f = &mut self.fleet;
        f.speed = speed;

        if f.drop_left > 0.0 {
            let d = f.drop_left.min(150.0 * dt);
            f.origin.y += d;
            f.drop_left -= d;
        } else {
            f.origin.x += f.dir * speed * dt;
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for e in &f.enemies {
                let x = f.origin.x + e.col as f32 * SLOT_DX;
                lo = lo.min(x);
                hi = hi.max(x);
            }
            if f.dir > 0.0 && hi >= RIGHT - EDGE_MARGIN {
                f.dir = -1.0;
                f.drop_left = DESCENT;
            } else if f.dir < 0.0 && lo <= LEFT + EDGE_MARGIN {
                f.dir = 1.0;
                f.drop_left = DESCENT;
            }
        }

        let origin = f.origin;
        let player = vec2(self.player.x, PLAYER_Y);
        let time = self.time;
        let mut aimed_bombs = Vec::new();
        for e in &mut f.enemies {
            let home = origin + vec2(e.col as f32 * SLOT_DX, e.row as f32 * SLOT_DY);
            e.flash = (e.flash - 5.0 * dt).max(0.0);
            match e.mode {
                Mode::Formation => {
                    e.pos = home + vec2(0.0, (time * 2.0 + e.phase).sin() * 3.0);
                    e.trail.clear();
                    continue;
                }
                Mode::Diving { t, bombed } => {
                    let t = t + dt;
                    let mut bombed = bombed;
                    if t < DIVE_WINDUP {
                        // rear up before the plunge so the player gets a tell
                        e.pos.y -= 80.0 * dt;
                    } else {
                        let homing = ((player.x - e.pos.x) * 2.2).clamp(-260.0, 260.0);
                        let weave = (t * 4.0 + e.phase).sin() * 150.0;
                        e.pos += vec2(homing + weave, 300.0 + 18.0 * level) * dt;
                        if !bombed && e.pos.y > 400.0 && e.pos.y < PLAYER_Y - 180.0 {
                            bombed = true;
                            aimed_bombs.push(e.pos);
                        }
                    }
                    e.mode = if e.pos.y > BOTTOM + 40.0 {
                        e.pos = vec2(home.x, TOP - 30.0);
                        e.trail.clear();
                        Mode::Returning
                    } else {
                        Mode::Diving { t, bombed }
                    };
                }
                Mode::Returning => {
                    let to_home = home - e.pos;
                    let step = 420.0 * dt;
                    if to_home.length() <= step.max(6.0) {
                        e.pos = home;
                        e.mode = Mode::Formation;
                    } else {
                        e.pos += to_home.normalize() * step;
                    }
                }
            }
            e.trail_tick = (e.trail_tick + 1) % 3;
            if e.trail_tick == 0 {
                e.trail.insert(0, e.pos);
                e.trail.truncate(18);
            }
        }

        for pos in aimed_bombs {
            let vel = (player - pos).normalize_or_zero() * 280.0;
            self.bombs.push(Bomb {
                pos,
                vel,
                zigzag: false,
                t: 0.0,
                base_x: pos.x,
            });
        }

        // Only the lowest alien in a column has a clear shot.
        let rate = (1.0 + 0.35 * level) * (1.0 + 0.8 * thinned);
        if self.bombs.len() < MAX_BOMBS && self.rng.chance(rate * dt) {
            let mut lowest: [Option<usize>; COLS] = [None; COLS];
            for (i, e) in self.fleet.enemies.iter().enumerate() {
                if e.mode != Mode::Formation {
                    continue;
                }
                match lowest[e.col] {
                    Some(j) if self.fleet.enemies[j].row >= e.row => {}
                    _ => lowest[e.col] = Some(i),
                }
            }
            let candidates: Vec<usize> = lowest.iter().flatten().copied().collect();
            if !candidates.is_empty() {
                let pos = self.fleet.enemies[candidates[self.rng.below(candidates.len())]].pos;
                let speed = 170.0 + self.rng.range(0.0, 90.0 + 25.0 * level);
                let zigzag = self.level >= 2 && self.rng.chance(0.3);
                self.bombs.push(Bomb {
                    pos: pos + vec2(0.0, ENEMY_R),
                    vel: vec2(0.0, speed),
                    zigzag,
                    t: 0.0,
                    base_x: pos.x,
                });
            }
        }

        self.fleet.dive_timer -= dt;
        if self.fleet.dive_timer <= 0.0 {
            self.fleet.dive_timer = (6.5 - 0.6 * level).max(2.0) * self.rng.range(0.7, 1.3);
            let in_formation: Vec<usize> = (0..self.fleet.enemies.len())
                .filter(|&i| self.fleet.enemies[i].mode == Mode::Formation)
                .collect();
            if !in_formation.is_empty() {
                let i = in_formation[self.rng.below(in_formation.len())];
                self.fleet.enemies[i].mode = Mode::Diving {
                    t: 0.0,
                    bombed: false,
                };
            }
        }
    }

    fn update_courier(&mut self, dt: f32) {
        match &mut self.courier {
            Some(c) => {
                c.pos.x += c.dir * 250.0 * dt;
                if c.pos.x < LEFT - 60.0 || c.pos.x > RIGHT + 60.0 {
                    self.courier = None;
                }
            }
            None => {
                self.courier_timer -= dt;
                if self.courier_timer <= 0.0 {
                    self.courier_timer = self.rng.range(18.0, 30.0);
                    let dir = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                    let x = if dir > 0.0 { LEFT - 50.0 } else { RIGHT + 50.0 };
                    self.courier = Some(Courier {
                        pos: vec2(x, TOP + 34.0),
                        dir,
                    });
                }
            }
        }
    }

    fn update_projectiles(&mut self, dt: f32) {
        for s in &mut self.shots {
            s.pos += s.vel * dt;
        }
        self.shots
            .retain(|s| s.pos.y > TOP - 20.0 && s.pos.x > LEFT - 20.0 && s.pos.x < RIGHT + 20.0);
        for b in &mut self.bombs {
            b.t += dt;
            b.pos += b.vel * dt;
            if b.zigzag {
                b.base_x += b.vel.x * dt;
                b.pos.x = b.base_x + (b.t * 6.0).sin() * 38.0;
            }
        }
        self.bombs.retain(|b| b.pos.y < BOTTOM + 10.0);
        for p in &mut self.powerups {
            p.pos.y += 150.0 * dt;
        }
        self.powerups.retain(|p| p.pos.y < BOTTOM + 20.0);
    }

    fn collide_shots(&mut self) {
        let mut si = 0;
        while si < self.shots.len() {
            let shot = self.shots[si];
            let damage = if shot.lance { 2 } else { 1 };
            let mut spent = false;

            // Lances burn through bombs; ordinary shots trade one for one.
            let mut bi = 0;
            while bi < self.bombs.len() && !spent {
                if circles_overlap(shot.pos, SHOT_R + 3.0, self.bombs[bi].pos, BOMB_R) {
                    let pos = self.bombs.swap_remove(bi).pos;
                    self.burst(pos, palette::RED, 10, 40.0, 220.0, None);
                    self.score += 5;
                    self.events.push(Event::BombShot);
                    spent = !shot.lance;
                } else {
                    bi += 1;
                }
            }

            if !spent
                && let Some(c) = &self.courier
                && circles_overlap(shot.pos, SHOT_R, c.pos, COURIER_R)
            {
                let pos = c.pos;
                self.courier = None;
                self.kill_courier(pos);
                spent = true;
            }

            let mut ei = 0;
            while ei < self.fleet.enemies.len() && !spent {
                let e = &mut self.fleet.enemies[ei];
                if !circles_overlap(shot.pos, SHOT_R, e.pos, ENEMY_R) {
                    ei += 1;
                    continue;
                }
                if e.hp > damage {
                    e.hp -= damage;
                    e.flash = 1.0;
                    let (pos, color) = (shot.pos, e.kind.color());
                    self.burst(pos, color, 12, 60.0, 260.0, None);
                    ei += 1;
                } else {
                    // swap_remove puts another enemy at ei, so don't advance
                    self.kill_enemy(ei, shot.pos.x);
                }
                if self.shots[si].pierce > 0 {
                    self.shots[si].pierce -= 1;
                } else {
                    spent = true;
                }
            }

            if spent {
                self.shots.swap_remove(si);
            } else {
                si += 1;
            }
        }
    }

    fn kill_enemy(&mut self, index: usize, impact_x: f32) {
        let e = self.fleet.enemies.swap_remove(index);
        self.combo += 1;
        self.combo_timer = COMBO_WINDOW;
        let gained = e.kind.points() * self.multiplier();
        self.score += gained;

        // As in the original: a hit left of centre sprays the debris rightward.
        let offset = ((impact_x - e.pos.x) / ENEMY_R).clamp(-1.0, 1.0);
        let bias = -offset * (std::f32::consts::PI / 6.0);
        let color = e.kind.color();
        self.burst(e.pos, color, 28, 300.0, 600.0, Some((bias, FRAC_PI_2)));
        self.burst(e.pos, color.mix(palette::WHITE, 0.4), 20, 50.0, 320.0, None);
        self.shocks.push(Shock {
            pos: e.pos,
            age: 0.0,
            strength: 1.0,
        });
        self.shake = self.shake.max(0.22);
        self.popups.push(Popup {
            pos: e.pos - vec2(0.0, 18.0),
            text: format!("+{gained}"),
            age: 0.0,
            color,
        });
        self.events.push(Event::Explosion);
        if self.powerups.len() < 2 && self.rng.chance(0.07) {
            self.drop_powerup(e.pos);
        }
    }

    fn kill_courier(&mut self, pos: Vec2) {
        self.combo += 1;
        self.combo_timer = COMBO_WINDOW;
        let gained = 150 * self.multiplier();
        self.score += gained;
        self.burst(pos, palette::MAGENTA, 70, 60.0, 520.0, None);
        self.shocks.push(Shock {
            pos,
            age: 0.0,
            strength: 1.6,
        });
        self.shake = self.shake.max(0.4);
        self.popups.push(Popup {
            pos,
            text: format!("+{gained}"),
            age: 0.0,
            color: palette::MAGENTA,
        });
        self.events.push(Event::Explosion);
        self.drop_powerup(pos);
    }

    fn drop_powerup(&mut self, pos: Vec2) {
        let mut options = vec![Power::Triple, Power::Overcharge];
        if !self.player.shield {
            options.push(Power::Shield);
        }
        let power = options[self.rng.below(options.len())];
        self.powerups.push(PowerUp { pos, power });
    }

    fn collide_player(&mut self) {
        if !self.player.alive {
            return;
        }
        let player = self.player_pos();

        let mut i = 0;
        while i < self.powerups.len() {
            if circles_overlap(player, PLAYER_R + 12.0, self.powerups[i].pos, POWERUP_R) {
                let p = self.powerups.swap_remove(i);
                match p.power {
                    Power::Triple => self.player.triple = TRIPLE_TIME,
                    Power::Shield => self.player.shield = true,
                    Power::Overcharge => self.player.overcharge = OVERCHARGE_DURATION,
                }
                self.score += 25;
                self.burst(p.pos, palette::WHITE, 24, 60.0, 300.0, None);
                self.events.push(Event::Pickup);
            } else {
                i += 1;
            }
        }

        let mut hit = false;
        let mut i = 0;
        while i < self.bombs.len() {
            if circles_overlap(player, PLAYER_R, self.bombs[i].pos, BOMB_R) {
                self.bombs.swap_remove(i);
                hit = true;
            } else {
                i += 1;
            }
        }
        // Ramming destroys the alien too, but earns nothing.
        let mut i = 0;
        while i < self.fleet.enemies.len() {
            if circles_overlap(player, PLAYER_R, self.fleet.enemies[i].pos, ENEMY_R) {
                let e = self.fleet.enemies.swap_remove(i);
                self.burst(e.pos, e.kind.color(), 40, 60.0, 420.0, None);
                self.events.push(Event::Explosion);
                hit = true;
            } else {
                i += 1;
            }
        }
        if hit {
            self.hit_player();
        }
    }

    fn hit_player(&mut self) {
        if self.player.invulnerable > 0.0 {
            return;
        }
        let pos = self.player_pos();
        if self.player.shield {
            self.player.shield = false;
            self.player.invulnerable = 1.0;
            self.burst(pos, palette::BLUE, 50, 120.0, 460.0, None);
            self.shocks.push(Shock {
                pos,
                age: 0.0,
                strength: 1.4,
            });
            self.shake = self.shake.max(0.4);
            self.events.push(Event::ShieldBreak);
            return;
        }
        self.lives = self.lives.saturating_sub(1);
        self.destroy_player();
        if self.lives == 0 {
            self.phase = Phase::GameOver(3.5);
            self.events.push(Event::GameOver);
        } else {
            self.phase = Phase::Respawning(1.6);
        }
    }

    fn destroy_player(&mut self) {
        let pos = self.player_pos();
        self.player.alive = false;
        self.player.triple = 0.0;
        self.player.overcharge = 0.0;
        self.combo = 0;
        self.combo_timer = 0.0;
        self.burst(pos, palette::AMBER, 120, 60.0, 620.0, None);
        self.burst(pos, palette::WHITE, 40, 200.0, 800.0, None);
        self.shocks.push(Shock {
            pos,
            age: 0.0,
            strength: 2.6,
        });
        self.shake = 1.0;
        self.events.push(Event::PlayerHit);
        // Sweep the sky so the next ship doesn't arrive under a falling bomb.
        self.shots.clear();
        for b in std::mem::take(&mut self.bombs) {
            self.burst(b.pos, palette::RED, 6, 30.0, 160.0, None);
        }
    }

    /// The fleet marching down to the player's line ends the game outright.
    fn check_invasion(&mut self) {
        let landed = self
            .fleet
            .enemies
            .iter()
            .any(|e| e.mode == Mode::Formation && e.pos.y + ENEMY_R >= PLAYER_Y - 26.0);
        if landed && self.phase == Phase::Playing {
            self.lives = 0;
            if self.player.alive {
                self.destroy_player();
            }
            self.phase = Phase::GameOver(3.5);
            self.events.push(Event::GameOver);
        }
    }

    fn check_wave_clear(&mut self) {
        if !self.fleet.enemies.is_empty() || self.phase != Phase::Playing {
            return;
        }
        self.score += 100 * self.level;
        self.level += 1;
        self.lives = (self.lives + 1).min(MAX_LIVES);
        for b in std::mem::take(&mut self.bombs) {
            self.burst(b.pos, palette::RED, 6, 30.0, 160.0, None);
        }
        self.fleet = Fleet::new(self.level, &mut self.rng);
        self.phase = Phase::WaveIntro(2.0);
        self.events.push(Event::WaveClear);
    }

    /// Sprays particles from `pos`. With a cone of `(bias, spread)` they fly
    /// mostly upward, skewed toward `bias` radians off vertical; otherwise in
    /// every direction.
    fn burst(
        &mut self,
        pos: Vec2,
        color: Rgb,
        count: usize,
        min_speed: f32,
        max_speed: f32,
        cone: Option<(f32, f32)>,
    ) {
        let room = MAX_PARTICLES.saturating_sub(self.particles.len());
        for _ in 0..count.min(room) {
            let angle = match cone {
                Some((bias, spread)) => {
                    let off = self.rng.exp(8.0) * spread;
                    bias + if self.rng.chance(0.5) { off } else { -off }
                }
                None => self.rng.range(0.0, TAU),
            };
            let speed = self.rng.range(min_speed, max_speed);
            let life = self.rng.range(0.45, 1.4);
            self.particles.push(Particle {
                pos,
                vel: vec2(angle.sin(), -angle.cos()) * speed,
                life,
                max_life: life,
                color,
                size: self.rng.range(1.2, 2.6),
            });
        }
    }

    /// A simple bot, used for the self-playing demo.
    pub fn autopilot(&self) -> Input {
        let px = self.player.x;
        let mut threat: Option<Vec2> = None;
        let dangers = self.bombs.iter().map(|b| b.pos).chain(
            self.fleet
                .enemies
                .iter()
                .filter(|e| e.mode != Mode::Formation)
                .map(|e| e.pos),
        );
        for d in dangers {
            let above = PLAYER_Y - d.y;
            if (0.0..260.0).contains(&above)
                && (d.x - px).abs() < 52.0
                && threat.is_none_or(|t| d.y > t.y)
            {
                threat = Some(d);
            }
        }

        let mut fire = false;
        let target_x = if let Some(t) = threat {
            if (t.x > px && px > LEFT + 120.0) || px > RIGHT - 120.0 {
                px - 120.0
            } else {
                px + 120.0
            }
        } else if let Some(p) = self.powerups.first() {
            p.pos.x
        } else {
            // Go for the lowest alien, leading it by the shot's travel time.
            let target = self
                .fleet
                .enemies
                .iter()
                .filter(|e| e.mode == Mode::Formation)
                .max_by(|a, b| {
                    let key = |e: &Enemy| e.pos.y * 4.0 - (e.pos.x - px).abs();
                    key(a).total_cmp(&key(b))
                });
            match target {
                Some(e) => {
                    let travel = (PLAYER_Y - e.pos.y) / (420.0 + 1100.0 * self.player.charge);
                    let x = e.pos.x + self.fleet.dir * self.fleet.speed * travel;
                    fire = (x - px).abs() < 14.0 && self.player.charge > 0.55;
                    x
                }
                None => px,
            }
        };
        Input {
            left: target_x < px - 6.0,
            right: target_x > px + 6.0,
            fire,
            fire_pressed: fire,
        }
    }
}
