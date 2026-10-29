//! Tverrsnittsprofiler langs en spline (issue #4, del 3 og 4).
//!
//! En profil er et lukket 2D-omriss av kubiske Bézier-segmenter i
//! tverrsnittsplanet (x langs U, y langs V i splinens ramme). Profilen er
//! målt i enheter av splinens tykkelse, så en sirkel har radius 1. Omrisset
//! kan være konkavt, for eksempel en H.
//!
//! En spline har null eller flere profilstasjoner langs buelengden. Hver
//! stasjon har sitt eget omriss eller er koblet til en master, og har egen
//! størrelse og rotasjon. Mellom to stasjoner interpoleres omrisset med
//! samplet korrespondanse, der startpunktet i den ene velges slik at
//! overgangen vrir seg minst mulig. Uten stasjoner er tverrsnittet en
//! sirkel.

use crate::skeleton::AnchorKind;
use serde::{Deserialize, Serialize};

pub type Vec2 = [f64; 2];
pub type MasterId = u32;

/// Punkter per kurvede segment når omrisset gjøres om til et polygon.
const POINTS_PER_CURVE: usize = 16;
/// Punkter i den jevne samplingen som brukes for overganger.
pub const RESAMPLE_POINTS: usize = 128;
/// Håndtakslengde for en sirkel av fire kubiske Bézier-segmenter.
const CIRCLE_K: f64 = 0.552_284_749_830_793_4;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfilePoint {
    pub position: Vec2,
    /// Håndtakene som forskyvninger fra punktet.
    pub handle_in: Vec2,
    pub handle_out: Vec2,
    pub kind: AnchorKind,
}

impl ProfilePoint {
    pub fn corner(position: Vec2) -> ProfilePoint {
        ProfilePoint {
            position,
            handle_in: [0.0; 2],
            handle_out: [0.0; 2],
            kind: AnchorKind::Corner,
        }
    }
}

/// Et lukket omriss. Siste punkt kobles til det første.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub points: Vec<ProfilePoint>,
}

fn add2(a: Vec2, b: Vec2) -> Vec2 {
    [a[0] + b[0], a[1] + b[1]]
}

fn sub2(a: Vec2, b: Vec2) -> Vec2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn scale2(a: Vec2, s: f64) -> Vec2 {
    [a[0] * s, a[1] * s]
}

