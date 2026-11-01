//! Grunnformene brukeren kan starte med: kule, kapsel, kjegle og boks.
//!
//! Kula bygges direkte som ikosfære. De andre formene beskrives med en
//! avstandsfunksjon (SDF) med avrundede kanter. En grov ikosfære projiseres
//! ut på overflaten langs stråler fra sentrum, og kanter lengre enn 4h/3 deles
//! til hele overflaten har omtrent ønsket kantlengde. Alle formene er
//! konvekse og inneholder sentrum, så hver stråle treffer overflaten én gang.

use crate::mesh::{length, normalize, sub, TriMesh, Vec3};
use crate::refine::{edge_key, split_edges};
use crate::sphere::{icosphere, icosphere_for_edge};
use std::collections::HashSet;

/// Største antall trekanter en ny grunnform kan få.
const MAX_TRIANGLES: usize = 400_000;

/// Avrundingsradius i mm for kantene på boks og kjegle.
const EDGE_ROUNDING_MM: f64 = 2.0;

/// En grunnform sentrert i origo med y opp. Alle mål er i mm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Primitive {
    Sphere {
        radius: f64,
    },
    /// `height` er total høyde inkludert de to halvkulene.
    Capsule {
        radius: f64,
        height: f64,
    },
    /// Spissen peker opp langs y.
    Cone {
        radius: f64,
        height: f64,
    },
    Box {
        size: Vec3,
    },
}

