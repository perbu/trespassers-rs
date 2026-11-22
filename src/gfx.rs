//! Batched additive "glow" primitives. Every shape in the game is built from
//! soft-edged line quads, discs, halos and flat fills, all drawn with a single
//! shader so the whole scene goes out in a handful of draw calls.

use macroquad::miniquad::{BlendFactor, BlendState, Equation, PipelineParams};
use macroquad::prelude::*;

use crate::font;

// macroquad silently drops meshes at or above 10000 vertices / 5000 indices.
const MAX_INDICES: usize = 4900;
const MAX_VERTICES: usize = 9000;

// The fragment shader picks a brightness profile from the u coordinate.
const HALO_U: f32 = 10.0;
const FILL_U: f32 = 20.0;

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying vec2 uv;
varying vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

const FRAGMENT: &str = r#"#version 100
precision highp float;
varying vec2 uv;
varying vec4 color;
void main() {
    vec3 c;
    if (uv.x > 15.0) {
        // flat fill
        c = color.rgb * color.a;
    } else if (uv.x > 5.0) {
        // wide soft halo
        float k = max(0.0, 1.0 - length(vec2(uv.x - 10.0, uv.y)));
        c = color.rgb * color.a * k * k;
    } else {
        // beam: hot core with a short falloff
        float d = length(uv);
        float core = smoothstep(0.5, 0.28, d);
        float glow = max(0.0, 1.0 - d);
        c = color.rgb * color.a * (core + glow * glow * 0.4) + vec3(core * core * 0.16 * color.a);
    }
    gl_FragColor = vec4(c, 1.0);
}"#;

pub struct Gfx {
    chunks: Vec<(Vec<Vertex>, Vec<u16>)>,
    material: Material,
}

