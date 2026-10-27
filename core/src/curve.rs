//! Senterlinjen til et uttrekk: en kubisk Hermite-kurve med
//! rotasjonsminimerende rammer (spesifikasjonen kap. 6 og 24).

use crate::mesh::{cross, dot, length, normalize, sub, Vec3};

pub type Mat3 = [[f64; 3]; 3];

pub const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn mat_vec(m: &Mat3, v: Vec3) -> Vec3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}

fn mat_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

/// Minste rotasjon som fører enhetsvektoren `a` over i `b` (Rodrigues).
pub fn rotation_between(a: Vec3, b: Vec3) -> Mat3 {
    let v = cross(a, b);
    let c = dot(a, b);
    if c > 1.0 - 1e-12 {
        return IDENTITY;
    }
    if c < -1.0 + 1e-9 {
        // 180°: roter om en vilkårlig akse vinkelrett på a.
        let axis = normalize(if a[0].abs() < 0.9 {
            cross(a, [1.0, 0.0, 0.0])
        } else {
            cross(a, [0.0, 1.0, 0.0])
        });
        let [x, y, z] = axis;
        return [
            [2.0 * x * x - 1.0, 2.0 * x * y, 2.0 * x * z],
            [2.0 * x * y, 2.0 * y * y - 1.0, 2.0 * y * z],
            [2.0 * x * z, 2.0 * y * z, 2.0 * z * z - 1.0],
        ];
    }
    let k = 1.0 / (1.0 + c);
    let [x, y, z] = v;
    [
        [c + x * x * k, x * y * k - z, x * z * k + y],
        [x * y * k + z, c + y * y * k, y * z * k - x],
        [x * z * k - y, y * z * k + x, c + z * z * k],
    ]
}

/// Kubisk Hermite-kurve C(s), s ∈ [0, 1], fra A til B med tangentene m0 og m1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hermite {
    pub a: Vec3,
    pub b: Vec3,
    pub m0: Vec3,
    pub m1: Vec3,
}

impl Hermite {
    /// Senterlinjen for et uttrekk fra A med overflatenormal n0 til grepet B.
    /// m0 = αL·n0 og m1 = βL·d med d = (B − A)/L. Ved innpressing snus m0, så
    /// kurven er rett når B ligger på normallinjen.
    pub fn for_pull(a: Vec3, n0: Vec3, b: Vec3, alpha: f64, beta: f64) -> Hermite {
        let ab = sub(b, a);
        let l = length(ab);
        if l < 1e-12 {
            return Hermite {
                a,
                b,
                m0: [0.0; 3],
                m1: [0.0; 3],
            };
        }
        let d = scale(ab, 1.0 / l);
        let sign = if dot(d, n0) < 0.0 { -1.0 } else { 1.0 };
        Hermite {
            a,
            b,
            m0: scale(n0, sign * alpha * l),
            m1: scale(d, beta * l),
        }
    }

    pub fn point(&self, s: f64) -> Vec3 {
        let (s2, s3) = (s * s, s * s * s);
        let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
        let h10 = s3 - 2.0 * s2 + s;
        let h01 = -2.0 * s3 + 3.0 * s2;
        let h11 = s3 - s2;
        add(
            add(scale(self.a, h00), scale(self.m0, h10)),
            add(scale(self.b, h01), scale(self.m1, h11)),
        )
    }

    pub fn derivative(&self, s: f64) -> Vec3 {
        let s2 = s * s;
        let d00 = 6.0 * s2 - 6.0 * s;
        let d10 = 3.0 * s2 - 4.0 * s + 1.0;
        let d01 = -6.0 * s2 + 6.0 * s;
        let d11 = 3.0 * s2 - 2.0 * s;
        add(
            add(scale(self.a, d00), scale(self.m0, d10)),
            add(scale(self.b, d01), scale(self.m1, d11)),
        )
    }

    pub fn tangent(&self, s: f64) -> Option<Vec3> {
        let d = self.derivative(s);
        (length(d) > 1e-12).then(|| normalize(d))
    }
}

/// Kurve med rotasjonsminimerende rammer (dobbel refleksjon, Wang mfl. 2008),
/// samplet jevnt i s. Rammen i s er rotasjonen fra starttangenten.
pub struct FramedCurve {
    pub curve: Hermite,
    rotations: Vec<Mat3>,
    tangents: Vec<Vec3>,
}

impl FramedCurve {
    pub const SAMPLES: usize = 64;

