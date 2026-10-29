//! Mesh rundt spline-skjelettet.
//!
//! Hvert spline-hierarki (en rot med alle grener) er én kropp. Kroppen er
//! området der avstanden til en av splinene er mindre enn splinens tykkelse.
//! Grenene smeltes glatt sammen med forelderen (polynomisk glatt minimum),
//! så en forgrening blir én sammenhengende form og ikke overlappende rør.
//! Overflaten hentes ut av avstandsfeltet med «surface nets» på et jevnt
//! rutenett med ønsket kantlengde som rutestørrelse.
//!
//! Tverrsnittet er foreløpig en sirkel med splinens tykkelse som radius.
//! Endene blir halvkuler, og en kort spline med stor tykkelse er en kule.

use crate::curve::{add, scale};
use crate::mesh::{dot, length, sub, TriMesh, Vec3};
use crate::skeleton::{bezier_point, Skeleton, SplineId, WorldSpline};
use std::collections::BTreeMap;

/// Punkter per Bézier-segment når kurven gjøres om til linjestykker.
const POINTS_PER_SEGMENT: usize = 24;
/// Største antall rutepunkter i ett felt, for å holde minnebruken nede.
const MAX_GRID_POINTS: usize = 12_000_000;

struct Tube {
    points: Vec<Vec3>,
    radius: f64,
}

fn tubes(
    world: &BTreeMap<SplineId, WorldSpline>,
    skeleton: &Skeleton,
    root: SplineId,
) -> Vec<Tube> {
    skeleton
        .descendants(root)
        .into_iter()
        .filter_map(|id| {
            let w = world.get(&id)?;
            let mut points = vec![w.segments[0][0]];
            for seg in &w.segments {
                points.extend(
                    (1..=POINTS_PER_SEGMENT)
                        .map(|i| bezier_point(seg, i as f64 / POINTS_PER_SEGMENT as f64)),
                );
            }
            Some(Tube {
                points,
                radius: skeleton.spline(id)?.thickness,
            })
        })
        .filter(|t| t.radius > 0.0)
        .collect()
}

fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    let t = if l2 < 1e-24 {
        0.0
    } else {
        (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0)
    };
    length(sub(p, add(a, scale(ab, t))))
}

/// Glatt minimum med overgangsbredde `k` (Quilez). Lik `min` når k = 0.
fn smooth_min(a: f64, b: f64, k: f64) -> f64 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// Bredden på sammensmeltingen mellom en gren og resten av kroppen.
fn blend_width(radius: f64) -> f64 {
    (radius * 0.5).clamp(0.5, 6.0)
}

struct Grid {
    origin: Vec3,
    h: f64,
    n: [usize; 3],
    values: Vec<f64>,
}

impl Grid {
    fn index(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.n[1] + j) * self.n[0] + i
    }

    fn point(&self, i: usize, j: usize, k: usize) -> Vec3 {
        [
            self.origin[0] + i as f64 * self.h,
            self.origin[1] + j as f64 * self.h,
            self.origin[2] + k as f64 * self.h,
        ]
    }

    /// Rutepunktene innenfor en boks, som indeksområder.
    fn range(&self, lo: Vec3, hi: Vec3) -> [(usize, usize); 3] {
        let mut out = [(0, 0); 3];
        for (axis, r) in out.iter_mut().enumerate() {
            let a = ((lo[axis] - self.origin[axis]) / self.h).floor().max(0.0) as usize;
            let b =
                (((hi[axis] - self.origin[axis]) / self.h).ceil() as usize).min(self.n[axis] - 1);
            *r = (a, b);
        }
        out
    }
}

