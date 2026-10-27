//! Enkel indeksert trekantmesh med kontroller av topologien.

use std::collections::HashMap;

pub type Vec3 = [f64; 3];

#[derive(Clone, Debug, Default)]
pub struct TriMesh {
    pub positions: Vec<Vec3>,
    /// Trekanter med punktindekser i mot-klokka rekkefølge sett utenfra.
    pub triangles: Vec<[u32; 3]>,
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn length(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

pub fn normalize(a: Vec3) -> Vec3 {
    let l = length(a);
    if l == 0.0 {
        a
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

impl TriMesh {
    /// Arealvektede punktnormaler.
    pub fn vertex_normals(&self) -> Vec<Vec3> {
        let mut normals = vec![[0.0; 3]; self.positions.len()];
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.positions[i as usize]);
            let n = cross(sub(b, a), sub(c, a));
            for &i in t {
                let acc = &mut normals[i as usize];
                acc[0] += n[0];
                acc[1] += n[1];
                acc[2] += n[2];
            }
        }
        normals.into_iter().map(normalize).collect()
    }

    /// Sann når hver kant deles av nøyaktig to trekanter med motsatt retning,
    /// dvs. en lukket, konsistent orientert 2-manifold.
    pub fn is_closed_manifold(&self) -> bool {
        let mut directed: HashMap<(u32, u32), u32> = HashMap::new();
        for t in &self.triangles {
            for k in 0..3 {
                *directed.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
            }
        }
        directed
            .iter()
            .all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1))
    }

    pub fn edge_count(&self) -> usize {
        let mut edges = std::collections::HashSet::new();
        for t in &self.triangles {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                edges.insert((a.min(b), a.max(b)));
            }
        }
        edges.len()
    }

    /// Lukket volum med fortegn (positivt for utovervendte trekanter).
    pub fn signed_volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.positions[i as usize]);
                dot(a, cross(b, c)) / 6.0
            })
            .sum()
    }
}
