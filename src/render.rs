//! Turns the world into glowing vector shapes.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use macroquad::prelude::{Vec2, vec2};

use crate::game::{
    BOTTOM, ENEMY_R, Enemy, H, Kind, LEFT, Mode, POPUP_LIFE, Phase, Power, RIGHT, Rng,
    SHOCK_LIFE, Shock, TOP, W, World,
};
use crate::gfx::Gfx;
use crate::palette::{self, AMBER, BLUE, CYAN, GREEN, MAGENTA, ORANGE, RED, Rgb, WHITE, YELLOW};

/// Drifting background stars, shared by the menu and the game.
pub struct Stars {
    stars: Vec<(Vec2, f32)>,
}

impl Stars {
    pub fn new() -> Stars {
        let mut rng = Rng::new(42);
        let stars = (0..240)
            .map(|_| (vec2(rng.range(0.0, W), rng.range(0.0, H)), rng.range(0.15, 1.0)))
            .collect();
        Stars { stars }
    }

    pub fn update(&mut self, dt: f32, speed: f32) {
        for (pos, depth) in &mut self.stars {
            pos.y += *depth * speed * dt;
            if pos.y > H + 4.0 {
                pos.y -= H + 8.0;
            }
        }
    }

    pub fn draw(&self, gfx: &mut Gfx, brightness: f32) {
        for &(pos, depth) in &self.stars {
            let color = Rgb(0.55, 0.7, 1.0).a(depth * depth * brightness);
            gfx.dot(pos, 0.7 + depth * 0.9, color);
        }
    }
}

pub fn draw_world(gfx: &mut Gfx, world: &World, stars: &Stars) {
    let t = world.time;
    draw_grid(gfx, world);
    stars.draw(gfx, 0.35);
    draw_frame(gfx);

    for p in &world.powerups {
        let (color, letter) = power_style(p.power);
        let pulse = 0.75 + 0.25 * (t * 6.0).sin();
        gfx.halo(p.pos, 70.0, color.a(0.22 * pulse));
        gfx.circle(p.pos, 16.0, 2.0, color.a(pulse));
        gfx.ngon(p.pos, 22.0, 4, t * 2.0, 1.2, color.a(0.5));
        gfx.text_centered(letter, p.pos.x, p.pos.y - 8.0, 16.0, WHITE.a(0.95));
    }

    for b in &world.bombs {
        let color = if b.zigzag { ORANGE } else { RED };
        let tail = b.pos - b.vel.normalize_or_zero() * 26.0;
        gfx.line(tail, b.pos, 1.6, color.a(0.45));
        gfx.ngon(b.pos, 7.0, 4, b.t * 9.0, 2.0, color.a(1.0));
        gfx.dot(b.pos, 2.0, WHITE.a(0.6 + 0.4 * (b.t * 20.0).sin()));
        gfx.halo(b.pos, 34.0, color.a(0.2));
    }

    for e in &world.fleet.enemies {
        draw_enemy_trail(gfx, e);
        let mut pos = e.pos;
        if let Mode::Diving { t: dive, .. } = e.mode
            && dive < 0.4
        {
            pos.x += (dive * 90.0).sin() * 3.0;
        }
        gfx.halo(pos, 92.0, e.kind.color().a(0.16));
        draw_enemy(gfx, e.kind, pos, ENEMY_R, t + e.phase, e.flash, e.hp);
    }

    if let Some(c) = &world.courier {
        draw_courier(gfx, c.pos, t);
    }

    for s in &world.shots {
        let dir = s.vel.normalize_or_zero();
        if s.lance {
            gfx.halo(s.pos, 90.0, CYAN.a(0.3));
            gfx.line(s.pos - dir * 110.0, s.pos, 2.0, CYAN.a(0.5));
            gfx.line(s.pos - dir * 46.0, s.pos, 5.0, CYAN.mix(WHITE, 0.6).a(1.0));
        } else {
            let color = AMBER.mix(WHITE, s.charge * 0.7);
            let len = 14.0 + 44.0 * s.charge;
            gfx.line(s.pos - dir * len, s.pos, 2.4 + 1.6 * s.charge, color.a(1.0));
            gfx.halo(s.pos, 30.0 + 30.0 * s.charge, AMBER.a(0.2));
        }
    }

    draw_player(gfx, world);

    for p in &world.particles {
        let k = p.life / p.max_life;
        let tail = p.pos - p.vel * 0.035;
        gfx.line(tail, p.pos, p.size * (0.4 + 0.6 * k), p.color.a(k * k));
    }

    for s in &world.shocks {
        let k = 1.0 - s.age / SHOCK_LIFE;
        let radius = 30.0 + 380.0 * s.age;
        gfx.circle(s.pos, radius, 1.5, WHITE.a(0.06 * k * k * s.strength));
    }

    for p in &world.popups {
        let k = 1.0 - p.age / POPUP_LIFE;
        gfx.text_centered(&p.text, p.pos.x, p.pos.y, 15.0, p.color.mix(WHITE, 0.4).a(k));
    }

    draw_hud(gfx, world);
    draw_banners(gfx, world);
}

