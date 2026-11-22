use super::*;

const DT: f32 = 1.0 / 120.0;

fn playing() -> World {
    let mut w = World::new(7);
    w.phase = Phase::Playing;
    w
}

fn idle(w: &mut World, seconds: f32) {
    for _ in 0..(seconds / DT) as usize {
        w.update(DT, &Input::default());
    }
}

/// Removes everything that could hit the player by chance.
fn pacify(w: &mut World) {
    w.fleet.dive_timer = f32::MAX;
    w.courier_timer = f32::MAX;
    w.bombs.clear();
}

#[test]
fn circles() {
    assert!(circles_overlap(vec2(0.0, 0.0), 5.0, vec2(8.0, 0.0), 5.0));
    assert!(!circles_overlap(vec2(0.0, 0.0), 5.0, vec2(20.0, 20.0), 5.0));
    // touching edges do not count as a collision
    assert!(!circles_overlap(vec2(0.0, 0.0), 5.0, vec2(10.0, 0.0), 5.0));
}

#[test]
fn fleet_reverses_and_descends_at_the_edge() {
    let mut w = playing();
    let start_y = w.fleet.origin.y;
    assert_eq!(w.fleet.dir, 1.0);
    for _ in 0..20_000 {
        pacify(&mut w);
        w.update(DT, &Input::default());
        if w.fleet.dir < 0.0 {
            break;
        }
    }
    assert_eq!(w.fleet.dir, -1.0);
    idle(&mut w, 0.5);
    assert!((w.fleet.origin.y - start_y - DESCENT).abs() < 0.01);
}

#[test]
fn kills_score_and_build_a_multiplier() {
    let mut w = playing();
    assert_eq!(w.multiplier(), 1);
    for _ in 0..4 {
        let i = w
            .fleet
            .enemies
            .iter()
            .position(|e| e.kind == Kind::Cyan)
            .unwrap();
        let x = w.fleet.enemies[i].pos.x;
        w.kill_enemy(i, x);
    }
    // three at x1, the fourth lifts the multiplier to x2
    assert_eq!(w.score, 10 * 3 + 10 * 2);
    assert_eq!(w.multiplier(), 2);
    assert_eq!(w.fleet.enemies.len(), COLS * ROWS - 4);

    // the chain lapses when no kill follows in time
    for _ in 0..((COMBO_WINDOW + 0.1) / DT) as usize {
        pacify(&mut w);
        w.update(DT, &Input::default());
    }
    assert_eq!(w.multiplier(), 1);
}

#[test]
fn full_charge_fires_a_piercing_lance() {
    let mut w = playing();
    pacify(&mut w);
    // line up under a column and let the gun charge
    w.player.x = w.fleet.enemies[0].pos.x;
    w.player.charge = 1.0;
    let column_x = w.player.x;
    w.fire();
    assert!(w.shots[0].lance);
    let before = w.fleet.enemies.len();
    // freeze the formation so the whole column stays in the line of fire
    for _ in 0..240 {
        pacify(&mut w);
        w.collide_shots();
        w.update_projectiles(DT, DT);
    }
    let killed = before - w.fleet.enemies.len();
    assert_eq!(killed, 1 + LANCE_PIERCE as usize);
    assert!(w.fleet.enemies.iter().all(|e| (e.pos.x - column_x).abs() > 1.0));
}

#[test]
fn weak_shot_stops_at_the_first_alien() {
    let mut w = playing();
    pacify(&mut w);
    w.player.x = w.fleet.enemies[0].pos.x;
    w.player.charge = 0.5;
    w.fire();
    assert!(!w.shots[0].lance);
    let before = w.fleet.enemies.len();
    for _ in 0..600 {
        w.collide_shots();
        w.update_projectiles(DT, DT);
    }
    assert_eq!(before - w.fleet.enemies.len(), 1);
    assert!(w.shots.is_empty());
}

