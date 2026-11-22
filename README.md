# Extraterrestrial Trespassers (Rust)

![gameplay](img.png?raw=true "Gameplay")

A Rust port of [Extraterrestrial Trespassers](../extraterrestrial_trespassers),
rebuilt as a neon vector shooter. Shoot down the alien fleet before it reaches
the bottom of the screen. The game is over when it does, or when you run out
of ships.

    cargo run --release

## Controls

| Key                | Action                               |
|--------------------|--------------------------------------|
| Left / Right, A / D| Move                                 |
| Space              | Fire (tap), or hold to fire lances   |
| P                  | Pause                                |
| F                  | Toggle fullscreen                    |
| Esc                | Back to the menu / quit              |

## How it plays

The gun charges while you are not firing, as in the original, and the charge
decides how fast the shot travels. What is new:

* **Lances.** A full charge fires a lance that punches through up to four
  aliens and burns away bombs in its path. Holding Space fires one every time
  the gun tops up.
* **Divers.** Aliens peel out of the formation and swoop at you, dropping an
  aimed bomb on the way. Ones that miss rejoin the formation from the top.
* **Chains.** Kills in quick succession build a score multiplier, up to x8.
  Losing a ship resets it.
* **Power-ups.** Dropped now and then by aliens, and always by the courier
  that crosses above the fleet: `T` triple shot, `C` overcharge (the gun
  charges three times as fast), `S` shield (absorbs one hit), `H` homing
  (shots steer toward aliens), `Z` slow time (everything but you and your
  shots runs at half speed), `D` deflector (destroys bombs near your ship).
* **Armour.** From wave 3 the top row takes two hits, from wave 5 the second
  row too. A lance still kills in one.
* Zigzagging bombs from wave 2. Bombs can be shot down.
* Clearing a wave awards a bonus ship (up to five).

## Options

Environment variables, also read from a `.env` file:

* `MUSIC_DISABLED` - set to anything to turn the music off
* `VOLUME` - master volume, 0.0-1.0 (default 0.8)
* `FULLSCREEN` - set to anything to start in fullscreen

`cargo run --release -- --demo` lets a bot play.

## Code

* `src/game/` - the simulation. Pure logic at a fixed 120 Hz step with no
  window, GPU or audio dependencies; `cargo test` runs it headless.
* `src/render.rs`, `src/menu.rs` - draw the world and the title screen.
* `src/gfx.rs` - batched additive glow primitives (beams, discs, halos, fills)
  drawn with one shader. `src/font.rs` is a stroke font built on them.
* `src/post.rs` - bloom, vignette and scanlines.
* `src/audio.rs` - sound via rodio; samples are embedded in the binary.

There are no image assets: everything on screen is generated geometry.