fn power_style(power: Power) -> (Rgb, &'static str) {
    match power {
        Power::Triple => (MAGENTA, "T"),
        Power::Shield => (BLUE, "S"),
        Power::Overcharge => (AMBER, "C"),
        Power::Homing => (YELLOW, "H"),
        Power::SlowTime => (CYAN, "Z"),
        Power::Deflector => (GREEN, "D"),
    }
}

/// Displacement of the background grid at `p`: explosions send a ripple
/// through it and the player's ship drags a small dent along.
fn warp(p: Vec2, shocks: &[Shock], player: Option<Vec2>) -> Vec2 {
    let mut d = Vec2::ZERO;
    for s in shocks {
        let to = p - s.pos;
        let r = to.length();
        let front = 30.0 + 380.0 * s.age;
        let k = (r - front) / 80.0;
        let fade = 1.0 - s.age / SHOCK_LIFE;
        d += to / r.max(1.0) * (-k * k).exp() * 22.0 * s.strength * fade;
    }
    if let Some(pl) = player {
        let to = p - pl;
        let r = to.length();
        d -= to / r.max(1.0) * 14.0 * (-(r / 150.0).powi(2)).exp();
    }
    d
}

fn draw_grid(gfx: &mut Gfx, world: &World) {
    const CELL: f32 = 64.0;
    let cols = ((RIGHT - LEFT) / CELL) as usize;
    let rows = ((BOTTOM - TOP) / CELL).ceil() as usize;
    let player = world.player.alive.then(|| world.player_pos());
    let point = |c: usize, r: usize| {
        let p = vec2(
            LEFT + c as f32 * CELL,
            (TOP + r as f32 * CELL).min(BOTTOM),
        );
        // keep the outermost lines glued to the frame
        if c == 0 || c == cols || r == 0 || r == rows {
            p
        } else {
            p + warp(p, &world.shocks, player)
        }
    };
    let color = palette::GRID.a(0.085);
    for r in 0..=rows {
        for c in 0..=cols {
            let p = point(c, r);
            if c < cols {
                gfx.seg(p, point(c + 1, r), 0.5, color);
            }
            if r < rows {
                gfx.seg(p, point(c, r + 1), 0.5, color);
            }
        }
    }
}

