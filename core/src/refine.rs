//! Lokal fortetting av meshen ved kantdeling (spesifikasjonen kap. 7).
//!
//! Kanter som er markert for deling får et nytt midtpunkt. Hver trekant
//! deles etter hvor mange av kantene dens som deles (0–3), slik at meshen
//! forblir lukket og konsistent orientert.

use crate::mesh::{TriMesh, Vec3};
use std::collections::{HashMap, HashSet};

pub type Edge = (u32, u32);

pub fn edge_key(a: u32, b: u32) -> Edge {
    (a.min(b), a.max(b))
}

/// Deler alle kanter i `edges`. Nye punkter plasseres med `midpoint`.
/// Returnerer antall nye punkter.
pub fn split_edges(
    mesh: &mut TriMesh,
    edges: &HashSet<Edge>,
    mut midpoint: impl FnMut(Vec3, Vec3) -> Vec3,
) -> usize {
    if edges.is_empty() {
        return 0;
    }
    // Sortert rekkefølge gjør resultatet deterministisk, slik at angre og
    // gjentatt beregning gir nøyaktig samme mesh.
    let mut sorted: Vec<Edge> = edges.iter().copied().collect();
    sorted.sort_unstable();
    let mut mids: HashMap<Edge, u32> = HashMap::with_capacity(edges.len());
    for (a, b) in sorted {
        let p = midpoint(mesh.positions[a as usize], mesh.positions[b as usize]);
        mesh.positions.push(p);
        mids.insert((a, b), (mesh.positions.len() - 1) as u32);
    }

    let mut out = Vec::with_capacity(mesh.triangles.len() + edges.len() * 2);
    for &[a, b, c] in &mesh.triangles {
        let m = |x: u32, y: u32| mids.get(&edge_key(x, y)).copied();
        let (ab, bc, ca) = (m(a, b), m(b, c), m(c, a));
        match (ab, bc, ca) {
            (None, None, None) => out.push([a, b, c]),
            (Some(ab), None, None) => out.extend([[a, ab, c], [ab, b, c]]),
            (None, Some(bc), None) => out.extend([[b, bc, a], [bc, c, a]]),
            (None, None, Some(ca)) => out.extend([[c, ca, b], [ca, a, b]]),
            (Some(ab), Some(bc), None) => out.extend([[ab, b, bc], [a, ab, bc], [a, bc, c]]),
            (None, Some(bc), Some(ca)) => out.extend([[bc, c, ca], [b, bc, ca], [b, ca, a]]),
            (Some(ab), None, Some(ca)) => out.extend([[ca, a, ab], [c, ca, ab], [c, ab, b]]),
            (Some(ab), Some(bc), Some(ca)) => {
                out.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]])
            }
        }
    }
    mesh.triangles = out;
    edges.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sphere::icosphere;

    fn mid(a: Vec3, b: Vec3) -> Vec3 {
        [
            (a[0] + b[0]) / 2.0,
            (a[1] + b[1]) / 2.0,
            (a[2] + b[2]) / 2.0,
        ]
    }

    #[test]
    fn deling_av_vilkaarlige_kanter_bevarer_lukket_manifold() {
        let mut m = icosphere(10.0, 2);
        let volume = m.signed_volume();
        // Del hver tredje kant, slik at alle tilfellene 0–3 delte kanter oppstår.
        let mut edges = HashSet::new();
        for (i, t) in m.triangles.iter().enumerate() {
            for k in 0..3 {
                if (i + k) % 3 == 0 {
                    edges.insert(edge_key(t[k], t[(k + 1) % 3]));
                }
            }
        }
        let added = split_edges(&mut m, &edges, mid);
        assert_eq!(added, edges.len());
        assert!(m.is_closed_manifold());
        // Midtpunkter på kordene endrer ikke volumet.
        assert!((m.signed_volume() - volume).abs() < 1e-9);
    }
}