/// Mesh for kroppen med roten `root`, med rutestørrelse `h` i mm.
pub fn body_mesh(skeleton: &Skeleton, root: SplineId, h: f64) -> Result<TriMesh, String> {
    if !(h.is_finite() && h > 0.0) {
        return Err("Ugyldig kantlengde".into());
    }
    let world = skeleton.evaluate()?;
    let tubes = tubes(&world, skeleton, root);
    if tubes.is_empty() {
        return Ok(TriMesh::default());
    }
    // Boksen rundt alle rørene, med plass til tykkelse, sammensmelting og en rand.
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for t in &tubes {
        let pad = t.radius + blend_width(t.radius) + 2.0 * h;
        for p in &t.points {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a] - pad);
                hi[a] = hi[a].max(p[a] + pad);
            }
        }
    }
    let n = [0, 1, 2].map(|a| ((hi[a] - lo[a]) / h).ceil() as usize + 1);
    if n[0] * n[1] * n[2] > MAX_GRID_POINTS {
        return Err("Kroppen er for stor for valgt kantlengde".into());
    }
    let far = 4.0 * h;
    let mut grid = Grid {
        origin: lo,
        h,
        n,
        values: vec![f64::INFINITY; n[0] * n[1] * n[2]],
    };

    for tube in &tubes {
        let k = blend_width(tube.radius);
        let reach = tube.radius + k + 2.0 * h;
        // Avstand til denne splinen alene, i boksen rundt kurven.
        let tlo = [0, 1, 2].map(|i| {
            tube.points
                .iter()
                .map(|p| p[i])
                .fold(f64::INFINITY, f64::min)
                - reach
        });
        let thi = [0, 1, 2].map(|i| {
            tube.points
                .iter()
                .map(|p| p[i])
                .fold(f64::NEG_INFINITY, f64::max)
                + reach
        });
        let [(bi, ei), (bj, ej), (bk, ek)] = grid.range(tlo, thi);
        let (li, lj) = (ei - bi + 1, ej - bj + 1);
        let mut local = vec![f64::INFINITY; li * lj * (ek - bk + 1)];
        for w in tube.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let slo = [0, 1, 2].map(|i| a[i].min(b[i]) - reach);
            let shi = [0, 1, 2].map(|i| a[i].max(b[i]) + reach);
            let [(i0, i1), (j0, j1), (k0, k1)] = grid.range(slo, shi);
            for kk in k0..=k1 {
                for jj in j0..=j1 {
                    for ii in i0..=i1 {
                        let d = segment_distance(grid.point(ii, jj, kk), a, b) - tube.radius;
                        let e = &mut local[((kk - bk) * lj + (jj - bj)) * li + (ii - bi)];
                        *e = e.min(d);
                    }
                }
            }
        }
        for kk in bk..=ek {
            for jj in bj..=ej {
                for ii in bi..=ei {
                    let d = local[((kk - bk) * lj + (jj - bj)) * li + (ii - bi)];
                    if !d.is_finite() {
                        continue;
                    }
                    let idx = grid.index(ii, jj, kk);
                    let v = &mut grid.values[idx];
                    *v = if v.is_finite() {
                        smooth_min(*v, d, k)
                    } else {
                        d
                    };
                }
            }
        }
    }
    for v in grid.values.iter_mut() {
        *v = v.min(far);
    }
    Ok(surface_nets(&grid))
}

