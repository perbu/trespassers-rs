//! Post-processing: the scene is rendered to an offscreen target, blurred
//! through a chain of progressively smaller targets and composited back with
//! bloom, a vignette and a hint of scanlines.

use macroquad::prelude::*;

use crate::game::{H, W};

const LEVELS: usize = 4;

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
varying vec2 uv;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}"#;

const BLUR: &str = r#"#version 100
precision highp float;
varying vec2 uv;
uniform sampler2D Texture;
uniform vec2 Step;
void main() {
    vec3 c = texture2D(Texture, uv).rgb * 0.2270270270;
    c += (texture2D(Texture, uv + Step * 1.3846153846).rgb
        + texture2D(Texture, uv - Step * 1.3846153846).rgb) * 0.3162162162;
    c += (texture2D(Texture, uv + Step * 3.2307692308).rgb
        + texture2D(Texture, uv - Step * 3.2307692308).rgb) * 0.0702702703;
    gl_FragColor = vec4(c, 1.0);
}"#;

const COMPOSITE: &str = r#"#version 100
precision highp float;
varying vec2 uv;
uniform sampler2D Texture;
uniform sampler2D Bloom0;
uniform sampler2D Bloom1;
uniform sampler2D Bloom2;
uniform sampler2D Bloom3;
void main() {
    vec2 d = uv - 0.5;
    // slight colour fringing toward the corners
    vec2 fringe = d * dot(d, d) * 0.004;
    vec3 c = vec3(
        texture2D(Texture, uv + fringe).r,
        texture2D(Texture, uv).g,
        texture2D(Texture, uv - fringe).b);
    c += texture2D(Bloom0, uv).rgb * 0.5
       + texture2D(Bloom1, uv).rgb * 0.55
       + texture2D(Bloom2, uv).rgb * 0.6
       + texture2D(Bloom3, uv).rgb * 0.7;
    float v = length(d * vec2(1.0, 0.82));
    // deep navy creeping in from the edges
    c += vec3(0.003, 0.008, 0.026) * smoothstep(0.1, 0.7, v);
    // Tone-map on the brightest channel so saturated neon keeps its hue,
    // then let only the very hottest spots bleed toward white.
    float m = max(c.r, max(c.g, c.b));
    c *= (1.0 - exp(-m * 1.6)) / max(m, 0.0001);
    c = mix(c, vec3(1.0), 0.45 * smoothstep(1.6, 5.0, m));
    c *= 1.0 - 0.3 * smoothstep(0.45, 0.9, v);
    c *= 0.965 + 0.035 * sin(gl_FragCoord.y * 3.14159265);
    gl_FragColor = vec4(c, 1.0);
}"#;

pub struct Post {
    scene: RenderTarget,
    ping: Vec<RenderTarget>,
    pong: Vec<RenderTarget>,
    blur_h: Vec<Material>,
    blur_v: Vec<Material>,
    composite: Material,
}

fn target(w: f32, h: f32) -> RenderTarget {
    let rt = render_target(w as u32, h as u32);
    rt.texture.set_filter(FilterMode::Linear);
    rt
}

fn blur_material(step: Vec2) -> Material {
    let material = load_material(
        ShaderSource::Glsl {
            vertex: VERTEX,
            fragment: BLUR,
        },
        MaterialParams {
            uniforms: vec![UniformDesc::new("Step", UniformType::Float2)],
            ..Default::default()
        },
    )
    .expect("blur shader");
    material.set_uniform("Step", step);
    material
}

impl Post {
    /// `supersample` is the scene resolution relative to the logical 1600x900.
    pub fn new(supersample: f32) -> Post {
        let scene = target(W * supersample, H * supersample);
        let (mut ping, mut pong, mut blur_h, mut blur_v) = (vec![], vec![], vec![], vec![]);
        for level in 0..LEVELS {
            let scale = 0.5f32.powi(level as i32 + 1);
            let (w, h) = ((W * scale).round(), (H * scale).round());
            ping.push(target(w, h));
            pong.push(target(w, h));
            blur_h.push(blur_material(vec2(1.0 / w, 0.0)));
            blur_v.push(blur_material(vec2(0.0, 1.0 / h)));
        }
        let composite = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: COMPOSITE,
            },
            MaterialParams {
                textures: (0..LEVELS).map(|i| format!("Bloom{i}")).collect(),
                ..Default::default()
            },
        )
        .expect("composite shader");
        Post {
            scene,
            ping,
            pong,
            blur_h,
            blur_v,
            composite,
        }
    }

    /// Routes drawing into the scene target, in logical coordinates.
    pub fn begin(&self, shake: Vec2) {
        set_camera(&Camera2D {
            render_target: Some(self.scene.clone()),
            ..Camera2D::from_display_rect(Rect::new(shake.x, shake.y, W, H))
        });
        clear_background(BLACK);
    }

    /// Blurs the scene and presents it letterboxed on screen.
    pub fn end(&self) {
        for level in 0..LEVELS {
            let source = if level == 0 {
                &self.scene.texture
            } else {
                &self.pong[level - 1].texture
            };
            pass(source, &self.ping[level], &self.blur_h[level]);
            pass(&self.ping[level].texture, &self.pong[level], &self.blur_v[level]);
            self.composite
                .set_texture(&format!("Bloom{level}"), self.pong[level].texture.clone());
        }

        set_default_camera();
        clear_background(BLACK);
        let scale = (screen_width() / W).min(screen_height() / H);
        let size = vec2(W, H) * scale;
        let origin = (vec2(screen_width(), screen_height()) - size) / 2.0;
        gl_use_material(&self.composite);
        draw_texture_ex(
            &self.scene.texture,
            origin.x,
            origin.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(size),
                // render targets come out upside down relative to the screen
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
    }
}

fn pass(source: &Texture2D, dest: &RenderTarget, material: &Material) {
    set_camera(&Camera2D {
        render_target: Some(dest.clone()),
        ..Camera2D::from_display_rect(Rect::new(0.0, 0.0, 1.0, 1.0))
    });
    gl_use_material(material);
    draw_texture_ex(
        source,
        0.0,
        0.0,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(1.0, 1.0)),
            ..Default::default()
        },
    );
    gl_use_default_material();
}