fn dot2(a: Vec2, b: Vec2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn len2(a: Vec2) -> f64 {
    dot2(a, a).sqrt()
}

fn bezier2(p: [Vec2; 4], t: f64) -> Vec2 {
    let s = 1.0 - t;
    let w = [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t];
    let mut out = [0.0; 2];
    for (q, wi) in p.iter().zip(w) {
        out = add2(out, scale2(*q, wi));
    }
    out
}

impl Profile {
    /// Ellipse med halvaksene `a` (langs U) og `b` (langs V).
    pub fn ellipse(a: f64, b: f64) -> Profile {
        let pts = [[a, 0.0], [0.0, b], [-a, 0.0], [0.0, -b]];
        let tangents = [[0.0, b], [-a, 0.0], [0.0, -b], [a, 0.0]];
        Profile {
            points: pts
                .iter()
                .zip(tangents)
                .map(|(p, t)| ProfilePoint {
                    position: *p,
                    handle_in: scale2(t, -CIRCLE_K),
                    handle_out: scale2(t, CIRCLE_K),
                    kind: AnchorKind::Smooth,
                })
                .collect(),
        }
    }

    pub fn circle() -> Profile {
        Profile::ellipse(1.0, 1.0)
    }

    /// Rektangel med rette kanter og hjørnepunkter.
    pub fn rectangle(w: f64, h: f64) -> Profile {
        Profile::polygon_of(&[[w, -h], [w, h], [-w, h], [-w, -h]])
    }

    /// En H med rette kanter og innhakk oppe og nede.
    pub fn h_shape() -> Profile {
        Profile::polygon_of(&[
            [1.0, -1.0],
            [1.0, 1.0],
            [0.45, 1.0],
            [0.45, 0.25],
            [-0.45, 0.25],
            [-0.45, 1.0],
            [-1.0, 1.0],
            [-1.0, -1.0],
            [-0.45, -1.0],
            [-0.45, -0.25],
            [0.45, -0.25],
            [0.45, -1.0],
        ])
    }

    fn polygon_of(points: &[Vec2]) -> Profile {
        Profile {
            points: points.iter().map(|p| ProfilePoint::corner(*p)).collect(),
        }
    }

    pub fn segment(&self, i: usize) -> [Vec2; 4] {
        let a = &self.points[i];
        let b = &self.points[(i + 1) % self.points.len()];
        [
            a.position,
            add2(a.position, a.handle_out),
            add2(b.position, b.handle_in),
            b.position,
        ]
    }

    fn is_straight(&self, i: usize) -> bool {
        let a = &self.points[i];
        let b = &self.points[(i + 1) % self.points.len()];
        len2(a.handle_out) < 1e-12 && len2(b.handle_in) < 1e-12
    }

    /// Omrisset som polygon. Ankerpunktene er med, så hjørner blir skarpe.
    pub fn polygon(&self) -> Vec<Vec2> {
        let mut out = Vec::new();
        for i in 0..self.points.len() {
            let seg = self.segment(i);
            out.push(seg[0]);
            if !self.is_straight(i) {
                for k in 1..POINTS_PER_CURVE {
                    out.push(bezier2(seg, k as f64 / POINTS_PER_CURVE as f64));
                }
            }
        }
        out
    }

    /// `n` punkter jevnt fordelt etter buelengde, fra første ankerpunkt.
    pub fn resample(&self, n: usize) -> Vec<Vec2> {
        resample_polygon(&self.polygon(), n)
    }

    /// Største avstand fra senterlinjen til omrisset.
    pub fn extent(&self) -> f64 {
        self.polygon().iter().map(|p| len2(*p)).fold(0.0, f64::max)
    }

    /// Legger inn et punkt i segment `i` ved parameter `t` uten å endre formen.
    pub fn insert_point(&mut self, i: usize, t: f64) -> Result<usize, String> {
        let n = self.points.len();
        if i >= n || !(0.0..=1.0).contains(&t) {
            return Err("Ugyldig plass for nytt punkt".into());
        }
        let [p0, p1, p2, p3] = self.segment(i);
        let lerp = |a: Vec2, b: Vec2| add2(a, scale2(sub2(b, a), t));
        let (q0, q1, q2) = (lerp(p0, p1), lerp(p1, p2), lerp(p2, p3));
        let (r0, r1) = (lerp(q0, q1), lerp(q1, q2));
        let mid = lerp(r0, r1);
        let straight = self.is_straight(i);
        let j = (i + 1) % n;
        if !straight {
            self.points[i].handle_out = sub2(q0, p0);
            self.points[j].handle_in = sub2(q2, p3);
        }
        let point = if straight {
            ProfilePoint::corner(mid)
        } else {
            ProfilePoint {
                position: mid,
                handle_in: sub2(r0, mid),
                handle_out: sub2(r1, mid),
                kind: AnchorKind::Smooth,
            }
        };
        self.points.insert(i + 1, point);
        Ok(i + 1)
    }

    /// Signert areal; positivt mot klokka.
    pub fn area(&self) -> f64 {
        let p = self.polygon();
        let n = p.len();
        (0..n)
            .map(|i| {
                let (a, b) = (p[i], p[(i + 1) % n]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>()
            / 2.0
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.points.len() < 3 {
            return Err("Et omriss trenger minst tre punkter".into());
        }
        let finite = |v: Vec2| v.iter().all(|x| x.is_finite());
        if self
            .points
            .iter()
            .any(|p| !finite(p.position) || !finite(p.handle_in) || !finite(p.handle_out))
        {
            return Err("Omrisset har ugyldige tall".into());
        }
        if self.area().abs() < 1e-6 {
            return Err("Omrisset har ikke noe areal".into());
        }
        if self_intersects(&self.polygon()) {
            return Err("Omrisset krysser seg selv".into());
        }
        Ok(())
    }
}

/// Jevn sampling av et lukket polygon etter buelengde.
pub fn resample_polygon(poly: &[Vec2], n: usize) -> Vec<Vec2> {
    let m = poly.len();
    let mut cum = vec![0.0];
    for i in 0..m {
        let l = len2(sub2(poly[(i + 1) % m], poly[i]));
        cum.push(cum[i] + l);
    }
    let total = cum[m];
    let mut out = Vec::with_capacity(n);
    let mut k = 0;
    for j in 0..n {
        let s = total * j as f64 / n as f64;
        while k + 1 < m && cum[k + 1] < s {
            k += 1;
        }
        let l = cum[k + 1] - cum[k];
        let t = if l > 1e-15 { (s - cum[k]) / l } else { 0.0 };
        let (a, b) = (poly[k], poly[(k + 1) % m]);
        out.push(add2(a, scale2(sub2(b, a), t)));
    }
    out
}

fn segments_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let orient = |p: Vec2, q: Vec2, r: Vec2| {
        let v = (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
        if v.abs() < 1e-12 {
            0.0
        } else {
            v.signum()
        }
    };
    let (o1, o2, o3, o4) = (
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    );
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

fn self_intersects(poly: &[Vec2]) -> bool {
    let n = poly.len();
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if segments_cross(poly[i], poly[(i + 1) % n], poly[j], poly[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}

/// Signert avstand fra `p` til et lukket polygon; negativ innenfor.
pub fn signed_distance(poly: &[Vec2], p: Vec2) -> f64 {
    let n = poly.len();
    let mut d = f64::INFINITY;
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (vi, vj) = (poly[i], poly[j]);
        let e = sub2(vj, vi);
        let w = sub2(p, vi);
        let ee = dot2(e, e);
        let t = if ee > 1e-24 {
            (dot2(w, e) / ee).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let b = sub2(w, scale2(e, t));
        d = d.min(dot2(b, b));
        if (vi[1] > p[1]) != (vj[1] > p[1])
            && p[0] < vi[0] + (p[1] - vi[1]) * (vj[0] - vi[0]) / (vj[1] - vi[1])
        {
            inside = !inside;
        }
        j = i;
    }
    if inside {
        -d.sqrt()
    } else {
        d.sqrt()
    }
}

/// Omrisset til en stasjon: eget eller koblet til en master.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileShape {
    Own { profile: Profile },
    Master { master: MasterId },
}

/// En profil på et sted langs splinen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Station {
    /// Andel av splinens buelengde i [0, 1].
    pub at: f64,
    pub shape: ProfileShape,
    /// Lokal størrelse; endrer ikke masteren eller andre forekomster.
    pub scale: f64,
    /// Lokal rotasjon i radianer i tverrsnittsplanet.
    pub rotation: f64,
}

impl Station {
    pub fn new(at: f64, profile: Profile) -> Station {
        Station {
            at,
            shape: ProfileShape::Own { profile },
            scale: 1.0,
            rotation: 0.0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.at) {
            return Err("Profilen må ligge på splinen".into());
        }
        if !(self.scale.is_finite() && self.scale > 0.0) || !self.rotation.is_finite() {
            return Err("Ugyldig størrelse eller rotasjon på profilen".into());
        }
        if let ProfileShape::Own { profile } = &self.shape {
            profile.validate()?;
        }
        Ok(())
    }
}

/// En stasjon med omrisset slått opp, klar til interpolasjon.
#[derive(Clone, Debug)]
pub struct ResolvedStation {
    pub at: f64,
    pub profile: Profile,
    pub scale: f64,
    pub rotation: f64,
}

fn transform(poly: &[Vec2], scale: f64, rotation: f64) -> Vec<Vec2> {
    let (s, c) = rotation.sin_cos();
    poly.iter()
        .map(|p| [scale * (c * p[0] - s * p[1]), scale * (s * p[0] + c * p[1])])
        .collect()
}

/// Forskyvningen av `b` som ligger nærmest `a` punkt for punkt.
fn best_shift(a: &[Vec2], b: &[Vec2]) -> usize {
    let n = a.len();
    (0..n)
        .min_by(|&x, &y| {
            let cost = |k: usize| -> f64 {
                (0..n)
                    .map(|i| {
                        let d = sub2(a[i], b[(i + k) % n]);
                        dot2(d, d)
                    })
                    .sum()
            };
            cost(x).total_cmp(&cost(y))
        })
        .unwrap_or(0)
}

/// Overgangen mellom stasjonene langs én spline.
pub struct ProfileTrack {
    stations: Vec<ResolvedStation>,
    /// Omrisset til hver stasjon som polygon og jevnt samplet.
    exact: Vec<Vec<Vec2>>,
    uniform: Vec<Vec<Vec2>>,
    /// Forskyvningen av neste stasjons sampling for minst vridning.
    shifts: Vec<usize>,
}

impl ProfileTrack {
    /// `stations` må være sortert etter `at`. Uten stasjoner er profilen en sirkel.
    pub fn new(stations: Vec<ResolvedStation>) -> ProfileTrack {
        let stations = if stations.is_empty() {
            vec![ResolvedStation {
                at: 0.0,
                profile: Profile::circle(),
                scale: 1.0,
                rotation: 0.0,
            }]
        } else {
            stations
        };
        let exact: Vec<_> = stations.iter().map(|s| s.profile.polygon()).collect();
        let uniform: Vec<_> = exact
            .iter()
            .map(|p| resample_polygon(p, RESAMPLE_POINTS))
            .collect();
        let shifts = (1..stations.len())
            .map(|i| {
                let a = transform(&uniform[i - 1], 1.0, stations[i - 1].rotation);
                let b = transform(&uniform[i], 1.0, stations[i].rotation);
                best_shift(&a, &b)
            })
            .collect();
        ProfileTrack {
            stations,
            exact,
            uniform,
            shifts,
        }
    }

    /// Omrisset i andelen `u` av buelengden, i enheter av tykkelsen.
    pub fn polygon_at(&self, u: f64) -> Vec<Vec2> {
        let st = &self.stations;
        let one = |i: usize| transform(&self.exact[i], st[i].scale, st[i].rotation);
        if u <= st[0].at || st.len() == 1 {
            return one(0);
        }
        let last = st.len() - 1;
        if u >= st[last].at {
            return one(last);
        }
        let i = st.partition_point(|s| s.at <= u).clamp(1, last);
        let (a, b) = (&st[i - 1], &st[i]);
        let span = b.at - a.at;
        let w = if span > 1e-12 { (u - a.at) / span } else { 0.0 };
        let scale = a.scale + (b.scale - a.scale) * w;
        if a.profile == b.profile {
            // Samme omriss i begge ender: behold hjørnene og bland bare størrelse og rotasjon.
            let rotation = a.rotation + (b.rotation - a.rotation) * w;
            return transform(&self.exact[i], scale, rotation);
        }
        // Omrissene blandes punkt for punkt i hver stasjons egen rotasjon,
        // med startpunktet som gir minst vridning.
        let ra = transform(&self.uniform[i - 1], 1.0, a.rotation);
        let rb = transform(&self.uniform[i], 1.0, b.rotation);
        let shift = self.shifts[i - 1];
        let n = ra.len();
        (0..n)
            .map(|k| {
                let (p, q) = (ra[k], rb[(k + shift) % n]);
                scale2(add2(p, scale2(sub2(q, p), w)), scale)
            })
            .collect()
    }

    /// Største utstrekning langs hele splinen, i enheter av tykkelsen.
    pub fn max_extent(&self) -> f64 {
        self.stations
            .iter()
            .zip(&self.exact)
            .map(|(s, p)| s.scale * p.iter().map(|q| len2(*q)).fold(0.0, f64::max))
            .fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sirkelen_har_radius_en() {
        let c = Profile::circle();
        c.validate().unwrap();
        for p in c.polygon() {
            assert!((len2(p) - 1.0).abs() < 3e-4, "{p:?}");
        }
        assert!((c.area() - std::f64::consts::PI).abs() < 1e-2);
    }

    #[test]
    fn h_er_konkav_med_rette_kanter_og_gyldig() {
        let h = Profile::h_shape();
        h.validate().unwrap();
        let poly = h.polygon();
        assert_eq!(poly.len(), 12);
        // Innhakket er utenfor, beina innenfor.
        assert!(signed_distance(&poly, [0.0, 0.6]) > 0.3);
        assert!(signed_distance(&poly, [0.0, 0.0]) < 0.0);
        assert!(signed_distance(&poly, [0.8, 0.8]) < 0.0);
        assert!((signed_distance(&poly, [2.0, 0.0]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn selvkryssing_avvises() {
        let bow = Profile::polygon_of(&[[1.0, 1.0], [-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0]]);
        assert!(bow.validate().is_err());
        let tiny = Profile::polygon_of(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]);
        assert!(tiny.validate().is_err());
    }

    #[test]
    fn nytt_punkt_endrer_ikke_formen() {
        let mut c = Profile::circle();
        let before = c.resample(64);
        c.insert_point(1, 0.3).unwrap();
        assert_eq!(c.points.len(), 5);
        let after = c.polygon();
        for p in after {
            assert!((len2(p) - 1.0).abs() < 3e-4);
        }
        let mut h = Profile::h_shape();
        h.insert_point(0, 0.5).unwrap();
        assert_eq!(h.points[1].position, [1.0, 0.0]);
        assert_eq!(h.points[1].kind, AnchorKind::Corner);
        assert_eq!(before.len(), 64);
    }

    #[test]
    fn overgangen_blander_omriss_storrelse_og_rotasjon() {
        let st = |at: f64, profile: Profile, scale: f64| ResolvedStation {
            at,
            profile,
            scale,
            rotation: 0.0,
        };
        let track = ProfileTrack::new(vec![
            st(0.2, Profile::circle(), 1.0),
            st(0.8, Profile::circle(), 2.0),
        ]);
        // Før første og etter siste stasjon beholdes profilen.
        let r = |u: f64| {
            track
                .polygon_at(u)
                .iter()
                .map(|p| len2(*p))
                .fold(0.0, f64::max)
        };
        assert!((r(0.0) - 1.0).abs() < 1e-3);
        assert!((r(1.0) - 2.0).abs() < 1e-3);
        assert!((r(0.5) - 1.5).abs() < 1e-3);
        assert!((track.max_extent() - 2.0).abs() < 1e-3);

        // Kopi av en H langs splinen: samme omriss gir eksakte hjørner.
        let h = ProfileTrack::new(vec![
            st(0.0, Profile::h_shape(), 1.0),
            st(1.0, Profile::h_shape(), 1.0),
        ]);
        assert_eq!(h.polygon_at(0.5), Profile::h_shape().polygon());

        // Sirkel til H: overgangen ligger mellom de to.
        let mix = ProfileTrack::new(vec![
            st(0.0, Profile::circle(), 1.0),
            st(1.0, Profile::h_shape(), 1.0),
        ]);
        let m = mix.polygon_at(0.5);
        assert_eq!(m.len(), RESAMPLE_POINTS);
        assert!(signed_distance(&m, [0.0, 0.0]) < 0.0);
    }

    #[test]
    fn startpunktet_velges_for_minst_vridning() {
        let a = Profile::rectangle(1.0, 1.0);
        let mut b = a.clone();
        b.points.rotate_left(2);
        let track = ProfileTrack::new(vec![
            ResolvedStation {
                at: 0.0,
                profile: a.clone(),
                scale: 1.0,
                rotation: 0.0,
            },
            ResolvedStation {
                at: 1.0,
                profile: b,
                scale: 1.0,
                rotation: 0.0,
            },
        ]);
        // Samme kvadrat med annet startpunkt: midten er fortsatt kvadratet, ikke en vridd form.
        let mid = track.polygon_at(0.5);
        let area: f64 = (0..mid.len())
            .map(|i| {
                let (p, q) = (mid[i], mid[(i + 1) % mid.len()]);
                p[0] * q[1] - q[0] * p[1]
            })
            .sum::<f64>()
            / 2.0;
        assert!((area.abs() - 4.0).abs() < 1e-6, "{area}");
    }
}
