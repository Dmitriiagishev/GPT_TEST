use std::ops::{Add, Div, Mul, Sub};

#[derive(Clone, Copy, Debug, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Self {
        self / self.length()
    }

    /// Rigid rotation: the same transform is used for vertices and normals.
    pub fn rotated(self, time: f64) -> Self {
        let (sx, cx) = (0.35 + time * 0.31).sin_cos();
        let (sy, cy) = (0.45 + time * 0.47).sin_cos();
        let (sz, cz) = (time * 0.13).sin_cos();
        let v = Self::new(self.x, self.y * cx - self.z * sx, self.y * sx + self.z * cx);
        let v = Self::new(v.x * cy + v.z * sy, v.y, -v.x * sy + v.z * cy);
        Self::new(v.x * cz - v.y * sz, v.x * sz + v.y * cz, v.z)
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, rhs: f64) -> Self {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

pub struct Face {
    /// Counterclockwise vertex order as seen from outside the solid.
    pub indices: Vec<usize>,
    pub normal: Vec3,
    pub center: Vec3,
}

pub struct Dodecahedron {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Face>,
}

impl Dodecahedron {
    pub fn new() -> Self {
        let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let mut vertices = Vec::with_capacity(20);
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    vertices.push(Vec3::new(x, y, z));
                }
            }
        }
        for a in [-1.0, 1.0] {
            for b in [-1.0, 1.0] {
                vertices.push(Vec3::new(0.0, a / phi, b * phi));
                vertices.push(Vec3::new(a / phi, b * phi, 0.0));
                vertices.push(Vec3::new(b * phi, 0.0, a / phi));
            }
        }

        // A face is a supporting plane containing exactly five vertices.
        // Derive the convex hull once instead of maintaining an index table.
        let mut faces = Vec::with_capacity(12);
        let mut seen = Vec::new();
        const EPS: f64 = 1e-8;
        for i in 0..vertices.len() {
            for j in i + 1..vertices.len() {
                for k in j + 1..vertices.len() {
                    let cross = (vertices[j] - vertices[i]).cross(vertices[k] - vertices[i]);
                    if cross.length() < EPS {
                        continue;
                    }
                    let mut normal = cross.normalized();
                    let distances: Vec<_> = vertices
                        .iter()
                        .map(|&v| normal.dot(v - vertices[i]))
                        .collect();
                    if distances.iter().all(|&d| d >= -EPS) {
                        normal = normal * -1.0;
                    } else if !distances.iter().all(|&d| d <= EPS) {
                        continue;
                    }
                    let mut indices: Vec<_> = distances
                        .iter()
                        .enumerate()
                        .filter_map(|(index, d)| (d.abs() < EPS).then_some(index))
                        .collect();
                    if indices.len() != 5 || seen.contains(&indices) {
                        continue;
                    }
                    seen.push(indices.clone());
                    let center = indices
                        .iter()
                        .fold(Vec3::default(), |sum, &v| sum + vertices[v])
                        / 5.0;
                    let u = (vertices[indices[0]] - center).normalized();
                    let v = normal.cross(u);
                    indices.sort_by(|&a, &b| {
                        let a = vertices[a] - center;
                        let b = vertices[b] - center;
                        a.dot(v)
                            .atan2(a.dot(u))
                            .total_cmp(&b.dot(v).atan2(b.dot(u)))
                    });
                    faces.push(Face {
                        indices,
                        normal,
                        center,
                    });
                }
            }
        }
        Self { vertices, faces }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn regular_closed_dodecahedron_has_twelve_outward_pentagons() {
        let solid = Dodecahedron::new();
        assert_eq!(solid.vertices.len(), 20);
        assert_eq!(solid.faces.len(), 12);
        let mut edges = BTreeMap::new();
        let expected_edge = 4.0 / (1.0 + 5.0_f64.sqrt());
        for face in &solid.faces {
            assert_eq!(face.indices.len(), 5);
            assert!((face.normal.length() - 1.0).abs() < 1e-8);
            assert!(face.normal.dot(face.center) > 0.0);
            for &point in &solid.vertices {
                assert!(face.normal.dot(point - face.center) < 1e-8);
            }
            for edge in 0..5 {
                let a = face.indices[edge];
                let b = face.indices[(edge + 1) % 5];
                assert!((face.normal.dot(solid.vertices[a] - face.center)).abs() < 1e-8);
                assert!(
                    ((solid.vertices[a] - solid.vertices[b]).length() - expected_edge).abs() < 1e-8
                );
                assert!(
                    (solid.vertices[a] - face.center)
                        .cross(solid.vertices[b] - face.center)
                        .dot(face.normal)
                        > 0.0
                );
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        assert_eq!(edges.len(), 30);
        assert!(edges.values().all(|&count| count == 2));
    }
}