    pub fn new(curve: Hermite) -> FramedCurve {
        let n = Self::SAMPLES;
        let pts: Vec<Vec3> = (0..=n).map(|i| curve.point(i as f64 / n as f64)).collect();
        let tangents: Vec<Vec3> = (0..=n)
            .map(|i| {
                curve
                    .tangent(i as f64 / n as f64)
                    .unwrap_or([0.0, 0.0, 0.0])
            })
            .collect();
        let degenerate = tangents.iter().any(|t| length(*t) < 0.5);
        let mut rotations = vec![IDENTITY; n + 1];
        if !degenerate {
            // Referansevektor vinkelrett på starttangenten, ført langs kurven.
            let t0 = tangents[0];
            let mut r = normalize(if t0[0].abs() < 0.9 {
                cross(t0, [1.0, 0.0, 0.0])
            } else {
                cross(t0, [0.0, 1.0, 0.0])
            });
            let s0 = cross(t0, r);
            let start = [t0, r, s0];
            for i in 0..n {
                let v1 = sub(pts[i + 1], pts[i]);
                let c1 = dot(v1, v1);
                if c1 < 1e-24 {
                    continue;
                }
                let r_l = sub(r, scale(v1, 2.0 / c1 * dot(v1, r)));
                let t_l = sub(tangents[i], scale(v1, 2.0 / c1 * dot(v1, tangents[i])));
                let v2 = sub(tangents[i + 1], t_l);
                let c2 = dot(v2, v2);
                r = if c2 < 1e-24 {
                    r_l
                } else {
                    sub(r_l, scale(v2, 2.0 / c2 * dot(v2, r_l)))
                };
                r = normalize(r);
                let t = tangents[i + 1];
                let frame = [t, r, cross(t, r)];
                // Rotasjonen som fører startrammen over i denne rammen: F_i · F_0ᵀ.
                rotations[i + 1] = frame_rotation(&start, &frame);
            }
        }
        FramedCurve {
            curve,
            rotations,
            tangents,
        }
    }

    /// Rotasjonen av tverrsnittet ved parameter s.
    pub fn rotation(&self, s: f64) -> Mat3 {
        let n = Self::SAMPLES;
        let s = s.clamp(0.0, 1.0);
        let k = ((s * n as f64).floor() as usize).min(n);
        let base = &self.rotations[k];
        match self.curve.tangent(s) {
            Some(t) if length(self.tangents[k]) > 0.5 => {
                mat_mul(&rotation_between(self.tangents[k], t), base)
            }
            _ => *base,
        }
    }
}

/// Rotasjonen R med R·startᵢ = frameᵢ for de tre ortonormale aksene.
fn frame_rotation(start: &[Vec3; 3], frame: &[Vec3; 3]) -> Mat3 {
    let mut m = [[0.0; 3]; 3];
    for k in 0..3 {
        for (i, row) in m.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell += frame[k][i] * start[k][j];
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3, eps: f64) -> bool {
        length(sub(a, b)) < eps
    }

    #[test]
    fn rett_kurve_naar_grepet_ligger_paa_normalen() {
        let c = Hermite::for_pull([0.0; 3], [0.0, 0.0, 1.0], [0.0, 0.0, 10.0], 1.0, 1.0);
        for i in 0..=10 {
            let s = i as f64 / 10.0;
            assert!(close(c.point(s), [0.0, 0.0, 10.0 * s], 1e-12));
        }
        // Innpressing gir også en rett linje, ikke en sløyfe.
        let c = Hermite::for_pull([0.0; 3], [0.0, 0.0, 1.0], [0.0, 0.0, -4.0], 1.0, 1.0);
        assert!(close(c.point(0.5), [0.0, 0.0, -2.0], 1e-12));
    }

    #[test]
    fn kurven_starter_langs_normalen_og_ender_i_grepets_retning() {
        let c = Hermite::for_pull([0.0; 3], [0.0, 0.0, 1.0], [10.0, 0.0, 10.0], 1.0, 1.0);
        assert!(close(c.point(1.0), [10.0, 0.0, 10.0], 1e-12));
        assert!(close(c.tangent(0.0).unwrap(), [0.0, 0.0, 1.0], 1e-12));
        let d = normalize([1.0, 0.0, 1.0]);
        assert!(close(c.tangent(1.0).unwrap(), d, 1e-12));
    }

    #[test]
    fn rammene_foelger_tangenten_uten_vridning_i_planet() {
        let c = Hermite::for_pull([0.0; 3], [0.0, 0.0, 1.0], [12.0, 0.0, 8.0], 1.0, 1.0);
        let f = FramedCurve::new(c);
        for i in 0..=20 {
            let s = i as f64 / 20.0;
            let r = f.rotation(s);
            // Rotasjonen fører starttangenten over i tangenten i s.
            assert!(
                close(mat_vec(&r, [0.0, 0.0, 1.0]), c.tangent(s).unwrap(), 2e-3),
                "s = {s}"
            );
            // Plan kurve i XZ: normalen til planet (y) holdes fast, altså ingen vridning.
            assert!(
                close(mat_vec(&r, [0.0, 1.0, 0.0]), [0.0, 1.0, 0.0], 1e-6),
                "s = {s}"
            );
        }
    }

    #[test]
    fn rotasjon_mellom_vektorer() {
        let r = rotation_between([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
        assert!(close(mat_vec(&r, [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0], 1e-12));
        let r = rotation_between([0.0, 0.0, 1.0], [0.0, 0.0, -1.0]);
        assert!(close(mat_vec(&r, [0.0, 0.0, 1.0]), [0.0, 0.0, -1.0], 1e-12));
    }
}