#[test]
fn bombs_cost_lives_and_the_last_one_ends_the_game() {
    let mut w = playing();
    for expected in (0..START_LIVES).rev() {
        pacify(&mut w);
        w.phase = Phase::Playing;
        w.player.alive = true;
        w.player.invulnerable = 0.0;
        let pos = w.player_pos();
        w.bombs.push(Bomb {
            pos,
            vel: Vec2::ZERO,
            zigzag: false,
            t: 0.0,
            base_x: pos.x,
        });
        w.collide_player();
        assert_eq!(w.lives, expected);
        assert!(!w.player.alive);
    }
    assert!(matches!(w.phase, Phase::GameOver(_)));
    assert!(w.events.contains(&Event::GameOver));
    idle(&mut w, 4.0);
    assert!(w.finished);
}

#[test]
fn shield_absorbs_one_hit() {
    let mut w = playing();
    pacify(&mut w);
    w.player.shield = true;
    w.hit_player();
    assert_eq!(w.lives, START_LIVES);
    assert!(w.player.alive && !w.player.shield);
    assert!(w.events.contains(&Event::ShieldBreak));
}

#[test]
fn respawn_gives_a_grace_period() {
    let mut w = playing();
    pacify(&mut w);
    w.hit_player();
    assert!(matches!(w.phase, Phase::Respawning(_)));
    idle(&mut w, 1.7);
    assert_eq!(w.phase, Phase::Playing);
    assert!(w.player.alive && w.player.invulnerable > 0.0);
    w.hit_player();
    assert_eq!(w.lives, START_LIVES - 1);
}

#[test]
fn clearing_the_wave_starts_the_next_with_a_bonus_life() {
    let mut w = playing();
    w.fleet.enemies.clear();
    w.update(DT, &Input::default());
    assert_eq!(w.level, 2);
    assert_eq!(w.lives, START_LIVES + 1);
    assert_eq!(w.fleet.enemies.len(), COLS * ROWS);
    assert!(matches!(w.phase, Phase::WaveIntro(_)));
    assert!(w.events.contains(&Event::WaveClear));
}

#[test]
fn fleet_reaching_the_player_line_is_game_over() {
    let mut w = playing();
    pacify(&mut w);
    w.fleet.origin.y = PLAYER_Y;
    w.update(DT, &Input::default());
    assert!(matches!(w.phase, Phase::GameOver(_)));
    assert_eq!(w.lives, 0);
}

#[test]
fn autopilot_survives_and_scores() {
    let mut w = World::new(3);
    for _ in 0..(30.0 / DT) as usize {
        let input = w.autopilot();
        w.update(DT, &input);
    }
    assert!(w.score > 100, "bot only scored {}", w.score);
}

#[test]
fn deflector_destroys_nearby_bombs() {
    let mut w = playing();
    pacify(&mut w);
    w.player.deflector = 5.0;
    let pos = w.player_pos() - vec2(0.0, DEFLECTOR_R - 20.0);
    w.bombs.push(Bomb { pos, vel: vec2(0.0, 100.0), zigzag: false, t: 0.0, base_x: pos.x });
    w.collide_player();
    assert!(w.bombs.is_empty());
    assert_eq!(w.lives, START_LIVES);
}

#[test]
fn slow_time_slows_bombs_but_not_the_player() {
    let mut w = playing();
    pacify(&mut w);
    w.player.slow = 5.0;
    let pos = vec2(W / 2.0, 300.0);
    w.bombs.push(Bomb { pos, vel: vec2(0.0, 200.0), zigzag: false, t: 0.0, base_x: pos.x });
    w.update(1.0, &Input::default());
    let dy = w.bombs[0].pos.y - pos.y;
    assert!((dy - 100.0).abs() < 1.0, "bomb moved {dy}");
}

#[test]
fn homing_shot_bends_toward_an_alien() {
    let mut w = playing();
    pacify(&mut w);
    let target = w.fleet.enemies[0].pos;
    w.player.x = target.x + 120.0;
    w.player.homing = 5.0;
    w.player.charge = 0.5;
    w.fire();
    let before = w.shots[0].vel.x;
    for _ in 0..30 {
        w.update_projectiles(DT, DT);
    }
    assert!(w.shots[0].vel.x < before, "shot did not turn left");
}