impl Primitive {
    /// Leser en grunnform fra navn og parametere slik grensesnittet sender dem.
    pub fn from_parts(kind: &str, params: &[f64]) -> Result<Primitive, String> {
        let p = |i: usize| params.get(i).copied().unwrap_or(f64::NAN);
        let prim = match kind {
            "sphere" => Primitive::Sphere { radius: p(0) },
            "capsule" => Primitive::Capsule {
                radius: p(0),
                height: p(1),
            },
            "cone" => Primitive::Cone {
                radius: p(0),
                height: p(1),
            },
            "box" => Primitive::Box {
                size: [p(0), p(1), p(2)],
            },
            _ => return Err(format!("Ukjent grunnform: {kind}")),
        };
        prim.validate()?;
        Ok(prim)
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Primitive::Sphere { .. } => "sphere",
            Primitive::Capsule { .. } => "capsule",
            Primitive::Cone { .. } => "cone",
            Primitive::Box { .. } => "box",
        }
    }

    fn validate(&self) -> Result<(), String> {
        let ok = |v: f64| v.is_finite() && v > 0.0 && v <= 2000.0;
        let valid = match *self {
            Primitive::Sphere { radius } => ok(radius),
            // En kapsel må være minst like høy som diameteren.
            Primitive::Capsule { radius, height } => {
                ok(radius) && ok(height) && height >= 2.0 * radius
            }
            Primitive::Cone { radius, height } => ok(radius) && ok(height),
            Primitive::Box { size } => size.iter().all(|&v| ok(v)),
        };
        if valid {
            Ok(())
        } else {
            Err(format!("Ugyldige mål for grunnformen: {self:?}"))
        }
    }

    /// Avstand med fortegn til overflaten: negativ innenfor.
    pub fn sdf(&self, p: Vec3) -> f64 {
        match *self {
            Primitive::Sphere { radius } => length(p) - radius,
            Primitive::Capsule { radius, height } => {
                let half = (height / 2.0 - radius).max(0.0);
                let y = p[1].clamp(-half, half);
                length([p[0], p[1] - y, p[2]]) - radius
            }
            Primitive::Cone { radius, height } => {
                // Omdreiningsflate: en avrundet trekant i planet (r, y).
                let r = (p[0] * p[0] + p[2] * p[2]).sqrt();
                let tri = [
                    [-radius, -height / 2.0],
                    [radius, -height / 2.0],
                    [0.0, height / 2.0],
                ];
                let rounding = EDGE_ROUNDING_MM.min(0.3 * inradius(&tri));
                triangle_sdf([r, p[1]], &inset(&tri, rounding)) - rounding
            }
            Primitive::Box { size } => {
                let rounding =
                    EDGE_ROUNDING_MM.min(0.25 * size.iter().copied().fold(f64::MAX, f64::min));
                let q = [0, 1, 2].map(|i| p[i].abs() - (size[i] / 2.0 - rounding));
                let outside = length(q.map(|v| v.max(0.0)));
                outside + q[0].max(q[1]).max(q[2]).min(0.0) - rounding
            }
        }
    }

    /// Radius til en kule om origo som inneholder hele formen.
    fn bounding_radius(&self) -> f64 {
        match *self {
            Primitive::Sphere { radius } => radius,
            Primitive::Capsule { height, .. } => height / 2.0,
            Primitive::Cone { radius, height } => (radius * radius + height * height / 4.0).sqrt(),
            Primitive::Box { size } => length(size) / 2.0,
        }
    }

    /// Punktet der strålen fra origo i retning `dir` treffer overflaten.
    fn project(&self, dir: Vec3) -> Vec3 {
        let d = normalize(dir);
        let (mut lo, mut hi) = (0.0, self.bounding_radius() * 1.01 + 1e-6);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if self.sdf(d.map(|v| v * mid)) < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let t = 0.5 * (lo + hi);
        d.map(|v| v * t)
    }

    /// Lukket trekantmesh med omtrent kantlengden `h` mm, sentrert i origo.
    pub fn mesh(&self, h: f64) -> Result<TriMesh, String> {
        self.validate()?;
        if !(h.is_finite() && h > 0.0) {
            return Err(format!("Ugyldig kantlengde: {h} mm"));
        }
        if let Primitive::Sphere { radius } = *self {
            return icosphere_for_edge(radius, h);
        }
        let mut mesh = icosphere(1.0, 2);
        for p in mesh.positions.iter_mut() {
            *p = self.project(*p);
        }
        let split_above = 4.0 / 3.0 * h;
        loop {
            let mut long = HashSet::new();
            for t in &mesh.triangles {
                for k in 0..3 {
                    let (a, b) = (t[k], t[(k + 1) % 3]);
                    let (pa, pb) = (mesh.positions[a as usize], mesh.positions[b as usize]);
                    if length(sub(pa, pb)) > split_above {
                        long.insert(edge_key(a, b));
                    }
                }
            }
            if long.is_empty() {
                break;
            }
            if mesh.triangles.len() + long.len() * 2 > MAX_TRIANGLES {
                return Err("Grunnformen blir for stor for valgt kantlengde".into());
            }
            split_edges(&mut mesh, &long, |a, b| {
                self.project([a[0] + b[0], a[1] + b[1], a[2] + b[2]])
            });
        }
        Ok(mesh)
    }

    /// Halv høyde langs y, brukt til å plassere formen på rutenettet.
    pub fn half_height(&self) -> f64 {
        match *self {
            Primitive::Sphere { radius } => radius,
            Primitive::Capsule { height, .. } | Primitive::Cone { height, .. } => height / 2.0,
            Primitive::Box { size } => size[1] / 2.0,
        }
    }
}

type Vec2 = [f64; 2];

fn sub2(a: Vec2, b: Vec2) -> Vec2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn len2(a: Vec2) -> f64 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}

/// Avstand med fortegn til en trekant med punktene mot klokka.
fn triangle_sdf(p: Vec2, tri: &[Vec2; 3]) -> f64 {
    let mut nearest = f64::MAX;
    let mut inside = true;
    for i in 0..3 {
        let (a, b) = (tri[i], tri[(i + 1) % 3]);
        let e = sub2(b, a);
        let w = sub2(p, a);
        let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1])).clamp(0.0, 1.0);
        nearest = nearest.min(len2([w[0] - e[0] * t, w[1] - e[1] * t]));
        // Punktet ligger til høyre for en kant (mot klokka) er utenfor.
        if e[0] * w[1] - e[1] * w[0] < 0.0 {
            inside = false;
        }
    }
    if inside {
        -nearest
    } else {
        nearest
    }
}

