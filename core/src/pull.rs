//! Uttrekk, innpressing og bøying fra et grep på overflaten
//! (spesifikasjonen kap. 4–6).
//!
//! Et punkt p med vekt w flyttes til C(w) + R(w)·(p − A), der C er
//! senterlinjen fra A til grepet B og R(w) er tverrsnittets rotasjon langs
//! den. Når B ligger på normallinjen, er dette en ren forskyvning langs
//! normalen. Forskyvningen er en funksjon av punktets posisjon før
//! operasjonen. Når en kant blir for lang, deles den i den opprinnelige
//! meshen og funksjonen evalueres på nytt, slik at nye punkter følger den
//! glatte formen i stedet for rette korder.

use crate::curve::{mat_vec, FramedCurve, Hermite};
use crate::mesh::{dot, length, sub, TriMesh, Vec3};
use crate::refine::{edge_key, split_edges};
use std::collections::HashSet;

/// Grense for antall trekanter i én kroppsdel før fortetting stoppes.
pub const MAX_TRIANGLES: usize = 600_000;

/// Formparametere for senterlinjens tangenter (kap. 24); α = β = 1 som første testpunkt.
pub const ALPHA: f64 = 1.0;
pub const BETA: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pull {
    /// Grepets startpunkt A på overflaten, i mm.
    pub anchor: Vec3,
    /// Overflatenormalen n0 i A (enhetsvektor), låst fra starten av draget.
    pub normal: Vec3,
    /// Grepets endepunkt B i mm. Senterlinjen går fra A til B.
    pub tip: Vec3,
    /// Påvirkningsradius i mm.
    pub radius: f64,
}

impl Pull {
    /// Rett uttrekk langs normalen. Negativ lengde presser inn.
    pub fn straight(anchor: Vec3, normal: Vec3, distance: f64, radius: f64) -> Pull {
        let tip = [
            anchor[0] + normal[0] * distance,
            anchor[1] + normal[1] * distance,
            anchor[2] + normal[2] * distance,
        ];
        Pull {
            anchor,
            normal,
            tip,
            radius,
        }
    }

    /// Avstanden |B − A| i mm (trådlengden i Alt + Shift).
    pub fn length(&self) -> f64 {
        length(sub(self.tip, self.anchor))
    }

    pub fn curve(&self) -> Hermite {
        Hermite::for_pull(self.anchor, self.normal, self.tip, ALPHA, BETA)
    }

    /// Vekt i [0, 1] for et punkt, glatt (C²) ned til null ved radius.
    pub fn weight(&self, p: Vec3) -> f64 {
        let t = length(sub(p, self.anchor)) / self.radius;
        if t >= 1.0 {
            0.0
        } else {
            let s = 1.0 - t * t;
            s * s * s
        }
    }

    pub fn displace(&self, framed: &FramedCurve, p: Vec3) -> Vec3 {
        let w = self.weight(p);
        if w == 0.0 {
            return p;
        }
        let c = framed.curve.point(w);
        let local = mat_vec(&framed.rotation(w), sub(p, self.anchor));
        [c[0] + local[0], c[1] + local[1], c[2] + local[2]]
    }

    pub fn validate(&self) -> Result<(), String> {
        let finite = |v: Vec3| v.iter().all(|x| x.is_finite());
        if !finite(self.anchor) || !finite(self.normal) || !finite(self.tip) {
            return Err("Grepet har ugyldige tall".into());
        }
        if !(self.radius.is_finite() && self.radius > 0.0) {
            return Err(format!("Ugyldig påvirkningsradius: {} mm", self.radius));
        }
        if (length(self.normal) - 1.0).abs() > 1e-6 {
            return Err("Normalen må være en enhetsvektor".into());
        }
        Ok(())
    }
}

/// Utfører uttrekket på `mesh` med fortetting til ønsket kantlengde `h` (mm).
pub fn apply_pull(mesh: &TriMesh, pull: &Pull, h: f64) -> Result<TriMesh, String> {
    pull.validate()?;
    let framed = FramedCurve::new(pull.curve());
    let split_above = 4.0 / 3.0 * h;
    let mut base = mesh.clone();

    for _ in 0..12 {
        let moved: Vec<Vec3> = base
            .positions
            .iter()
            .map(|&p| pull.displace(&framed, p))
            .collect();
        let mut long = HashSet::new();
        for t in &base.triangles {
            // Bare trekanter som faktisk påvirkes, kan få nye lange kanter.
            if t.iter()
                .all(|&i| pull.weight(base.positions[i as usize]) == 0.0)
            {
                continue;
            }
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                if length(sub(moved[a as usize], moved[b as usize])) > split_above
                    && length(sub(base.positions[a as usize], base.positions[b as usize]))
                        > h * 0.05
                {
                    long.insert(edge_key(a, b));
                }
            }
        }
        if long.is_empty() || base.triangles.len() + long.len() * 2 > MAX_TRIANGLES {
            break;
        }
        split_edges(&mut base, &long, |a, b| {
            [
                (a[0] + b[0]) / 2.0,
                (a[1] + b[1]) / 2.0,
                (a[2] + b[2]) / 2.0,
            ]
        });
    }

    for p in base.positions.iter_mut() {
        *p = pull.displace(&framed, *p);
    }
    Ok(base)
}