impl Gfx {
    pub fn new() -> Gfx {
        let additive = BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One);
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(additive),
                    alpha_blend: Some(additive),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("glow shader");
        Gfx {
            chunks: vec![(Vec::new(), Vec::new())],
            material,
        }
    }

    /// Draws everything queued since the last flush into the current camera.
    pub fn flush(&mut self) {
        gl_use_material(&self.material);
        for (vertices, indices) in &mut self.chunks {
            if indices.is_empty() {
                continue;
            }
            let mesh = Mesh {
                vertices: std::mem::take(vertices),
                indices: std::mem::take(indices),
                texture: None,
            };
            draw_mesh(&mesh);
            // keep the allocations for the next frame
            *vertices = mesh.vertices;
            *indices = mesh.indices;
            vertices.clear();
            indices.clear();
        }
        gl_use_default_material();
        self.chunks.truncate(1);
    }

    fn reserve(&mut self, vertices: usize, indices: usize) -> &mut (Vec<Vertex>, Vec<u16>) {
        let last = self.chunks.last().unwrap();
        if last.0.len() + vertices > MAX_VERTICES || last.1.len() + indices > MAX_INDICES {
            self.chunks.push((Vec::new(), Vec::new()));
        }
        self.chunks.last_mut().unwrap()
    }

    fn quad(&mut self, p: [Vec2; 4], uv: [Vec2; 4], color: Color) {
        let (vertices, indices) = self.reserve(4, 6);
        let base = vertices.len() as u16;
        for i in 0..4 {
            vertices.push(Vertex::new(p[i].x, p[i].y, 0.0, uv[i].x, uv[i].y, color));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn half_extent(width: f32) -> f32 {
        width * 1.5 + 1.0
    }

    /// Line segment without end caps; use for segments of smooth curves.
    pub fn seg(&mut self, a: Vec2, b: Vec2, width: f32, color: Color) {
        let h = Self::half_extent(width);
        let n = (b - a).normalize_or_zero().perp() * h;
        self.quad(
            [a + n, b + n, b - n, a - n],
            [vec2(0., 1.), vec2(0., 1.), vec2(0., -1.), vec2(0., -1.)],
            color,
        );
    }

    /// Line segment with round caps.
    pub fn line(&mut self, a: Vec2, b: Vec2, width: f32, color: Color) {
        let h = Self::half_extent(width);
        let dir = (b - a).try_normalize().unwrap_or(Vec2::X);
        let n = dir.perp() * h;
        let e = dir * h;
        let (top, bot) = (vec2(0., 1.), vec2(0., -1.));
        self.quad(
            [a - e + n, a + n, a - n, a - e - n],
            [vec2(1., 1.), top, bot, vec2(1., -1.)],
            color,
        );
        self.quad([a + n, b + n, b - n, a - n], [top, top, bot, bot], color);
        self.quad(
            [b + n, b + e + n, b + e - n, b - n],
            [top, vec2(1., 1.), vec2(1., -1.), bot],
            color,
        );
    }

    pub fn polyline(&mut self, points: &[Vec2], closed: bool, width: f32, color: Color) {
        for pair in points.windows(2) {
            self.line(pair[0], pair[1], width, color);
        }
        if closed && points.len() > 2 {
            self.line(points[points.len() - 1], points[0], width, color);
        }
    }

    pub fn arc(&mut self, c: Vec2, r: f32, from: f32, to: f32, width: f32, color: Color) {
        let span = (to - from).abs();
        let n = ((r * span * 0.18) as usize).clamp(3, 72);
        let mut prev = c + Vec2::from_angle(from) * r;
        for i in 1..=n {
            let p = c + Vec2::from_angle(from + (to - from) * i as f32 / n as f32) * r;
            self.seg(prev, p, width, color);
            prev = p;
        }
    }

    pub fn circle(&mut self, c: Vec2, r: f32, width: f32, color: Color) {
        self.arc(c, r, 0.0, std::f32::consts::TAU, width, color);
    }

    /// Regular polygon outline with `n` corners.
    pub fn ngon(&mut self, c: Vec2, r: f32, n: usize, rot: f32, width: f32, color: Color) {
        let points = ngon_points(c, r, n, rot);
        self.polyline(&points, true, width, color);
    }

    pub fn fill_poly(&mut self, points: &[Vec2], color: Color) {
        if points.len() < 3 {
            return;
        }
        let count = points.len();
        let (vertices, indices) = self.reserve(count, (count - 2) * 3);
        let base = vertices.len() as u16;
        for p in points {
            vertices.push(Vertex::new(p.x, p.y, 0.0, FILL_U, 0.0, color));
        }
        for i in 1..count as u16 - 1 {
            indices.extend_from_slice(&[base, base + i, base + i + 1]);
        }
    }

    pub fn fill_ngon(&mut self, c: Vec2, r: f32, n: usize, rot: f32, color: Color) {
        self.fill_poly(&ngon_points(c, r, n, rot), color);
    }

    /// Small bright disc.
    pub fn dot(&mut self, c: Vec2, r: f32, color: Color) {
        let h = r * 2.0;
        self.quad(
            [c + vec2(-h, -h), c + vec2(h, -h), c + vec2(h, h), c + vec2(-h, h)],
            [vec2(-1., -1.), vec2(1., -1.), vec2(1., 1.), vec2(-1., 1.)],
            color,
        );
    }

    /// Wide, soft radial glow.
    pub fn halo(&mut self, c: Vec2, r: f32, color: Color) {
        let u = HALO_U;
        self.quad(
            [c + vec2(-r, -r), c + vec2(r, -r), c + vec2(r, r), c + vec2(-r, r)],
            [vec2(u - 1., -1.), vec2(u + 1., -1.), vec2(u + 1., 1.), vec2(u - 1., 1.)],
            color,
        );
    }

    /// Stroke-font text with its top-left corner at `pos`; `size` is the cap height.
    pub fn text(&mut self, s: &str, pos: Vec2, size: f32, color: Color) {
        let unit = size / font::HEIGHT;
        let width = (size * 0.085).max(1.1);
        let mut x = pos.x;
        for ch in s.chars() {
            for stroke in font::glyph(ch) {
                let mut prev: Option<Vec2> = None;
                for &(gx, gy) in *stroke {
                    let p = vec2(x + gx as f32 * unit, pos.y + gy as f32 * unit);
                    if let Some(q) = prev {
                        self.line(q, p, width, color);
                    }
                    prev = Some(p);
                }
            }
            x += font::ADVANCE * unit;
        }
    }

    pub fn text_centered(&mut self, s: &str, center_x: f32, y: f32, size: f32, color: Color) {
        let w = font::width(s, size);
        self.text(s, vec2(center_x - w / 2.0, y), size, color);
    }

    pub fn text_right(&mut self, s: &str, right_x: f32, y: f32, size: f32, color: Color) {
        let w = font::width(s, size);
        self.text(s, vec2(right_x - w, y), size, color);
    }
}

pub fn ngon_points(c: Vec2, r: f32, n: usize, rot: f32) -> Vec<Vec2> {
    (0..n)
        .map(|i| c + Vec2::from_angle(rot + std::f32::consts::TAU * i as f32 / n as f32) * r)
        .collect()
}
