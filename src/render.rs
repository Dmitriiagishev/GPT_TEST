use crate::geometry::{Dodecahedron, Vec3};
use std::fmt::Write;

const CAMERA_Z: f64 = 5.5;
const PALETTE: [[u8; 3]; 12] = [
    [255, 110, 110],
    [255, 184, 82],
    [250, 221, 97],
    [124, 224, 121],
    [74, 218, 185],
    [84, 203, 249],
    [123, 152, 255],
    [181, 131, 247],
    [241, 126, 203],
    [249, 155, 115],
    [142, 232, 224],
    [220, 190, 255],
];
const RAMP: &[u8] = b".:-=+*#%@";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub glyph: u8,
    pub rgb: [u8; 3],
}

#[derive(Clone, Copy)]
struct Projected {
    x: f64,
    y: f64,
    inverse_depth: f64,
}

pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    depth: Vec<f64>,
}

/// Light stays above and to the viewer's left while the object rotates.
fn illumination(normal: Vec3) -> f64 {
    let light = Vec3::new(-0.6, 0.85, 1.0).normalized();
    0.22 + 0.78 * normal.dot(light).max(0.0)
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width * height],
            depth: vec![0.0; width * height],
        }
    }

    pub fn render(&mut self, solid: &Dodecahedron, time: f64, cell_aspect: f64) {
        self.cells.fill(Cell::default());
        self.depth.fill(0.0);
        if self.width == 0 || self.height == 0 {
            return;
        }
        // Fit the perspective-projected bounding sphere into both dimensions.
        // cell_aspect is cell height / cell width, usually about 2.
        let radius = 3.0_f64.sqrt();
        let apparent_radius = radius * CAMERA_Z / (CAMERA_Z * CAMERA_Z - radius * radius).sqrt();
        let scale = (self.width as f64 / cell_aspect).min(self.height as f64)
            / (2.0 * apparent_radius * 1.08);
        let project = |v: Vec3| {
            let inverse_depth = 1.0 / (CAMERA_Z - v.z);
            Projected {
                x: self.width as f64 / 2.0 + v.x * inverse_depth * CAMERA_Z * scale * cell_aspect,
                y: self.height as f64 / 2.0 - v.y * inverse_depth * CAMERA_Z * scale,
                inverse_depth,
            }
        };
        let projected: Vec<_> = solid
            .vertices
            .iter()
            .map(|&v| project(v.rotated(time)))
            .collect();
        for (index, face) in solid.faces.iter().enumerate() {
            let normal = face.normal.rotated(time);
            let center = face.center.rotated(time);
            if normal.dot(Vec3::new(0.0, 0.0, CAMERA_Z) - center) <= 0.0 {
                continue;
            }
            let brightness = illumination(normal);
            let cell = Cell {
                glyph: RAMP[(brightness * (RAMP.len() - 1) as f64).round() as usize],
                rgb: PALETTE[index].map(|c| (c as f64 * brightness).round() as u8),
            };
            // A fan of three triangles fills each convex pentagon.
            for edge in 1..face.indices.len() - 1 {
                self.triangle(
                    [
                        projected[face.indices[0]],
                        projected[face.indices[edge]],
                        projected[face.indices[edge + 1]],
                    ],
                    cell,
                );
            }
        }
    }

    fn triangle(&mut self, vertices: [Projected; 3], cell: Cell) {
        let [a, b, c] = vertices;
        let edge = |a: Projected, b: Projected, x: f64, y: f64| {
            (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
        };
        let area = edge(a, b, c.x, c.y);
        if area.abs() < 1e-10 {
            return;
        }
        let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
        let max_x = a.x.max(b.x).max(c.x).ceil().min(self.width as f64) as usize;
        let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
        let max_y = a.y.max(b.y).max(c.y).ceil().min(self.height as f64) as usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let px = x as f64 + 0.5;
                let py = y as f64 + 0.5;
                let weights = [
                    edge(b, c, px, py) / area,
                    edge(c, a, px, py) / area,
                    edge(a, b, px, py) / area,
                ];
                if weights.iter().any(|&w| w < -1e-9) {
                    continue;
                }
                // Reciprocal depth is linear in screen space under perspective.
                let inverse_depth = weights[0] * a.inverse_depth
                    + weights[1] * b.inverse_depth
                    + weights[2] * c.inverse_depth;
                let offset = y * self.width + x;
                if inverse_depth > self.depth[offset] {
                    self.depth[offset] = inverse_depth;
                    self.cells[offset] = cell;
                }
            }
        }
    }

    /// ASCII characters with ANSI foreground colors and a black background.
    pub fn write_ansi(&self, output: &mut String) {
        let mut color = None;
        output.push_str("\x1b[0;40m");
        for y in 0..self.height {
            if y > 0 {
                output.push_str("\r\n");
            }
            for cell in &self.cells[y * self.width..(y + 1) * self.width] {
                if cell.glyph == 0 {
                    output.push(' ');
                    continue;
                }
                if color != Some(cell.rgb) {
                    let [r, g, b] = cell.rgb;
                    write!(output, "\x1b[38;2;{r};{g};{b}m")
                        .expect("writing to a String cannot fail");
                    color = Some(cell.rgb);
                }
                output.push(cell.glyph as char);
            }
        }
        output.push_str("\x1b[0m");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_comes_from_the_upper_left() {
        assert!(illumination(Vec3::new(-1.0, 0.0, 0.0)) > illumination(Vec3::new(1.0, 0.0, 0.0)));
        assert!(illumination(Vec3::new(0.0, 1.0, 0.0)) > illumination(Vec3::new(0.0, -1.0, 0.0)));
    }

    #[test]
    fn filled_solid_fits_at_different_sizes_and_rotations() {
        let solid = Dodecahedron::new();
        for (width, height, aspect) in
            [(80, 23, 2.0), (160, 49, 2.0), (30, 60, 2.0), (120, 40, 1.0)]
        {
            let mut canvas = Canvas::new(width, height);
            for step in 0..60 {
                canvas.render(&solid, step as f64 * 0.2, aspect);
                let filled = canvas.cells.iter().filter(|cell| cell.glyph != 0).count();
                assert!(
                    filled > width * height / 20,
                    "missing solid at {width}x{height}"
                );
                assert!(canvas.cells[..width].iter().all(|c| c.glyph == 0));
                assert!(
                    canvas.cells[(height - 1) * width..]
                        .iter()
                        .all(|c| c.glyph == 0)
                );
                for row in canvas.cells.chunks(width) {
                    assert_eq!(row[0].glyph, 0);
                    assert_eq!(row[width - 1].glyph, 0);
                    // Convex projection: no holes between the silhouette edges.
                    if let (Some(first), Some(last)) = (
                        row.iter().position(|c| c.glyph != 0),
                        row.iter().rposition(|c| c.glyph != 0),
                    ) {
                        assert!(row[first..=last].iter().all(|c| c.glyph != 0));
                    }
                }
            }
        }
    }

    #[test]
    fn z_buffer_retains_the_nearest_triangle_in_either_draw_order() {
        let mut canvas = Canvas::new(8, 8);
        let triangle = |inverse_depth| {
            [
                Projected {
                    x: 0.0,
                    y: 0.0,
                    inverse_depth,
                },
                Projected {
                    x: 8.0,
                    y: 0.0,
                    inverse_depth,
                },
                Projected {
                    x: 0.0,
                    y: 8.0,
                    inverse_depth,
                },
            ]
        };
        let far = Cell {
            glyph: b'.',
            rgb: [100, 0, 0],
        };
        let near = Cell {
            glyph: b'#',
            rgb: [0, 255, 0],
        };
        for reverse in [false, true] {
            canvas.cells.fill(Cell::default());
            canvas.depth.fill(0.0);
            let layers = if reverse {
                [(0.5, near), (0.2, far)]
            } else {
                [(0.2, far), (0.5, near)]
            };
            for (depth, cell) in layers {
                canvas.triangle(triangle(depth), cell);
            }
            assert_eq!(canvas.cells[2 * 8 + 2], near);
        }
    }

    #[test]
    fn resizing_scales_the_solid_and_animation_changes_the_frame() {
        let solid = Dodecahedron::new();
        let mut small = Canvas::new(80, 24);
        let mut large = Canvas::new(160, 48);
        small.render(&solid, 0.0, 2.0);
        large.render(&solid, 0.0, 2.0);
        let count = |canvas: &Canvas| canvas.cells.iter().filter(|c| c.glyph != 0).count();
        let ratio = count(&large) as f64 / count(&small) as f64;
        assert!((3.7..4.3).contains(&ratio));
        let original = small.cells.clone();
        small.render(&solid, 1.0, 2.0);
        assert_ne!(original, small.cells);
    }

    #[test]
    fn tiny_and_empty_viewports_do_not_panic() {
        let solid = Dodecahedron::new();
        for (w, h) in [(0, 0), (1, 1), (2, 1), (1, 10)] {
            let mut canvas = Canvas::new(w, h);
            canvas.render(&solid, 0.0, 2.0);
            canvas.write_ansi(&mut String::new());
        }
    }
}