fn draw_frame(gfx: &mut Gfx) {
    let corners = [
        vec2(LEFT, TOP),
        vec2(RIGHT, TOP),
        vec2(RIGHT, BOTTOM),
        vec2(LEFT, BOTTOM),
    ];
    gfx.polyline(&corners, true, 1.6, AMBER.a(0.85));
    // heavier brackets in the corners
    for (i, &c) in corners.iter().enumerate() {
        let sx = if i == 0 || i == 3 { 1.0 } else { -1.0 };
        let sy = if i < 2 { 1.0 } else { -1.0 };
        gfx.line(c, c + vec2(sx * 46.0, 0.0), 3.2, AMBER.a(1.0));
        gfx.line(c, c + vec2(0.0, sy * 46.0), 3.2, AMBER.a(1.0));
    }
}

fn draw_enemy_trail(gfx: &mut Gfx, e: &Enemy) {
    let n = e.trail.len();
    if n < 2 {
        return;
    }
    let color = e.kind.color();
    let mut prev = e.pos;
    for (i, &p) in e.trail.iter().enumerate() {
        let k = 1.0 - i as f32 / n as f32;
        gfx.line(prev, p, 0.8 + 2.6 * k, color.a(0.75 * k * k));
        prev = p;
    }
}

/// Each alien type is its own little piece of geometry; `t` drives the spin.
pub fn draw_enemy(gfx: &mut Gfx, kind: Kind, pos: Vec2, r: f32, t: f32, flash: f32, hp: u8) {
    let base = kind.color().mix(WHITE, flash);
    let line = base.a(1.0);
    let fill = base.a(0.3 + 0.5 * flash);
    let w = r * 0.115;
    match kind {
        Kind::Cyan => {
            gfx.circle(pos, r, w, line);
            let rot = t * 1.4;
            gfx.fill_ngon(pos, r * 0.68, 3, rot, fill);
            gfx.ngon(pos, r * 0.68, 3, rot, w * 0.6, line);
        }
        Kind::Yellow => {
            let rot = (t * 1.6).sin() * 0.5 + FRAC_PI_2 / 2.0;
            gfx.ngon(pos, r * 1.05, 4, rot, w, line);
            gfx.fill_ngon(pos, r * 0.5, 4, -rot * 2.0, fill);
            gfx.ngon(pos, r * 0.5, 4, -rot * 2.0, w * 0.6, line);
        }
        Kind::Red => {
            let rot = t * 0.9;
            let points: Vec<Vec2> = (0..8)
                .map(|i| {
                    let radius = if i % 2 == 0 { r * 1.12 } else { r * 0.46 };
                    pos + Vec2::from_angle(rot + TAU * i as f32 / 8.0) * radius
                })
                .collect();
            gfx.fill_poly(&[points[1], points[3], points[5], points[7]], fill);
            gfx.polyline(&points, true, w, line);
            gfx.dot(pos, r * 0.12, WHITE.a(0.8));
        }
        Kind::Green => {
            let rot = t * 0.7;
            gfx.ngon(pos, r, 6, rot, w, line);
            gfx.fill_ngon(pos, r * 0.55, 6, -rot, fill);
            gfx.ngon(pos, r * 0.55, 6, -rot, w * 0.6, line);
        }
    }
    if hp > 1 {
        // armour plating
        for i in 0..3 {
            let a = t * 1.2 + TAU * i as f32 / 3.0;
            gfx.arc(pos, r * 1.36, a, a + 1.4, w * 0.7, base.mix(WHITE, 0.5).a(0.9));
        }
    }
}

fn draw_courier(gfx: &mut Gfx, pos: Vec2, t: f32) {
    let color = MAGENTA;
    gfx.halo(pos, 110.0, color.a(0.22));
    let hull = [
        pos + vec2(-30.0, 2.0),
        pos + vec2(-14.0, -7.0),
        pos + vec2(14.0, -7.0),
        pos + vec2(30.0, 2.0),
        pos + vec2(16.0, 10.0),
        pos + vec2(-16.0, 10.0),
    ];
    gfx.fill_poly(&hull, color.a(0.25));
    gfx.polyline(&hull, true, 2.2, color.a(1.0));
    gfx.arc(pos + vec2(0.0, -7.0), 11.0, PI, TAU, 1.8, color.mix(WHITE, 0.4).a(1.0));
    for i in 0..4 {
        let on = ((t * 8.0) as i32 + i) % 4 == 0;
        let x = -18.0 + 12.0 * i as f32;
        gfx.dot(pos + vec2(x, 2.0), 1.8, WHITE.a(if on { 1.0 } else { 0.25 }));
    }
}