fn incircle(tri: &[Vec2; 3]) -> (Vec2, f64) {
    let side = |i: usize| len2(sub2(tri[(i + 1) % 3], tri[(i + 2) % 3]));
    let (a, b, c) = (side(0), side(1), side(2));
    let p = a + b + c;
    let center = [
        (a * tri[0][0] + b * tri[1][0] + c * tri[2][0]) / p,
        (a * tri[0][1] + b * tri[1][1] + c * tri[2][1]) / p,
    ];
    let e1 = sub2(tri[1], tri[0]);
    let e2 = sub2(tri[2], tri[0]);
    let area = 0.5 * (e1[0] * e2[1] - e1[1] * e2[0]).abs();
    (center, 2.0 * area / p)
}

fn inradius(tri: &[Vec2; 3]) -> f64 {
    incircle(tri).1
}

/// Trekanten flyttet innover med `d`: skalering om innsenteret.
fn inset(tri: &[Vec2; 3], d: f64) -> [Vec2; 3] {
    let (c, r) = incircle(tri);
    let s = (r - d) / r;
    tri.map(|p| [c[0] + (p[0] - c[0]) * s, c[1] + (p[1] - c[1]) * s])
}

/// Flytter alle punkter med `offset`.
pub fn translate(mesh: &mut TriMesh, offset: Vec3) {
    for p in mesh.positions.iter_mut() {
        *p = [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]];
    }
}

/// Største avvik fra overflaten blant punktene, brukt i tester.
pub fn max_surface_error(prim: &Primitive, mesh: &TriMesh) -> f64 {
    mesh.positions
        .iter()
        .map(|&p| prim.sdf(p).abs())
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> [Primitive; 4] {
        [
            Primitive::Sphere { radius: 25.0 },
            Primitive::Capsule {
                radius: 15.0,
                height: 60.0,
            },
            Primitive::Cone {
                radius: 25.0,
                height: 50.0,
            },
            Primitive::Box {
                size: [45.0, 45.0, 45.0],
            },
        ]
    }

    #[test]
    fn alle_grunnformer_er_lukkede_og_vender_utover() {
        for prim in all() {
            let m = prim.mesh(1.5).unwrap();
            assert!(m.is_closed_manifold(), "{prim:?}");
            let chi = m.positions.len() as i64 - m.edge_count() as i64 + m.triangles.len() as i64;
            assert_eq!(chi, 2, "{prim:?}");
            assert!(m.signed_volume() > 0.0, "{prim:?}");
        }
    }

    #[test]
    fn punktene_ligger_paa_overflaten_og_kantene_er_korte() {
        for prim in all() {
            let m = prim.mesh(1.5).unwrap();
            assert!(max_surface_error(&prim, &m) < 1e-6, "{prim:?}");
            for t in &m.triangles {
                for k in 0..3 {
                    let (a, b) = (
                        m.positions[t[k] as usize],
                        m.positions[t[(k + 1) % 3] as usize],
                    );
                    assert!(length(sub(a, b)) <= 2.0 + 1e-9, "{prim:?}");
                }
            }
        }
    }

    #[test]
    fn volumet_stemmer_med_maalene() {
        let pi = std::f64::consts::PI;
        let check = |prim: Primitive, exact: f64| {
            let v = prim.mesh(1.0).unwrap().signed_volume();
            // Avrundede kanter gjør boks og kjegle litt mindre enn de skarpe målene.
            assert!(
                (v - exact).abs() / exact < 0.04,
                "{prim:?}: {v} mot {exact}"
            );
        };
        check(
            Primitive::Capsule {
                radius: 10.0,
                height: 40.0,
            },
            pi * 100.0 * 20.0 + 4.0 / 3.0 * pi * 1000.0,
        );
        check(
            Primitive::Cone {
                radius: 20.0,
                height: 40.0,
            },
            pi * 400.0 * 40.0 / 3.0,
        );
        check(
            Primitive::Box {
                size: [30.0, 20.0, 40.0],
            },
            24_000.0,
        );
    }

    #[test]
    fn ugyldige_maal_avvises() {
        assert!(Primitive::from_parts("capsule", &[20.0, 30.0]).is_err());
        assert!(Primitive::from_parts("box", &[10.0, 10.0]).is_err());
        assert!(Primitive::from_parts("cone", &[10.0, -1.0]).is_err());
        assert!(Primitive::from_parts("torus", &[10.0]).is_err());
        assert!(Primitive::from_parts("box", &[10.0, 20.0, 30.0]).is_ok());
    }
}
