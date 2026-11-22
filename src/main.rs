mod audio;
mod font;
mod game;
mod gfx;
mod menu;
mod palette;
mod post;
mod render;

use std::path::PathBuf;

use macroquad::prelude::*;

use audio::{Audio, Sfx};
use game::{Event, Input, World};
use menu::{Menu, MenuAction, MenuInput};

/// The simulation runs at a fixed rate, independent of the display.
const STEP: f32 = 1.0 / 120.0;

fn conf() -> Conf {
    Conf {
        window_title: "Extraterrestrial Trespassers".to_string(),
        window_width: 1600,
        window_height: 900,
        high_dpi: true,
        fullscreen: std::env::var_os("FULLSCREEN").is_some(),
        ..Default::default()
    }
}

enum Scene {
    Menu,
    Game(Box<World>),
}

struct Options {
    /// Let the bot play.
    demo: bool,
    /// Demo mode that also saves screenshots to this directory, then exits.
    shots: Option<PathBuf>,
}

fn options() -> Options {
    let mut args = std::env::args().skip(1);
    let mut options = Options {
        demo: false,
        shots: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => options.demo = true,
            "--shots" => {
                options.demo = true;
                options.shots = Some(args.next().unwrap_or_else(|| ".".into()).into());
            }
            other => {
                eprintln!("unknown argument: {other}\nusage: trespassers-rs [--demo]");
                std::process::exit(2);
            }
        }
    }
    options
}

fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

#[macroquad::main(conf)]
async fn main() {
    let _ = dotenvy::dotenv();
    let options = options();

    let volume = std::env::var("VOLUME")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .map_or(0.8, |v| v.clamp(0.0, 1.0));
    let music = std::env::var_os("MUSIC_DISABLED").is_none();
    let audio = if options.shots.is_some() {
        None
    } else {
        Audio::new(volume, music)
    };

    let supersample = if screen_dpi_scale() > 1.0 { 2.0 } else { 1.0 };
    let post = post::Post::new(supersample);
    let mut gfx = gfx::Gfx::new();
    let mut stars = render::Stars::new();
    let mut menu = Menu::default();
    let mut scene = if options.demo && options.shots.is_none() {
        Scene::Game(Box::new(World::new(seed())))
    } else {
        Scene::Menu
    };

    let mut high_score = 0;
    let mut accumulator = 0.0;
    let mut fire_latch = false;
    let mut paused = false;
    let mut fullscreen = std::env::var_os("FULLSCREEN").is_some();
    let mut frame = 0u32;

    loop {
        frame += 1;
        if is_key_pressed(KeyCode::F) {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }

        // In screenshot mode time is simulated so the output is reproducible.
        let frame_time = if options.shots.is_some() {
            4.0 / 60.0
        } else {
            get_frame_time().min(0.1)
        };

        let mut quit = false;
        match &mut scene {
            Scene::Menu => {
                if let Some(a) = &audio {
                    a.set_music_level(1.0);
                }
                stars.update(frame_time, 160.0);
                let input = MenuInput {
                    up: is_key_pressed(KeyCode::Up),
                    down: is_key_pressed(KeyCode::Down),
                    select: is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space),
                };
                if options.shots.is_some() && frame == 20 {
                    // screenshot run: one capture of the menu, then the demo
                    scene = Scene::Game(Box::new(World::new(1)));
                    continue;
                }
                match menu.update(frame_time, &input) {
                    Some(MenuAction::Start) => {
                        scene = Scene::Game(Box::new(World::new(seed())));
                        accumulator = 0.0;
                        paused = false;
                    }
                    Some(MenuAction::Quit) => quit = true,
                    None => quit = is_key_pressed(KeyCode::Escape),
                }
            }
            Scene::Game(world) => {
                if let Some(a) = &audio {
                    a.set_music_level(0.5);
                }
                if is_key_pressed(KeyCode::P) {
                    paused = !paused;
                }
                fire_latch |= is_key_pressed(KeyCode::Space);
                if !paused {
                    stars.update(frame_time, 30.0);
                    accumulator += frame_time;
                    while accumulator >= STEP {
                        accumulator -= STEP;
                        let input = if options.demo {
                            world.autopilot()
                        } else {
                            Input {
                                left: is_key_down(KeyCode::Left) || is_key_down(KeyCode::A),
                                right: is_key_down(KeyCode::Right) || is_key_down(KeyCode::D),
                                fire: is_key_down(KeyCode::Space),
                                fire_pressed: std::mem::take(&mut fire_latch),
                            }
                        };
                        world.update(STEP, &input);
                    }
                }
                for event in world.events.drain(..) {
                    let Some(a) = &audio else { continue };
                    a.play(match event {
                        Event::Shoot { charge } => Sfx::Shoot { charge },
                        Event::Explosion => Sfx::Explosion,
                        Event::PlayerHit => Sfx::Thud,
                        Event::BombShot => Sfx::BombShot,
                        Event::Pickup => Sfx::Pickup,
                        Event::ShieldBreak => Sfx::ShieldBreak,
                        Event::WaveClear => Sfx::WaveClear,
                        Event::GameOver => Sfx::GameOver,
                    });
                }
                high_score = high_score.max(world.score);
                if world.finished || is_key_pressed(KeyCode::Escape) {
                    quit = options.demo;
                    scene = Scene::Menu;
                }
            }
        }
        if quit {
            break;
        }

        let shake = match &scene {
            Scene::Game(world) if !paused => {
                let s = world.shake * world.shake * 14.0;
                let t = world.time;
                vec2((t * 71.0).sin(), (t * 93.0).cos()) * s
            }
            _ => Vec2::ZERO,
        };
        post.begin(shake);
        match &scene {
            Scene::Menu => {
                stars.draw(&mut gfx, 0.9);
                menu.draw(&mut gfx, high_score);
            }
            Scene::Game(world) => {
                render::draw_world(&mut gfx, world, &stars);
                if paused {
                    gfx.text_centered("PAUSED", game::W / 2.0, 410.0, 72.0, palette::WHITE.a(0.9));
                }
            }
        }
        gfx.flush();
        post.end();

        if let Some(dir) = &options.shots {
            if matches!(frame, 15 | 75 | 150 | 260 | 400) {
                let path = dir.join(format!("shot-{frame:03}.png"));
                get_screen_data().export_png(&path.to_string_lossy());
            }
            if frame > 400 {
                break;
            }
        }
        next_frame().await;
    }
}