/// Overflaten der feltet skifter fortegn. Ett punkt per rute som krysses,
/// plassert i snittet av kantkryssingene, og én firkant per rutekant som
/// krysses. Feltet er positivt langs randen, så overflaten er lukket.
fn surface_nets(grid: &Grid) -> TriMesh {
    let [nx, ny, nz] = grid.n;
    let cell = |i: usize, j: usize, k: usize| (k * (ny - 1) + j) * (nx - 1) + i;
    let mut vertex_of = vec![u32::MAX; (nx - 1) * (ny - 1) * (nz - 1)];
    let mut positions: Vec<Vec3> = Vec::new();
    const CORNERS: [[usize; 3]; 8] = [
        [0, 0, 0],
        [1, 0, 0],
        [0, 1, 0],
        [1, 1, 0],
        [0, 0, 1],
        [1, 0, 1],
        [0, 1, 1],
        [1, 1, 1],
    ];
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for k in 0..nz - 1 {
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let v: Vec<f64> = CORNERS
                    .iter()
                    .map(|c| grid.values[grid.index(i + c[0], j + c[1], k + c[2])])
                    .collect();
                let inside = v.iter().filter(|x| **x < 0.0).count();
                if inside == 0 || inside == 8 {
                    continue;
                }
                let mut sum = [0.0; 3];
                let mut count = 0.0;
                for &(a, b) in &EDGES {
                    if (v[a] < 0.0) != (v[b] < 0.0) {
                        let t = v[a] / (v[a] - v[b]);
                        let pa =
                            grid.point(i + CORNERS[a][0], j + CORNERS[a][1], k + CORNERS[a][2]);
                        let pb =
                            grid.point(i + CORNERS[b][0], j + CORNERS[b][1], k + CORNERS[b][2]);
                        sum = add(sum, add(pa, scale(sub(pb, pa), t)));
                        count += 1.0;
                    }
                }
                vertex_of[cell(i, j, k)] = positions.len() as u32;
                positions.push(scale(sum, 1.0 / count));
            }
        }
    }

    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut quad = |q: [u32; 4], flip: bool| {
        let [a, b, c, d] = if flip { [q[0], q[3], q[2], q[1]] } else { q };
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    };
    // For hver rutekant med fortegnsskifte: firkanten av de fire rutene rundt kanten.
    for k in 1..nz - 1 {
        for j in 1..ny - 1 {
            for i in 1..nx - 1 {
                let v0 = grid.values[grid.index(i, j, k)] < 0.0;
                if i + 1 < nx && v0 != (grid.values[grid.index(i + 1, j, k)] < 0.0) {
                    let q = [
                        vertex_of[cell(i, j - 1, k - 1)],
                        vertex_of[cell(i, j, k - 1)],
                        vertex_of[cell(i, j, k)],
                        vertex_of[cell(i, j - 1, k)],
                    ];
                    quad(q, !v0);
                }
                if j + 1 < ny && v0 != (grid.values[grid.index(i, j + 1, k)] < 0.0) {
                    let q = [
                        vertex_of[cell(i - 1, j, k - 1)],
                        vertex_of[cell(i - 1, j, k)],
                        vertex_of[cell(i, j, k)],
                        vertex_of[cell(i, j, k - 1)],
                    ];
                    quad(q, !v0);
                }
                if k + 1 < nz && v0 != (grid.values[grid.index(i, j, k + 1)] < 0.0) {
                    let q = [
                        vertex_of[cell(i - 1, j - 1, k)],
                        vertex_of[cell(i, j - 1, k)],
                        vertex_of[cell(i, j, k)],
                        vertex_of[cell(i - 1, j, k)],
                    ];
                    quad(q, !v0);
                }
            }
        }
    }
    TriMesh {
        positions,
        triangles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::SkeletonEdit as E;
    use std::f64::consts::PI;

    fn sphere(r: f64) -> (Skeleton, SplineId) {
        let mut sk = Skeleton::new();
        let id = sk
            .apply(&E::AddRoot {
                start: [0.0, 25.0, 0.0],
                direction: [0.0, 1.0, 0.0],
                length: 0.5,
                thickness: r,
            })
            .unwrap()
            .unwrap();
        (sk, id)
    }

    #[test]
    fn kort_spline_med_tykkelse_gir_en_lukket_kule() {
        let (sk, id) = sphere(25.0);
        let m = body_mesh(&sk, id, 1.5).unwrap();
        assert!(m.is_closed_manifold());
        let v = m.signed_volume();
        let expected = 4.0 / 3.0 * PI * 25.0f64.powi(3);
        assert!(v > 0.0, "utoverpekende trekanter");
        assert!((v - expected).abs() / expected < 0.05, "{v} mot {expected}");
        // Alle punkter ligger nær kuleflaten rundt splinen.
        for p in &m.positions {
            let r = length(sub(*p, [0.0, 25.25, 0.0]));
            assert!((r - 25.0).abs() < 1.0, "{r}");
        }
    }

    #[test]
    fn gren_smelter_sammen_med_forelderen_til_en_kropp() {
        let (mut sk, id) = sphere(20.0);
        sk.apply(&E::AddChild {
            parent: id,
            at: 1.0,
            direction: [1.0, 0.0, 0.0],
            length: 40.0,
            thickness: 6.0,
        })
        .unwrap();
        let m = body_mesh(&sk, id, 1.5).unwrap();
        assert!(m.is_closed_manifold());
        // Én sammenhengende flate: alle trekanter nås fra den første.
        assert_eq!(components(&m), 1);
        // Grenen stikker ut til x = 40 + 6 mm.
        let max_x = m.positions.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        assert!((max_x - 46.0).abs() < 1.0, "{max_x}");
        assert!(m.signed_volume() > 4.0 / 3.0 * PI * 20.0f64.powi(3));
    }

    #[test]
    fn separate_rotter_er_separate_kropper() {
        let (mut sk, a) = sphere(10.0);
        let b = sk
            .apply(&E::AddRoot {
                start: [100.0, 0.0, 0.0],
                direction: [0.0, 1.0, 0.0],
                length: 30.0,
                thickness: 5.0,
            })
            .unwrap()
            .unwrap();
        let ma = body_mesh(&sk, a, 1.5).unwrap();
        assert!(ma.positions.iter().all(|p| p[0] < 20.0));
        let mb = body_mesh(&sk, b, 1.5).unwrap();
        assert!(mb.positions.iter().all(|p| p[0] > 80.0));
    }

    fn components(m: &TriMesh) -> usize {
        let mut parent: Vec<usize> = (0..m.positions.len()).collect();
        fn find(p: &mut [usize], x: usize) -> usize {
            let mut r = x;
            while p[r] != r {
                r = p[r];
            }
            p[x] = r;
            r
        }
        for t in &m.triangles {
            for k in 0..3 {
                let (a, b) = (
                    find(&mut parent, t[k] as usize),
                    find(&mut parent, t[(k + 1) % 3] as usize),
                );
                parent[a] = b;
            }
        }
        let mut roots: Vec<usize> = (0..m.positions.len())
            .map(|i| find(&mut parent, i))
            .collect();
        roots.sort_unstable();
        roots.dedup();
        roots.len()
    }
}
