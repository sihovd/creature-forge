//! Grunnformen kule, laget ved oppdeling av et ikosaeder.

use crate::mesh::{normalize, TriMesh};
use std::collections::HashMap;

/// Høyeste tillatte oppdelingsnivå (20 · 4^7 ≈ 330 000 trekanter).
pub const MAX_SUBDIVISIONS: u32 = 7;

pub fn icosphere(radius_mm: f64, subdivisions: u32) -> TriMesh {
    let t = (1.0 + 5f64.sqrt()) / 2.0;
    let mut unit: Vec<[f64; 3]> = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .into_iter()
    .map(normalize)
    .collect();

    #[rustfmt::skip]
    let mut triangles: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];

    for _ in 0..subdivisions {
        let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
        let mut mid = |a: u32, b: u32, pts: &mut Vec<[f64; 3]>| -> u32 {
            *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                let (pa, pb) = (pts[a as usize], pts[b as usize]);
                pts.push(normalize([
                    (pa[0] + pb[0]) / 2.0,
                    (pa[1] + pb[1]) / 2.0,
                    (pa[2] + pb[2]) / 2.0,
                ]));
                (pts.len() - 1) as u32
            })
        };
        let mut next = Vec::with_capacity(triangles.len() * 4);
        for [a, b, c] in triangles {
            let ab = mid(a, b, &mut unit);
            let bc = mid(b, c, &mut unit);
            let ca = mid(c, a, &mut unit);
            next.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        triangles = next;
    }

    TriMesh {
        positions: unit.into_iter().map(|p| p.map(|v| v * radius_mm)).collect(),
        triangles,
    }
}

/// Velger oppdelingsnivå slik at gjennomsnittlig kantlengde blir høyst
/// `target_edge_mm`, begrenset av [`MAX_SUBDIVISIONS`].
pub fn icosphere_for_edge(radius_mm: f64, target_edge_mm: f64) -> Result<TriMesh, String> {
    if !(radius_mm.is_finite() && radius_mm > 0.0) {
        return Err(format!("Ugyldig radius: {radius_mm} mm"));
    }
    if !(target_edge_mm.is_finite() && target_edge_mm > 0.0) {
        return Err(format!("Ugyldig kantlengde: {target_edge_mm} mm"));
    }
    // Ikosaederets kant på enhetskulen er ca. 1,05; hver oppdeling halverer den.
    let base_edge = 1.0515 * radius_mm;
    let mut level = 0;
    while level < MAX_SUBDIVISIONS && base_edge / 2f64.powi(level as i32) > target_edge_mm {
        level += 1;
    }
    Ok(icosphere(radius_mm, level))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::length;
    use std::f64::consts::PI;

    #[test]
    fn alle_punkter_ligger_paa_radius() {
        let m = icosphere(25.0, 3);
        for p in &m.positions {
            assert!((length(*p) - 25.0).abs() < 1e-9);
        }
    }

    #[test]
    fn kulen_er_lukket_og_har_euler_karakteristikk_2() {
        for level in 0..=4 {
            let m = icosphere(10.0, level);
            assert!(m.is_closed_manifold(), "nivå {level}");
            let chi = m.positions.len() as i64 - m.edge_count() as i64 + m.triangles.len() as i64;
            assert_eq!(chi, 2, "nivå {level}");
            assert_eq!(m.triangles.len(), 20 * 4usize.pow(level));
        }
    }

    #[test]
    fn trekantene_vender_utover_og_volumet_naermer_seg_kula() {
        let r = 20.0;
        let m = icosphere(r, 5);
        let exact = 4.0 / 3.0 * PI * r * r * r;
        let v = m.signed_volume();
        assert!(v > 0.0);
        assert!((v - exact).abs() / exact < 0.005, "volum {v} mot {exact}");
    }

    #[test]
    fn kantlengde_styrer_oppdeling() {
        let grov = icosphere_for_edge(25.0, 10.0).unwrap();
        let fin = icosphere_for_edge(25.0, 1.0).unwrap();
        assert!(fin.triangles.len() > grov.triangles.len());
        assert!(icosphere_for_edge(25.0, 0.0).is_err());
        assert!(icosphere_for_edge(f64::NAN, 1.0).is_err());
    }
}
