//! Title screen.

use macroquad::prelude::vec2;

use crate::game::{Kind, W};
use crate::gfx::Gfx;
use crate::palette::{AMBER, CYAN, WHITE};
use crate::render::draw_enemy;

const OPTIONS: [&str; 3] = ["START GAME", "CREDITS", "QUIT"];

pub enum MenuAction {
    Start,
    Quit,
}

#[derive(Default)]
pub struct MenuInput {
    pub up: bool,
    pub down: bool,
    pub select: bool,
}

#[derive(Default)]
pub struct Menu {
    selection: usize,
    credits: bool,
    time: f32,
}

impl Menu {
    pub fn update(&mut self, dt: f32, input: &MenuInput) -> Option<MenuAction> {
        self.time += dt;
        if self.credits {
            self.credits = !input.select;
            return None;
        }
        if input.up && self.selection > 0 {
            self.selection -= 1;
        }
        if input.down && self.selection < OPTIONS.len() - 1 {
            self.selection += 1;
        }
        if input.select {
            match self.selection {
                0 => return Some(MenuAction::Start),
                1 => self.credits = true,
                _ => return Some(MenuAction::Quit),
            }
        }
        None
    }

    pub fn draw(&self, gfx: &mut Gfx, high_score: u32) {
        let t = self.time;
        let cx = W / 2.0;
        if self.credits {
            gfx.text_centered("CREDITS", cx, 220.0, 64.0, AMBER.a(1.0));
            gfx.text_centered("A GAME BY PERBU", cx, 400.0, 30.0, WHITE.a(0.95));
            gfx.text_centered("ORIGINALLY WRITTEN IN GO - REBUILT IN RUST", cx, 470.0, 20.0, CYAN.a(0.9));
            gfx.text_centered("PRESS ENTER", cx, 680.0, 18.0, AMBER.a(0.5 + 0.4 * (t * 4.0).sin()));
            return;
        }

        gfx.text_centered("EXTRATERRESTRIAL", cx, 150.0, 58.0, CYAN.a(1.0));
        gfx.text_centered("TRESPASSERS", cx, 250.0, 104.0, AMBER.a(1.0));

        let kinds = [Kind::Green, Kind::Red, Kind::Yellow, Kind::Cyan];
        for (i, kind) in kinds.into_iter().enumerate() {
            let x = cx + (i as f32 - 1.5) * 120.0;
            let pos = vec2(x, 446.0 + (t * 2.0 + i as f32).sin() * 5.0);
            gfx.halo(pos, 90.0, kind.color().a(0.16));
            draw_enemy(gfx, kind, pos, 24.0, t + i as f32, 0.0, 1);
        }

        for (i, option) in OPTIONS.iter().enumerate() {
            let y = 560.0 + i as f32 * 62.0;
            if i == self.selection {
                let pulse = 0.85 + 0.15 * (t * 6.0).sin();
                gfx.text_centered(&format!("> {option} <"), cx, y, 32.0, WHITE.a(pulse));
            } else {
                gfx.text_centered(option, cx, y, 28.0, CYAN.a(0.55));
            }
        }

        if high_score > 0 {
            gfx.text_centered(&format!("HIGH SCORE {high_score:06}"), cx, 60.0, 18.0, AMBER.a(0.85));
        }
        gfx.text_centered(
            "ARROWS: MOVE - SPACE: FIRE - HOLD TO CHARGE A LANCE - P: PAUSE - F: FULLSCREEN",
            cx,
            842.0,
            13.0,
            AMBER.a(0.6),
        );
    }
}
