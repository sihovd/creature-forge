//! Ferdige 3D-mesher som settes inn på en spline (horn, klør, øyne …).
//!
//! En del er en mesh med høyde 1 langs +Y og foten i origo. Den plasseres i
//! rammen til splinen ved andelen `at`: langs kurven, eller ut fra
//! overflaten. Delene legges til kroppens mesh etter at overflaten er laget,
//! og speiles med resten når speilmodus er på.

use crate::curve::{add, scale};
use crate::mesh::{cross, normalize, TriMesh, Vec3};
use crate::skeleton::{Frame, Part, PartSource, Skeleton, SplineId, WorldSpline};
use std::collections::BTreeMap;

/// De innebygde delene, i rekkefølgen de vises.
pub const BUILTIN: [&str; 5] = ["Horn", "Klo", "Pigg", "Tann", "Øye"];

const AROUND: usize = 24;
const RINGS: usize = 16;

/// En dreid form: radius `r(y)` for y i [0, 1], bøyd `bend(y)` langs x og
/// klemt sammen langs z med `flat`.
fn lathe(r: impl Fn(f64) -> f64, bend: impl Fn(f64) -> f64, flat: f64) -> TriMesh {
    let mut m = TriMesh::default();
    m.positions.push([bend(0.0), 0.0, 0.0]);
    for j in 1..RINGS {
        let y = j as f64 / RINGS as f64;
        let (rad, x0) = (r(y), bend(y));
        for i in 0..AROUND {
            let a = i as f64 / AROUND as f64 * std::f64::consts::TAU;
            m.positions
                .push([x0 + rad * a.cos(), y, -rad * a.sin() * flat]);
        }
    }
    m.positions.push([bend(1.0), 1.0, 0.0]);
    let ring = |j: usize, i: usize| (1 + (j - 1) * AROUND + i % AROUND) as u32;
    let top = (m.positions.len() - 1) as u32;
    for i in 0..AROUND {
        m.triangles.push([0, ring(1, i + 1), ring(1, i)]);
        for j in 1..RINGS - 1 {
            let (a, b, c, d) = (
                ring(j, i),
                ring(j, i + 1),
                ring(j + 1, i),
                ring(j + 1, i + 1),
            );
            m.triangles.push([a, b, d]);
            m.triangles.push([a, d, c]);
        }
        m.triangles
            .push([ring(RINGS - 1, i), ring(RINGS - 1, i + 1), top]);
    }
    if m.signed_volume() < 0.0 {
        for t in m.triangles.iter_mut() {
            t.swap(1, 2);
        }
    }
    m
}

/// En innebygd del, eller `None` for et ukjent navn.
pub fn builtin(name: &str) -> Option<TriMesh> {
    Some(match name {
        "Horn" => lathe(|y| 0.3 * (1.0 - y).powf(0.9), |y| 0.45 * y * y, 1.0),
        "Klo" => lathe(|y| 0.22 * (1.0 - y).powf(0.8), |y| 0.55 * y * y, 0.55),
        "Pigg" => lathe(|y| 0.25 * (1.0 - y), |_| 0.0, 1.0),
        "Tann" => lathe(|y| 0.3 * (1.0 - y).powf(0.6), |y| 0.12 * y * y, 0.7),
        "Øye" => lathe(|y| (y * (1.0 - y)).max(0.0).sqrt(), |_| 0.0, 1.0),
        _ => return None,
    })
}

/// Meshen til en del, før den plasseres.
pub fn source_mesh(skeleton: &Skeleton, source: &PartSource) -> Option<TriMesh> {
    match source {
        PartSource::Builtin { name } => builtin(name),
        PartSource::Stored { id } => skeleton.part_mesh(*id).map(|m| TriMesh {
            positions: m.positions.clone(),
            triangles: m.triangles.clone(),
        }),
    }
}

/// Foten og aksene til en del: `y` er delens høyderetning, `x` retningen en
/// bøyd del bøyer mot, og `z` fullfører en høyrehendt ramme.
pub fn placement(frame: &Frame, part: &Part, thickness: f64) -> (Vec3, [Vec3; 3]) {
    let (sin, cos) = part.rotation.sin_cos();
    let around = normalize(add(scale(frame.u, cos), scale(frame.v, sin)));
    if part.outward {
        // Ut fra overflaten, litt senket inn i kroppen.
        let base = add(frame.origin, scale(around, thickness * 0.8));
        let x = frame.t;
        (base, [x, around, normalize(cross(x, around))])
    } else {
        let y = frame.t;
        (frame.origin, [around, y, normalize(cross(around, y))])
    }
}

fn place(mesh: &TriMesh, base: Vec3, axes: [Vec3; 3], size: f64) -> TriMesh {
    TriMesh {
        positions: mesh
            .positions
            .iter()
            .map(|p| {
                let mut q = base;
                for (k, axis) in axes.iter().enumerate() {
                    q = add(q, scale(*axis, p[k] * size));
                }
                q
            })
            .collect(),
        triangles: mesh.triangles.clone(),
    }
}

/// Legger `b` til `a` som egne trekanter.
pub fn append(a: &mut TriMesh, b: &TriMesh) {
    let n = a.positions.len() as u32;
    a.positions.extend_from_slice(&b.positions);
    a.triangles
        .extend(b.triangles.iter().map(|t| t.map(|i| i + n)));
}

/// Speilet om planet x = 0, med snudd omløpsretning så normalene peker ut.
pub fn mirrored(m: &TriMesh) -> TriMesh {
    TriMesh {
        positions: m.positions.iter().map(|p| [-p[0], p[1], p[2]]).collect(),
        triangles: m.triangles.iter().map(|t| [t[0], t[2], t[1]]).collect(),
    }
}

/// Alle delene i kroppen `root`, plassert i verdenskoordinater.
pub fn body_parts(
    skeleton: &Skeleton,
    world: &BTreeMap<SplineId, WorldSpline>,
    root: SplineId,
) -> TriMesh {
    let mut out = TriMesh::default();
    for id in skeleton.descendants(root) {
        let (Some(spline), Some(w)) = (skeleton.spline(id), world.get(&id)) else {
            continue;
        };
        for part in &spline.parts {
            let Some(mesh) = source_mesh(skeleton, &part.source) else {
                continue;
            };
            let (base, axes) = placement(&w.frame_at(part.at), part, spline.thickness);
            append(&mut out, &place(&mesh, base, axes, part.size));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn innebygde_deler_er_lukkede_og_vender_utover() {
        for name in BUILTIN {
            let m = builtin(name).unwrap();
            assert!(m.is_closed_manifold(), "{name}");
            assert!(m.signed_volume() > 0.0, "{name}");
            let top = m.positions.iter().map(|p| p[1]).fold(0.0, f64::max);
            assert!((top - 1.0).abs() < 1e-9, "{name}");
        }
        assert!(builtin("Vinge").is_none());
    }

    #[test]
    fn speiling_beholder_volumet() {
        let m = builtin("Horn").unwrap();
        let v = m.signed_volume();
        assert!((mirrored(&m).signed_volume() - v).abs() < 1e-9);
    }
}