fn draw_player(gfx: &mut Gfx, world: &World) {
    let p = &world.player;
    if !p.alive {
        return;
    }
    let t = world.time;
    let pos = world.player_pos();
    // flicker while the respawn grace period lasts
    let vis = if p.invulnerable > 0.0 {
        0.35 + 0.4 * ((t * 22.0).sin() * 0.5 + 0.5)
    } else {
        1.0
    };
    let full = p.charge >= 1.0;
    let pulse = if full { 0.5 + 0.5 * (t * 16.0).sin() } else { 0.0 };
    let color = AMBER.mix(WHITE, pulse * 0.5);

    gfx.halo(pos, 120.0 + 40.0 * p.charge, AMBER.a(0.13 * vis * (0.6 + 0.4 * p.charge)));
    gfx.ngon(pos, 25.0, 6, FRAC_PI_2, 2.8, color.a(vis));

    // the ship sits inside the hexagon and leans into its movement
    let lean = p.vx / 540.0 * 0.3;
    let rot = |v: Vec2| pos + Vec2::from_angle(lean).rotate(v);
    let ship = [rot(vec2(0.0, -15.0)), rot(vec2(11.0, 9.0)), rot(vec2(0.0, 4.0)), rot(vec2(-11.0, 9.0))];
    gfx.fill_poly(&[ship[0], ship[1], ship[2]], AMBER.a(0.35 * vis));
    gfx.fill_poly(&[ship[0], ship[2], ship[3]], AMBER.a(0.35 * vis));
    gfx.polyline(&ship, true, 1.8, color.mix(WHITE, 0.3).a(vis));
    let flame = 10.0 + 5.0 * (t * 40.0).sin() + p.vx.abs() / 60.0;
    gfx.line(rot(vec2(0.0, 8.0)), rot(vec2(0.0, 8.0 + flame)), 2.0, ORANGE.a(0.8 * vis));

    // six charge lamps, one per corner
    for i in 0..6 {
        let a = -FRAC_PI_2 + TAU * i as f32 / 6.0;
        let lit = p.charge * 6.0 >= i as f32 + 1.0;
        let lamp = pos + Vec2::from_angle(a) * 35.0;
        if lit {
            let c = if full { CYAN.mix(WHITE, 0.5) } else { AMBER.mix(WHITE, 0.3) };
            gfx.dot(lamp, 2.6, c.a(vis));
        } else {
            gfx.dot(lamp, 1.6, AMBER.a(0.25 * vis));
        }
    }
    if full {
        gfx.circle(pos, 31.0 + 2.0 * pulse, 1.2, CYAN.a(0.7 * vis));
    }
    if p.deflector > 0.0 {
        let k = if p.deflector < 2.0 { 0.5 + 0.5 * (t * 14.0).sin() } else { 1.0 };
        gfx.circle(pos, crate::game::DEFLECTOR_R, 1.4, GREEN.a(0.35 * k));
        gfx.circle(pos, crate::game::DEFLECTOR_R - 6.0, 1.0, GREEN.a(0.15 * k));
    }
    if p.shield {
        for i in 0..3 {
            let a = t * 2.4 + TAU * i as f32 / 3.0;
            gfx.arc(pos, 44.0, a, a + 1.7, 2.2, BLUE.mix(WHITE, 0.3).a(0.95));
        }
        gfx.halo(pos, 90.0, BLUE.a(0.25));
    }
}

