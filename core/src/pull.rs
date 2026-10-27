//! Uttrekk og innpressing fra et grep på overflaten (spesifikasjonen kap. 4).
//!
//! Forskyvningen er en funksjon av punktets posisjon før operasjonen. Når en
//! kant blir for lang etter forskyvningen, deles den i den opprinnelige
//! meshen og funksjonen evalueres på nytt. Slik får nye punkter nøyaktig samme
//! glatte form som resten, i stedet for å ligge på rette korder.

use crate::mesh::{dot, length, sub, TriMesh, Vec3};
use crate::refine::{edge_key, split_edges};
use std::collections::HashSet;

/// Grense for antall trekanter i én kroppsdel før fortetting stoppes.
pub const MAX_TRIANGLES: usize = 600_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pull {
    /// Grepets startpunkt A på overflaten, i mm.
    pub anchor: Vec3,
    /// Uttrekksretning (enhetsvektor), låst fra starten av draget.
    pub direction: Vec3,
    /// Uttrekkslengde i mm. Negativ verdi presser overflaten inn.
    pub distance: f64,
    /// Påvirkningsradius i mm.
    pub radius: f64,
}

impl Pull {
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

    pub fn displace(&self, p: Vec3) -> Vec3 {
        let w = self.weight(p) * self.distance;
        [
            p[0] + w * self.direction[0],
            p[1] + w * self.direction[1],
            p[2] + w * self.direction[2],
        ]
    }

    pub fn validate(&self) -> Result<(), String> {
        let finite = |v: Vec3| v.iter().all(|x| x.is_finite());
        if !finite(self.anchor) || !finite(self.direction) || !self.distance.is_finite() {
            return Err("Grepet har ugyldige tall".into());
        }
        if !(self.radius.is_finite() && self.radius > 0.0) {
            return Err(format!("Ugyldig påvirkningsradius: {} mm", self.radius));
        }
        if (length(self.direction) - 1.0).abs() > 1e-6 {
            return Err("Uttrekksretningen må være en enhetsvektor".into());
        }
        Ok(())
    }
}

/// Utfører uttrekket på `mesh` med fortetting til ønsket kantlengde `h` (mm).
pub fn apply_pull(mesh: &TriMesh, pull: &Pull, h: f64) -> Result<TriMesh, String> {
    pull.validate()?;
    let split_above = 4.0 / 3.0 * h;
    let mut base = mesh.clone();

    for _ in 0..12 {
        let moved: Vec<Vec3> = base.positions.iter().map(|&p| pull.displace(p)).collect();
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
        *p = pull.displace(*p);
    }
    Ok(base)
}

/// Høyden på grepet etter uttrekket, målt langs retningen fra A.
pub fn tip_offset(mesh: &TriMesh, pull: &Pull) -> f64 {
    mesh.positions
        .iter()
        .map(|&p| dot(sub(p, pull.anchor), pull.direction))
        .fold(f64::MIN, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sphere::icosphere;

    /// Grep i punkt 0 på kula, rett ut langs normalen.
    fn pull_up(distance: f64) -> Pull {
        let anchor = icosphere(25.0, 0).positions[0];
        let direction = crate::mesh::normalize(anchor);
        Pull {
            anchor,
            direction,
            distance,
            radius: 12.0,
        }
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
        p.direction = [0.0, 0.0, 2.0];
        assert!(apply_pull(&sphere, &p, 1.5).is_err());
    }
}
