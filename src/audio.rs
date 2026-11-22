//! Sound effects and music. Everything is embedded in the binary. If no audio
//! device can be opened the game simply runs silent.

use std::f32::consts::TAU;
use std::io::Cursor;
use std::num::NonZero;

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

static SHOOT: &[u8] = include_bytes!("../assets/shoot.wav");
static EXPLOSION: &[u8] = include_bytes!("../assets/explosion.wav");
static THUD: &[u8] = include_bytes!("../assets/thud.mp3");
static GAME_OVER: &[u8] = include_bytes!("../assets/gameover.mp3");
static SONG: &[u8] = include_bytes!("../assets/intro.mp3");

const SAMPLE_RATE: u32 = 44_100;

#[derive(Clone, Copy)]
pub enum Sfx {
    Shoot { charge: f32 },
    Explosion,
    Thud,
    GameOver,
    Pickup,
    ShieldBreak,
    WaveClear,
    BombShot,
}

pub struct Audio {
    sink: MixerDeviceSink,
    music: Option<Player>,
    volume: f32,
}

impl Audio {
    /// `volume` scales everything; `music` enables the background song.
    pub fn new(volume: f32, music: bool) -> Option<Audio> {
        let mut sink = DeviceSinkBuilder::open_default_sink().ok()?;
        sink.log_on_drop(false);
        let music = music
            .then(|| {
                let song = Decoder::new_looped(Cursor::new(SONG)).ok()?;
                let player = Player::connect_new(sink.mixer());
                player.set_volume(volume);
                player.append(song);
                Some(player)
            })
            .flatten();
        Some(Audio {
            sink,
            music,
            volume,
        })
    }

    /// Music level relative to the master volume.
    pub fn set_music_level(&self, level: f32) {
        if let Some(music) = &self.music {
            music.set_volume(self.volume * level);
        }
    }

    pub fn play(&self, sfx: Sfx) {
        let v = self.volume;
        let mixer = self.sink.mixer();
        let sample = |bytes: &'static [u8]| Decoder::new(Cursor::new(bytes)).ok();
        match sfx {
            Sfx::Shoot { charge } => {
                // charged shots sound deeper and louder
                if let Some(s) = sample(SHOOT) {
                    mixer.add(s.speed(1.25 - 0.55 * charge).amplify(v * (0.45 + 0.4 * charge)));
                }
            }
            Sfx::Explosion => {
                if let Some(s) = sample(EXPLOSION) {
                    mixer.add(s.amplify(v * 0.8));
                }
            }
            Sfx::Thud => {
                if let Some(s) = sample(THUD) {
                    mixer.add(s.amplify(v));
                }
            }
            Sfx::GameOver => {
                if let Some(s) = sample(GAME_OVER) {
                    mixer.add(s.amplify(v));
                }
            }
            Sfx::Pickup => mixer.add(tones(&[660.0, 880.0, 1320.0], 0.07, v * 0.3)),
            Sfx::WaveClear => mixer.add(tones(&[523.0, 659.0, 784.0, 1047.0], 0.11, v * 0.3)),
            Sfx::ShieldBreak => mixer.add(tones(&[520.0, 390.0, 260.0, 130.0], 0.06, v * 0.4)),
            Sfx::BombShot => mixer.add(tones(&[1500.0, 900.0], 0.03, v * 0.2)),
        }
    }
}

/// A short run of square-ish notes, for the effects the original had no sample for.
fn tones(freqs: &[f32], note_len: f32, gain: f32) -> SamplesBuffer {
    let per_note = (note_len * SAMPLE_RATE as f32) as usize;
    let mut samples = Vec::with_capacity(per_note * freqs.len());
    for &freq in freqs {
        for i in 0..per_note {
            let t = i as f32 / SAMPLE_RATE as f32;
            let envelope = (1.0 - i as f32 / per_note as f32).powf(0.6);
            let phase = (t * freq * TAU).sin();
            // soft-clipped sine: buzzy without the harshness of a pure square
            samples.push((phase * 3.0).tanh() * envelope * gain);
        }
    }
    SamplesBuffer::new(
        NonZero::new(1).unwrap(),
        NonZero::new(SAMPLE_RATE).unwrap(),
        samples,
    )
}