/// Høyden på grepet etter uttrekket, målt langs retningen fra A.
pub fn tip_offset(mesh: &TriMesh, pull: &Pull) -> f64 {
    mesh.positions
        .iter()
        .map(|&p| dot(sub(p, pull.anchor), pull.normal))
        .fold(f64::MIN, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sphere::icosphere;

    /// Grep i punkt 0 på kula, rett ut langs normalen.
    fn pull_up(distance: f64) -> Pull {
        let anchor = icosphere(25.0, 0).positions[0];
        Pull::straight(anchor, crate::mesh::normalize(anchor), distance, 12.0)
    }

    fn max_edge_near(m: &TriMesh, pull: &Pull) -> f64 {
        let mut max: f64 = 0.0;
        for t in &m.triangles {
            for k in 0..3 {
                let (a, b) = (
                    m.positions[t[k] as usize],
                    m.positions[t[(k + 1) % 3] as usize],
                );
                if length(sub(a, pull.anchor)) < pull.radius * 3.0 {
                    max = max.max(length(sub(a, b)));
                }
            }
        }
        max
    }

    #[test]
    fn uttrekk_gir_lukket_mesh_med_riktig_hoeyde() {
        let sphere = icosphere(25.0, 4);
        let pull = pull_up(20.0);
        let out = apply_pull(&sphere, &pull, 1.5).unwrap();
        assert!(out.is_closed_manifold());
        assert!(out.signed_volume() > sphere.signed_volume());
        // Toppen flyttes nøyaktig uttrekkslengden fordi A er et punkt i meshen.
        assert!((tip_offset(&out, &pull) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn uttrekk_fortetter_saa_kantene_ikke_strekkes() {
        let sphere = icosphere(25.0, 4);
        let pull = pull_up(30.0);
        let out = apply_pull(&sphere, &pull, 1.5).unwrap();
        assert!(out.triangles.len() > sphere.triangles.len());
        // Ingen kant nær uttrekket skal være mye lengre enn delingsgrensen.
        assert!(
            max_edge_near(&out, &pull) < 4.0 / 3.0 * 1.5 * 1.05,
            "{}",
            max_edge_near(&out, &pull)
        );
    }

    #[test]
    fn punkter_utenfor_radius_flyttes_ikke() {
        let sphere = icosphere(25.0, 3);
        let pull = pull_up(10.0);
        let out = apply_pull(&sphere, &pull, 1.5).unwrap();
        for (i, p) in sphere.positions.iter().enumerate() {
            if length(sub(*p, pull.anchor)) >= pull.radius {
                assert_eq!(out.positions[i], *p);
            }
        }
    }

    #[test]
    fn innpressing_minsker_volumet() {
        let sphere = icosphere(25.0, 4);
        let out = apply_pull(&sphere, &pull_up(-5.0), 1.5).unwrap();
        assert!(out.is_closed_manifold());
        assert!(out.signed_volume() < sphere.signed_volume());
    }

    #[test]
    fn ugyldig_grep_avvises() {
        let sphere = icosphere(25.0, 1);
        let mut p = pull_up(5.0);
        p.radius = 0.0;
        assert!(apply_pull(&sphere, &p, 1.5).is_err());
        let mut p = pull_up(5.0);
        p.normal = [0.0, 0.0, 2.0];
        assert!(apply_pull(&sphere, &p, 1.5).is_err());
    }

    #[test]
    fn boyd_uttrekk_ender_i_grepet_og_er_lukket() {
        let sphere = icosphere(25.0, 4);
        let straight = pull_up(20.0);
        // Grepet flyttet sidelengs: senterlinjen bøyer seg.
        let n = straight.normal;
        let side = crate::mesh::normalize(crate::mesh::cross(n, [0.0, 1.0, 0.0]));
        let tip = [
            straight.tip[0] + side[0] * 12.0,
            straight.tip[1] + side[1] * 12.0,
            straight.tip[2] + side[2] * 12.0,
        ];
        let bent = Pull { tip, ..straight };
        let out = apply_pull(&sphere, &bent, 1.5).unwrap();
        assert!(out.is_closed_manifold());
        assert!(out.signed_volume() > sphere.signed_volume());
        // Punktet i A (vekt 1) havner nøyaktig i B.
        let a = sphere
            .positions
            .iter()
            .position(|p| *p == straight.anchor)
            .unwrap();
        assert!(length(sub(out.positions[a], tip)) < 1e-9);
    }

    #[test]
    fn rett_grep_gir_samme_resultat_som_ren_forskyvning() {
        let sphere = icosphere(25.0, 3);
        let pull = pull_up(15.0);
        let out = apply_pull(&sphere, &pull, 1.5).unwrap();
        for (i, p) in sphere.positions.iter().enumerate() {
            let w = pull.weight(*p) * 15.0;
            let expected = [
                p[0] + w * pull.normal[0],
                p[1] + w * pull.normal[1],
                p[2] + w * pull.normal[2],
            ];
            assert!(length(sub(out.positions[i], expected)) < 1e-9);
        }
    }
}