fn draw_hud(gfx: &mut Gfx, world: &World) {
    let y = 20.0;
    gfx.text(&format!("WAVE {}", world.level), vec2(LEFT, y), 20.0, AMBER.a(0.95));

    gfx.text_centered(&format!("{:06}", world.score), W / 2.0, y - 2.0, 24.0, WHITE.a(0.95));
    let mult = world.multiplier();
    if mult > 1 {
        let x = W / 2.0 + 110.0;
        gfx.text(&format!("X{mult}"), vec2(x, y), 20.0, CYAN.a(1.0));
        let left = (world.combo_timer / 2.2).clamp(0.0, 1.0);
        gfx.line(vec2(x, y + 28.0), vec2(x + 40.0 * left, y + 28.0), 1.4, CYAN.a(0.8));
    }

    gfx.text_right(&format!("LIVES: {}", world.lives), RIGHT, y, 20.0, WHITE.a(0.9));

    // charge gauge
    let (x0, x1, y0, y1) = (RIGHT - 330.0, RIGHT - 170.0, y + 3.0, y + 17.0);
    let frame = [vec2(x0, y0), vec2(x1, y0), vec2(x1, y1), vec2(x0, y1)];
    gfx.polyline(&frame, true, 1.0, AMBER.a(0.5));
    let charge = world.player.charge;
    if world.player.alive && charge > 0.0 {
        let full = charge >= 1.0;
        let color = if full {
            CYAN.mix(WHITE, 0.4 + 0.3 * (world.time * 16.0).sin())
        } else {
            AMBER
        };
        let xe = x0 + 3.0 + (x1 - x0 - 6.0) * charge;
        gfx.fill_poly(
            &[vec2(x0 + 3.0, y0 + 3.0), vec2(xe, y0 + 3.0), vec2(xe, y1 - 3.0), vec2(x0 + 3.0, y1 - 3.0)],
            color.a(if full { 0.9 } else { 0.6 }),
        );
    }

    // active power-ups, below the frame
    let mut x = LEFT;
    let by = BOTTOM + 10.0;
    let p = &world.player;
    let mut label = |text: String, color: Rgb| {
        gfx.text(&text, vec2(x, by), 13.0, color.a(0.95));
        x += crate::font::width(&text, 13.0) + 36.0;
    };
    if p.triple > 0.0 {
        label(format!("TRIPLE {:02}", p.triple.ceil() as u32), MAGENTA);
    }
    if p.overcharge > 0.0 {
        label(format!("OVERCHARGE {:02}", p.overcharge.ceil() as u32), AMBER);
    }
    if p.homing > 0.0 {
        label(format!("HOMING {:02}", p.homing.ceil() as u32), YELLOW);
    }
    if p.slow > 0.0 {
        label(format!("SLOW TIME {:02}", p.slow.ceil() as u32), CYAN);
    }
    if p.deflector > 0.0 {
        label(format!("DEFLECTOR {:02}", p.deflector.ceil() as u32), GREEN);
    }
    if p.shield {
        label("SHIELD".to_string(), BLUE);
    }
}

fn draw_banners(gfx: &mut Gfx, world: &World) {
    match world.phase {
        Phase::WaveIntro(left) => {
            let k = (left * 2.0).min(1.0);
            gfx.text_centered(&format!("WAVE {}", world.level), W / 2.0, 430.0, 72.0, AMBER.a(k));
            if world.level > 1 {
                gfx.text_centered("BONUS SHIP", W / 2.0, 540.0, 22.0, CYAN.a(k));
            }
        }
        Phase::GameOver(left) => {
            let k = ((3.5 - left) * 1.5).clamp(0.0, 1.0);
            gfx.text_centered("GAME OVER", W / 2.0, 380.0, 96.0, RED.a(k));
            let score = format!("SCORE {:06}", world.score);
            gfx.text_centered(&score, W / 2.0, 520.0, 30.0, WHITE.a(k));
        }
        _ => {}
    }
}
