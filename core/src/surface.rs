//! Mesh rundt spline-skjelettet.
//!
//! Hvert spline-hierarki (en rot med alle grener) er én kropp. Kroppen er
//! området der avstanden til en av splinene er mindre enn splinens tykkelse.
//! Grenene smeltes glatt sammen med forelderen (polynomisk glatt minimum),
//! så en forgrening blir én sammenhengende form og ikke overlappende rør.
//! Overflaten hentes ut av avstandsfeltet med «surface nets» på et jevnt
//! rutenett med ønsket kantlengde som rutestørrelse.
//!
//! Tverrsnittet er en sirkel med splinens tykkelse som radius, eller
//! profilene langs splinen (se `profile`). Endene blir avrundet, så en kort
//! spline med stor tykkelse er en kule. Med flat ende beholdes hele
//! endeprofilen helt ut.

use crate::curve::{add, scale};
use crate::mesh::{dot, length, sub, TriMesh, Vec3};
use crate::profile::{signed_distance, ProfileTrack, Vec2};
use crate::skeleton::{bezier_point, Skeleton, SplineId, WorldSpline};
use std::collections::BTreeMap;

/// Punkter per Bézier-segment når kurven gjøres om til linjestykker.
const POINTS_PER_SEGMENT: usize = 24;
/// Største antall rutepunkter i ett felt, for å holde minnebruken nede.
const MAX_GRID_POINTS: usize = 12_000_000;

struct Tube {
    points: Vec<Vec3>,
    /// Største avstand fra kurven til overflaten, for boksene.
    radius: f64,
    /// Profiler langs kurven; `None` er en sirkel med radius `radius`.
    field: Option<ProfileField>,
}

/// Tverrsnittet langs en spline, forhåndsberegnet i jevne trinn.
struct ProfileField {
    /// Buelengden langs linjestykkene frem til hvert punkt.
    cum: Vec<f64>,
    bins: Vec<Bin>,
    flat_end: bool,
}

struct Bin {
    u: Vec3,
    v: Vec3,
    t: Vec3,
    /// Omrisset i mm.
    poly: Vec<Vec2>,
    extent: f64,
}

impl ProfileField {
    fn new(
        points: &[Vec3],
        w: &WorldSpline,
        track: &ProfileTrack,
        thickness: f64,
        h: f64,
        flat_end: bool,
    ) -> ProfileField {
        let mut cum = vec![0.0];
        for p in points.windows(2) {
            cum.push(cum.last().unwrap() + length(sub(p[1], p[0])));
        }
        let total = *cum.last().unwrap();
        let n = ((total / (0.5 * h)).ceil() as usize).clamp(8, 2000);
        let bins = (0..=n)
            .map(|b| {
                let u = b as f64 / n as f64;
                let f = w.frame_at(u);
                let poly: Vec<Vec2> = track
                    .polygon_at(u)
                    .iter()
                    .map(|p| [p[0] * thickness, p[1] * thickness])
                    .collect();
                let extent = poly.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max);
                Bin {
                    u: f.u,
                    v: f.v,
                    t: f.t,
                    poly,
                    extent,
                }
            })
            .collect();
        ProfileField {
            cum,
            bins,
            flat_end,
        }
    }

    /// Signert avstand fra `p`, der `c` er nærmeste punkt på linjestykke
    /// `i` med parameter `t`.
    fn distance(&self, p: Vec3, i: usize, t: f64, c: Vec3) -> f64 {
        let total = *self.cum.last().unwrap();
        let s = self.cum[i] + t * (self.cum[i + 1] - self.cum[i]);
        let n = self.bins.len() - 1;
        let k = if total > 0.0 {
            ((s / total) * n as f64).round() as usize
        } else {
            0
        };
        let bin = &self.bins[k.min(n)];
        let w = sub(p, c);
        let q = [dot(w, bin.u), dot(w, bin.v)];
        let ax = dot(w, bin.t);
        let last = self.cum.len() - 2;
        // Avstand forbi enden av kurven, langs kurvens retning.
        let out = if i == 0 && t <= 0.0 && ax < 0.0 {
            -ax
        } else if i == last && t >= 1.0 && ax > 0.0 {
            ax
        } else {
            0.0
        };
        let d = signed_distance(&bin.poly, q);
        if out <= 0.0 {
            return d;
        }
        if self.flat_end {
            // Profilen trukket rett ut og kuttet flatt.
            return d.max(0.0).hypot(out);
        }
        // Avrundet ende: profilen krymper som en halvkule utover.
        let r = bin.extent;
        if out >= r {
            return out - r + d.max(0.0);
        }
        let k = (1.0 - (out / r).powi(2)).sqrt();
        let scaled: Vec<Vec2> = bin.poly.iter().map(|p| [p[0] * k, p[1] * k]).collect();
        signed_distance(&scaled, q).max(out - r)
    }
}

fn tubes(
    world: &BTreeMap<SplineId, WorldSpline>,
    skeleton: &Skeleton,
    root: SplineId,
    h: f64,
) -> Result<Vec<Tube>, String> {
    let mut out = Vec::new();
    for id in skeleton.descendants(root) {
        let (Some(w), Some(spline)) = (world.get(&id), skeleton.spline(id)) else {
            continue;
        };
        if spline.thickness <= 0.0 {
            continue;
        }
        let mut points = vec![w.segments[0][0]];
        for seg in &w.segments {
            points.extend(
                (1..=POINTS_PER_SEGMENT)
                    .map(|i| bezier_point(seg, i as f64 / POINTS_PER_SEGMENT as f64)),
            );
        }
        let (radius, field) = if spline.profiles.is_empty() && !spline.flat_end {
            (spline.thickness, None)
        } else {
            let track = ProfileTrack::new(skeleton.resolved_profiles(id)?);
            let field = ProfileField::new(&points, w, &track, spline.thickness, h, spline.flat_end);
            (spline.thickness * track.max_extent(), Some(field))
        };
        out.push(Tube {
            points,
            radius,
            field,
        });
    }
    Ok(out)
}

/// Avstanden til linjestykket a–b og parameteren til nærmeste punkt.
fn segment_project(p: Vec3, a: Vec3, b: Vec3) -> (f64, f64) {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    let t = if l2 < 1e-24 {
        0.0
    } else {
        (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0)
    };
    (length(sub(p, add(a, scale(ab, t)))), t)
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
    let tubes = tubes(&world, skeleton, root, h)?;
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
    let mirror = skeleton.mirror();
    if mirror {
        // Rutenettet ligger symmetrisk om x = 0, så hvert rutepunkt har et
        // speilbilde i et annet rutepunkt.
        let m = (lo[0].abs().max(hi[0].abs()) / h).ceil();
        lo[0] = -m * h;
        hi[0] = m * h;
    }
    let mut n = [0, 1, 2].map(|a| ((hi[a] - lo[a]) / h).ceil() as usize + 1);
    if mirror {
        n[0] = ((hi[0] - lo[0]) / h).round() as usize + 1;
    }
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
        accumulate(&mut grid, tube, h);
    }
    if mirror {
        symmetrize(&mut grid);
    }
    for (cutter, cut) in skeleton.cutters_of(root) {
        subtract(&mut grid, &world, skeleton, cutter, cut.sharp, mirror)?;
    }
    for v in grid.values.iter_mut() {
        *v = v.min(far);
    }
    Ok(surface_nets(&mut grid))
}

/// Speiler feltet om x = 0. Rutenettet må ligge symmetrisk om x = 0.
fn symmetrize(grid: &mut Grid) {
    let n0 = grid.n[0];
    for row in grid.values.chunks_mut(n0) {
        for i in 0..n0 / 2 {
            let v = row[i].min(row[n0 - 1 - i]);
            row[i] = v;
            row[n0 - 1 - i] = v;
        }
    }
}

/// Glatt maksimum: snittet av to former med en avrundet overgang av bredde `k`.
fn smooth_max(a: f64, b: f64, k: f64) -> f64 {
    -smooth_min(-a, -b, k)
}

/// Bredden på overgangen langs et kutt. En skarp kant får en overgang på
/// én rute, så kanten er så skarp rutenettet kan gjengi uten at
/// overflaten får tvetydige ruter (og dermed hull). Myk overgang er som
/// sammensmeltingen av det tynneste røret i kutteren.
fn cut_blend(tubes: &[Tube], sharp: bool, h: f64) -> f64 {
    let r = tubes.iter().map(|t| t.radius).fold(f64::INFINITY, f64::min);
    if sharp || !r.is_finite() {
        h
    } else {
        blend_width(r).max(h)
    }
}

/// Trekker den negative formen med roten `cutter` fra feltet i rutenettet
/// (issue #12 punkt 4). Kutterens felt beregnes i et utsnitt av samme
/// rutenett rundt kutteren, så punktene faller sammen.
fn subtract(
    grid: &mut Grid,
    world: &BTreeMap<SplineId, WorldSpline>,
    skeleton: &Skeleton,
    cutter: SplineId,
    sharp: bool,
    mirror: bool,
) -> Result<(), String> {
    let h = grid.h;
    let tubes = tubes(world, skeleton, cutter, h)?;
    if tubes.is_empty() {
        return Ok(());
    }
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
    if mirror {
        let m = lo[0].abs().max(hi[0].abs());
        lo[0] = -m;
        hi[0] = m;
    }
    let grid_hi = grid.point(grid.n[0] - 1, grid.n[1] - 1, grid.n[2] - 1);
    if (0..3).any(|a| hi[a] < grid.origin[a] || lo[a] > grid_hi[a]) {
        return Ok(());
    }
    let [(i0, i1), (j0, j1), (k0, k1)] = grid.range(lo, hi);
    let mut sub = Grid {
        origin: grid.point(i0, j0, k0),
        h,
        n: [i1 - i0 + 1, j1 - j0 + 1, k1 - k0 + 1],
        values: vec![f64::INFINITY; (i1 - i0 + 1) * (j1 - j0 + 1) * (k1 - k0 + 1)],
    };
    for t in &tubes {
        accumulate(&mut sub, t, h);
    }
    let k = cut_blend(&tubes, sharp, h);
    let n0 = grid.n[0];
    for kk in 0..sub.n[2] {
        for jj in 0..sub.n[1] {
            for ii in 0..sub.n[0] {
                let mut c = sub.values[sub.index(ii, jj, kk)];
                if mirror {
                    // Speilbildet av rutepunktet i hovedrutenettet.
                    let mi = n0 - 1 - (i0 + ii);
                    if (i0..=i1).contains(&mi) {
                        c = c.min(sub.values[sub.index(mi - i0, jj, kk)]);
                    }
                }
                if !c.is_finite() {
                    continue;
                }
                let idx = grid.index(i0 + ii, j0 + jj, k0 + kk);
                let v = &mut grid.values[idx];
                if v.is_finite() {
                    *v = smooth_max(*v, -c, k);
                }
            }
        }
    }
    Ok(())
}

/// Legger feltet til ett rør inn i rutenettet med glatt minimum.
fn accumulate(grid: &mut Grid, tube: &Tube, h: f64) {
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
    let size = li * lj * (ek - bk + 1);
    // Nærmeste linjestykke for hvert rutepunkt: avstand, stykke og parameter.
    let mut near = vec![(f64::INFINITY, 0u32, 0.0f64); size];
    for (si, w) in tube.points.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let slo = [0, 1, 2].map(|i| a[i].min(b[i]) - reach);
        let shi = [0, 1, 2].map(|i| a[i].max(b[i]) + reach);
        let [(i0, i1), (j0, j1), (k0, k1)] = grid.range(slo, shi);
        for kk in k0..=k1 {
            for jj in j0..=j1 {
                for ii in i0..=i1 {
                    let (d, t) = segment_project(grid.point(ii, jj, kk), a, b);
                    let e = &mut near[((kk - bk) * lj + (jj - bj)) * li + (ii - bi)];
                    if d < e.0 {
                        *e = (d, si as u32, t);
                    }
                }
            }
        }
    }
    let mut local = vec![f64::INFINITY; size];
    for kk in bk..=ek {
        for jj in bj..=ej {
            for ii in bi..=ei {
                let idx = ((kk - bk) * lj + (jj - bj)) * li + (ii - bi);
                let (d, si, t) = near[idx];
                if !d.is_finite() {
                    continue;
                }
                local[idx] = match &tube.field {
                    None => d - tube.radius,
                    Some(f) => {
                        let si = si as usize;
                        let (a, b) = (tube.points[si], tube.points[si + 1]);
                        let c = add(a, scale(sub(b, a), t));
                        f.distance(grid.point(ii, jj, kk), si, t, c)
                    }
                };
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

impl Tube {
    /// Signert avstand fra `p` til overflaten rundt denne splinen.
    fn distance(&self, p: Vec3) -> f64 {
        let mut best = (f64::INFINITY, 0, 0.0);
        for (si, w) in self.points.windows(2).enumerate() {
            let (d, t) = segment_project(p, w[0], w[1]);
            if d < best.0 {
                best = (d, si, t);
            }
        }
        let (d, si, t) = best;
        match &self.field {
            None => d - self.radius,
            Some(f) => {
                let (a, b) = (self.points[si], self.points[si + 1]);
                f.distance(p, si, t, add(a, scale(sub(b, a), t)))
            }
        }
    }
}

/// Avstandsfeltet til en kropp i enkeltpunkter, likt feltet i rutenettet.
pub struct BodyField {
    tubes: Vec<Tube>,
    /// Boksen rundt hvert rør, med plass til sammensmeltingen.
    boxes: Vec<(Vec3, Vec3)>,
    mirror: bool,
    /// Negative former som skjærer i kroppen, med overgangsbredden.
    cuts: Vec<(BodyField, f64)>,
}

impl BodyField {
    /// Feltet til kroppen `root` med de negative formene som skjærer i den.
    pub fn new(skeleton: &Skeleton, root: SplineId, h: f64) -> Result<BodyField, String> {
        let mut field = BodyField::plain(skeleton, root, h)?;
        for (cutter, cut) in skeleton.cutters_of(root) {
            let c = BodyField::plain(skeleton, cutter, h)?;
            let k = cut_blend(&c.tubes, cut.sharp, h);
            field.cuts.push((c, k));
        }
        Ok(field)
    }

    /// Feltet til kroppen alene.
    fn plain(skeleton: &Skeleton, root: SplineId, h: f64) -> Result<BodyField, String> {
        let world = skeleton.evaluate()?;
        let tubes = tubes(&world, skeleton, root, h)?;
        let boxes = tubes
            .iter()
            .map(|t| {
                let pad = t.radius + blend_width(t.radius);
                let lo = [0, 1, 2]
                    .map(|i| t.points.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min) - pad);
                let hi = [0, 1, 2].map(|i| {
                    t.points
                        .iter()
                        .map(|p| p[i])
                        .fold(f64::NEG_INFINITY, f64::max)
                        + pad
                });
                (lo, hi)
            })
            .collect();
        Ok(BodyField {
            tubes,
            boxes,
            mirror: skeleton.mirror(),
            cuts: Vec::new(),
        })
    }

    fn one_side(&self, p: Vec3) -> f64 {
        let mut v = f64::INFINITY;
        for (tube, (lo, hi)) in self.tubes.iter().zip(&self.boxes) {
            // Et rør langt unna endrer ikke det glatte minimumet.
            let outside = (0..3)
                .map(|i| (lo[i] - p[i]).max(p[i] - hi[i]).max(0.0))
                .fold(0.0f64, |a, b| a.hypot(b));
            if v.is_finite() && outside > v + blend_width(tube.radius) {
                continue;
            }
            let d = tube.distance(p);
            v = if v.is_finite() {
                smooth_min(v, d, blend_width(tube.radius))
            } else {
                d
            };
        }
        v
    }

    pub fn value(&self, p: Vec3) -> f64 {
        let mut v = self.one_side(p);
        if self.mirror {
            v = v.min(self.one_side([-p[0], p[1], p[2]]));
        }
        for (cutter, k) in &self.cuts {
            let c = cutter.value(p);
            if c.is_finite() && v.is_finite() {
                v = smooth_max(v, -c, *k);
            }
        }
        v
    }

    fn gradient(&self, p: Vec3, e: f64) -> Vec3 {
        let d = |k: usize| {
            let (mut a, mut b) = (p, p);
            a[k] += e;
            b[k] -= e;
            (self.value(a) - self.value(b)) / (2.0 * e)
        };
        [d(0), d(1), d(2)]
    }

    /// Flytter `p` inn på overflaten med noen Newton-steg langs gradienten.
    /// Hvert steg er begrenset til `max_step`, så punktet ikke hopper.
    pub fn project(&self, mut p: Vec3, max_step: f64) -> Vec3 {
        for _ in 0..4 {
            let f = self.value(p);
            if !f.is_finite() || f.abs() < max_step * 1e-3 {
                break;
            }
            let g = self.gradient(p, max_step * 0.05);
            let g2 = dot(g, g);
            if g2 < 1e-12 {
                break;
            }
            let mut step = scale(g, -f / g2);
            let l = length(step);
            if l > max_step {
                step = scale(step, max_step / l);
            }
            p = add(p, step);
        }
        p
    }
}

/// Flere trekanter i knekkene: kanter der nabotrekantene står i større
/// vinkel enn `angle_deg` deles, og punktene flyttes inn på overflaten.
/// Gjentas `levels` ganger, så skarpe kanter får stadig finere trekanter.
pub fn refine_creases(mesh: &mut TriMesh, field: &BodyField, h: f64, levels: u32, angle_deg: f64) {
    use crate::refine::{edge_key, split_edges, Edge};
    use std::collections::{HashMap, HashSet};
    let limit = angle_deg.to_radians().cos();
    for level in 0..levels {
        let normals: Vec<Vec3> = mesh
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
                crate::mesh::normalize(crate::mesh::cross(sub(b, a), sub(c, a)))
            })
            .collect();
        let mut faces: HashMap<Edge, Vec<usize>> = HashMap::new();
        for (f, t) in mesh.triangles.iter().enumerate() {
            for k in 0..3 {
                faces
                    .entry(edge_key(t[k], t[(k + 1) % 3]))
                    .or_default()
                    .push(f);
            }
        }
        // Kanter kortere enn dette er fine nok for dette nivået.
        let min_len = h / 2f64.powi(level as i32 + 2);
        let edges: HashSet<Edge> = faces
            .iter()
            .filter(|(e, f)| {
                f.len() == 2
                    && dot(normals[f[0]], normals[f[1]]) < limit
                    && length(sub(
                        mesh.positions[e.0 as usize],
                        mesh.positions[e.1 as usize],
                    )) > min_len
            })
            .map(|(e, _)| *e)
            .collect();
        if edges.is_empty() {
            break;
        }
        let touched: HashSet<u32> = edges.iter().flat_map(|e| [e.0, e.1]).collect();
        let step = h / 2f64.powi(level as i32 + 1);
        split_edges(mesh, &edges, |a, b| {
            field.project(scale(add(a, b), 0.5), step)
        });
        let mut touched: Vec<u32> = touched.into_iter().collect();
        touched.sort_unstable();
        for i in touched {
            let p = mesh.positions[i as usize];
            mesh.positions[i as usize] = field.project(p, step);
        }
    }
}

/// Fjerner tvetydige sideflater i rutenettet: en side der hjørnene på den
/// ene diagonalen er inne og hjørnene på den andre ute. Der deles en kant
/// mellom to ruter av fire firkanter, og meshen blir ikke en lukket flate.
/// Det skjer der to flater møtes i en skarp kile, for eksempel langs kanten
/// av et kutt. Hjørnet nærmest overflaten flyttes over til den andre siden
/// (en endring på under én rute), slik at diagonalen som er sammenhengende
/// i sidens midtpunkt kobles. Gjentas til ingen sider er tvetydige.
fn resolve_ambiguous_faces(grid: &mut Grid) {
    let [nx, ny, nz] = grid.n;
    if nx < 2 || ny < 2 || nz < 2 {
        return;
    }
    let eps = grid.h * 1e-6;
    let dirs = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
    for _ in 0..8 {
        let mut changed = false;
        // Sidene med normal langs aksen `axis`, utspent av de to andre aksene.
        for axis in 0..3 {
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            let (da, db) = (dirs[a], dirs[b]);
            let mut lim = grid.n;
            lim[a] -= 1;
            lim[b] -= 1;
            for k in 0..lim[2] {
                for j in 0..lim[1] {
                    for i in 0..lim[0] {
                        let at = |o: [usize; 3]| grid.index(i + o[0], j + o[1], k + o[2]);
                        let idx = [
                            at([0, 0, 0]),
                            at(da),
                            at([da[0] + db[0], da[1] + db[1], da[2] + db[2]]),
                            at(db),
                        ];
                        let v = idx.map(|x| grid.values[x]);
                        let s = v.map(|x| x < 0.0);
                        if !(s[0] == s[2] && s[1] == s[3] && s[0] != s[1]) {
                            continue;
                        }
                        // Sidens midtpunkt avgjør hvilken diagonal som henger sammen.
                        let finite: Vec<f64> =
                            v.iter().copied().filter(|x| x.is_finite()).collect();
                        let centre_inside = finite.len() == 4 && finite.iter().sum::<f64>() < 0.0;
                        // Flytt hjørnet nærmest overflaten på diagonalen som ikke henger sammen.
                        let pick = (0..4)
                            .filter(|&c| s[c] != centre_inside)
                            .min_by(|&x, &y| v[x].abs().total_cmp(&v[y].abs()))
                            .unwrap();
                        grid.values[idx[pick]] = if centre_inside { -eps } else { eps };
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// Overflaten der feltet skifter fortegn. Ett punkt per rute som krysses,
/// plassert i snittet av kantkryssingene, og én firkant per rutekant som
/// krysses. Feltet er positivt langs randen, så overflaten er lukket.
fn surface_nets(grid: &mut Grid) -> TriMesh {
    resolve_ambiguous_faces(grid);
    let grid = &*grid;
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
            orientation: Default::default(),
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

    fn rod(profiles: Vec<E>) -> (Skeleton, SplineId) {
        let mut sk = Skeleton::new();
        let id = sk
            .apply(&E::AddRoot {
                start: [0.0, 0.0, 0.0],
                direction: [1.0, 0.0, 0.0],
                length: 60.0,
                thickness: 10.0,
            })
            .unwrap()
            .unwrap();
        for p in profiles {
            sk.apply(&p).unwrap();
        }
        (sk, id)
    }

    /// Antall meshpunkter innenfor en boks.
    fn count_in(m: &TriMesh, lo: Vec3, hi: Vec3) -> usize {
        m.positions
            .iter()
            .filter(|p| (0..3).all(|a| p[a] > lo[a] && p[a] < hi[a]))
            .count()
    }

    #[test]
    fn h_profil_gir_innhakk_langs_hele_splinen() {
        use crate::profile::Profile;
        // H i begge ender gir H langs hele splinen.
        let (sk, id) = rod(vec![
            E::AddProfile {
                spline: 1,
                at: 0.0,
                from: None,
            },
            E::SetProfile {
                spline: 1,
                station: 0,
                profile: Profile::h_shape(),
            },
            E::AddProfile {
                spline: 1,
                at: 1.0,
                from: Some(0),
            },
            E::SetFlatEnd {
                spline: 1,
                flat: true,
            },
        ]);
        let m = body_mesh(&sk, id, 1.0).unwrap();
        assert!(m.is_closed_manifold());
        assert_eq!(components(&m), 1);
        // H-en er 20 x 20 mm og ligger i planet vinkelrett på x.
        for p in &m.positions {
            assert!(p[1].abs() < 10.8 && p[2].abs() < 10.8, "{p:?}");
            assert!(p[0] > -0.8 && p[0] < 60.8, "flat ende: {p:?}");
        }
        // Flaten går inn i innhakket (|v| < 2,5 mm mellom beina) midt på.
        let notch = m
            .positions
            .iter()
            .filter(|p| p[0] > 20.0 && p[0] < 40.0)
            .map(|p| p[1].abs().min(p[2].abs()))
            .fold(f64::MAX, f64::min);
        assert!(notch < 3.5, "{notch}");
        // Volumet er H-ens areal ganger lengden.
        let area = Profile::h_shape().area().abs() * 100.0;
        let v = m.signed_volume();
        assert!((v - area * 60.0).abs() / (area * 60.0) < 0.06, "{v}");
    }

    #[test]
    fn profilene_blandes_langs_splinen() {
        use crate::profile::Profile;
        let (sk, id) = rod(vec![
            E::AddProfile {
                spline: 1,
                at: 0.0,
                from: None,
            },
            E::AddProfile {
                spline: 1,
                at: 1.0,
                from: Some(0),
            },
            E::SetStation {
                spline: 1,
                station: 1,
                at: 1.0,
                scale: 2.0,
                rotation: 0.0,
            },
            E::SetProfile {
                spline: 1,
                station: 1,
                profile: Profile::rectangle(1.0, 0.5),
            },
        ]);
        let m = body_mesh(&sk, id, 1.0).unwrap();
        assert!(m.is_closed_manifold());
        // Ved starten en sirkel med radius 10, ved enden et 40 x 20 mm rektangel.
        let near = |x: f64| -> (f64, f64) {
            m.positions
                .iter()
                .filter(|p| (p[0] - x).abs() < 1.0)
                .fold((0.0, 0.0), |(a, b), p| {
                    (f64::max(a, p[1].abs()), f64::max(b, p[2].abs()))
                })
        };
        let (y0, z0) = near(2.0);
        assert!(
            (y0 - 10.0).abs() < 1.2 && (z0 - 10.0).abs() < 1.2,
            "{y0} {z0}"
        );
        let (y1, z1) = near(57.0);
        assert!(y1 > z1 + 4.0, "{y1} {z1}");
        assert!(count_in(&m, [55.0, 15.0, -2.0], [58.0, 22.0, 2.0]) > 0 || y1 > 15.0);
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

    /// Hvor nær meshen kommer hjørnene i et rektangulært tverrsnitt 20 × 10 mm
    /// midt på staven (2 er et helt skarpt hjørne).
    fn corner_reach(m: &TriMesh) -> f64 {
        m.positions
            .iter()
            .filter(|p| (20.0..40.0).contains(&p[0]))
            .map(|p| {
                (p[1].abs() / 10.0 + p[2].abs() / 5.0).max(p[2].abs() / 10.0 + p[1].abs() / 5.0)
            })
            .fold(0.0, f64::max)
    }

    #[test]
    fn knekker_faar_flere_trekanter_og_skarpere_kanter() {
        let (sk, id) = rod(vec![E::AddProfile {
            spline: 1,
            at: 0.5,
            from: None,
        }]);
        let mut sk = sk;
        sk.apply(&E::SetProfile {
            spline: id,
            station: 0,
            profile: crate::profile::Profile::rectangle(1.0, 0.5),
        })
        .unwrap();
        let h = 2.0;
        let plain = body_mesh(&sk, id, h).unwrap();
        let mut fine = plain.clone();
        let field = BodyField::new(&sk, id, h).unwrap();
        refine_creases(&mut fine, &field, h, 2, 25.0);
        assert!(fine.is_closed_manifold());
        assert!(fine.triangles.len() > plain.triangles.len());
        assert!(
            corner_reach(&fine) > corner_reach(&plain) + 0.02,
            "{} {}",
            corner_reach(&fine),
            corner_reach(&plain)
        );
        // Glatte flater får nesten ingen nye trekanter.
        let (ball, b) = sphere(20.0);
        let mut m = body_mesh(&ball, b, h).unwrap();
        let before = m.triangles.len();
        let field = BodyField::new(&ball, b, h).unwrap();
        refine_creases(&mut m, &field, h, 2, 25.0);
        assert!(m.triangles.len() < before + before / 50);
    }

    #[test]
    fn speilmodus_speiler_kroppen_om_x_null() {
        let mut sk = Skeleton::new();
        let id = sk
            .apply(&E::AddRoot {
                start: [20.0, 0.0, 0.0],
                direction: [0.0, 1.0, 0.0],
                length: 30.0,
                thickness: 5.0,
            })
            .unwrap()
            .unwrap();
        let one = body_mesh(&sk, id, 1.5).unwrap();
        sk.apply(&E::SetMirror { mirror: true }).unwrap();
        let both = body_mesh(&sk, id, 1.5).unwrap();
        assert_eq!(components(&one), 1);
        assert_eq!(components(&both), 2);
        assert!(
            (both.signed_volume() - 2.0 * one.signed_volume()).abs() < 0.02 * both.signed_volume()
        );
        let min_x = both
            .positions
            .iter()
            .map(|p| p[0])
            .fold(f64::INFINITY, f64::min);
        assert!((min_x + 25.0).abs() < 1.0, "{min_x}");
        // Feltet i enkeltpunkter følger speilingen.
        let field = BodyField::new(&sk, id, 1.5).unwrap();
        assert!(field.value([-20.0, 15.0, 0.0]) < -4.0);
        assert!((field.value([-25.0, 15.0, 0.0])).abs() < 0.1);
    }

    /// En kule med radius 20 i (0, 25, 0) og en kule med radius 10 i `at`.
    fn ball_and_cutter(at: Vec3) -> (Skeleton, SplineId, SplineId) {
        let (mut sk, ball) = sphere(20.0);
        let cutter = sk
            .apply(&E::AddRoot {
                start: [at[0], at[1] - 0.25, at[2]],
                direction: [0.0, 1.0, 0.0],
                length: 0.5,
                thickness: 10.0,
            })
            .unwrap()
            .unwrap();
        (sk, ball, cutter)
    }

    fn cut(targets: Vec<SplineId>) -> crate::skeleton::Cut {
        crate::skeleton::Cut {
            targets,
            active: true,
            sharp: true,
        }
    }

    #[test]
    fn negativ_form_skjaerer_bare_i_valgte_lag() {
        // Kutteren sitter på kulas side ved x = 20.
        let (mut sk, ball, cutter) = ball_and_cutter([20.0, 25.25, 0.0]);
        let whole = body_mesh(&sk, ball, 1.0).unwrap();
        sk.apply(&E::SetCut {
            spline: cutter,
            cut: Some(cut(vec![ball])),
        })
        .unwrap();
        let cut_mesh = body_mesh(&sk, ball, 1.0).unwrap();
        assert!(cut_mesh.is_closed_manifold());
        // Volumet av kula minus linsen der kulene overlapper.
        let (r1, r2, d) = (20.0f64, 10.0f64, 20.0f64);
        let lens = PI
            * (r1 + r2 - d).powi(2)
            * (d * d + 2.0 * d * r2 - 3.0 * r2 * r2 + 2.0 * d * r1 + 6.0 * r1 * r2 - 3.0 * r1 * r1)
            / (12.0 * d);
        let expected = whole.signed_volume() - lens;
        let v = cut_mesh.signed_volume();
        assert!((v - expected).abs() / expected < 0.03, "{v} mot {expected}");
        // Ingen punkter inne i kutteren, og kula når ikke lenger x = 20.
        for p in &cut_mesh.positions {
            assert!(length(sub(*p, [20.0, 25.25, 0.0])) > 10.0 - 0.6, "{p:?}");
        }
        let max_x = cut_mesh
            .positions
            .iter()
            .map(|p| p[0])
            .fold(f64::MIN, f64::max);
        // Kanten av kuttet ligger der kulene møtes, ved x = 17,5.
        assert!(max_x < 18.0, "{max_x}");
        // Kutteren selv er uendret.
        let own = body_mesh(&sk, cutter, 1.0).unwrap();
        assert!((own.signed_volume() - 4.0 / 3.0 * PI * 1000.0).abs() < 0.06 * 4188.0);
        // Feltet i enkeltpunkter (brukt til fortetting) har samme kutt.
        let field = BodyField::new(&sk, ball, 1.0).unwrap();
        assert!(field.value([19.0, 25.25, 0.0]) > 0.0);
        assert!(field.value([-10.0, 25.25, 0.0]) < 0.0);
    }

    #[test]
    fn negativ_form_som_ikke_er_aktiv_skjaerer_ikke() {
        let (mut sk, ball, cutter) = ball_and_cutter([20.0, 25.25, 0.0]);
        let whole = body_mesh(&sk, ball, 1.5).unwrap();
        let mut c = cut(vec![ball]);
        c.active = false;
        sk.apply(&E::SetCut {
            spline: cutter,
            cut: Some(c),
        })
        .unwrap();
        assert_eq!(
            body_mesh(&sk, ball, 1.5).unwrap().positions,
            whole.positions
        );
        // Uten mål skjærer den heller ikke.
        sk.apply(&E::SetCut {
            spline: cutter,
            cut: Some(cut(vec![])),
        })
        .unwrap();
        assert_eq!(
            body_mesh(&sk, ball, 1.5).unwrap().positions,
            whole.positions
        );
    }

    #[test]
    fn myk_overgang_fjerner_mer_enn_skarp_kant() {
        let (mut sk, ball, cutter) = ball_and_cutter([20.0, 25.25, 0.0]);
        sk.apply(&E::SetCut {
            spline: cutter,
            cut: Some(cut(vec![ball])),
        })
        .unwrap();
        let sharp = body_mesh(&sk, ball, 1.0).unwrap().signed_volume();
        let mut c = cut(vec![ball]);
        c.sharp = false;
        sk.apply(&E::SetCut {
            spline: cutter,
            cut: Some(c),
        })
        .unwrap();
        let soft = body_mesh(&sk, ball, 1.0).unwrap();
        assert!(soft.is_closed_manifold());
        assert!(
            soft.signed_volume() < sharp,
            "{} {sharp}",
            soft.signed_volume()
        );
    }

    #[test]
    fn speilet_negativ_form_skjaerer_paa_begge_sider() {
        // En stav langs x, og en kutter ved x = 15 som speiles til x = -15.
        let mut sk = Skeleton::new();
        let rod = sk
            .apply(&E::AddRoot {
                start: [-30.0, 0.0, 0.0],
                direction: [1.0, 0.0, 0.0],
                length: 60.0,
                thickness: 5.0,
            })
            .unwrap()
            .unwrap();
        let cutter = sk
            .apply(&E::AddRoot {
                start: [15.0, -0.25, 0.0],
                direction: [0.0, 1.0, 0.0],
                length: 0.5,
                thickness: 8.0,
            })
            .unwrap()
            .unwrap();
        sk.apply(&E::SetMirror { mirror: true }).unwrap();
        sk.apply(&E::SetCut {
            spline: cutter,
            cut: Some(cut(vec![rod])),
        })
        .unwrap();
        let m = body_mesh(&sk, rod, 1.0).unwrap();
        assert!(m.is_closed_manifold());
        // Staven er kuttet i tre biter: midten og de to endene.
        assert_eq!(components(&m), 3);
        for x in [15.0, -15.0] {
            assert!(m
                .positions
                .iter()
                .all(|p| length(sub(*p, [x, 0.0, 0.0])) > 7.0));
        }
    }

    #[test]
    fn en_enkelt_kontur_former_omraadet_rundt_seg_og_kan_flyttes() {
        use crate::profile::Profile;
        let flat = |at: f64| {
            rod(vec![
                E::AddProfile {
                    spline: 1,
                    at,
                    from: None,
                },
                E::SetProfile {
                    spline: 1,
                    station: 0,
                    profile: Profile::ellipse(2.0, 0.5),
                },
            ])
        };
        // Bredden (langs y eller z) der konturen ligger og ved den andre enden.
        let width = |m: &TriMesh, x0: f64, x1: f64| {
            m.positions
                .iter()
                .filter(|p| p[0] > x0 && p[0] < x1)
                .map(|p| p[1].abs().max(p[2].abs()))
                .fold(0.0, f64::max)
        };
        let (sk, id) = flat(0.2);
        let near = body_mesh(&sk, id, 1.0).unwrap();
        assert!(near.is_closed_manifold());
        // Ved konturen er formen 20 mm bred; mot den andre enden går den
        // over i vanlig tykkelse (10 mm).
        assert!(
            width(&near, 10.0, 14.0) > 18.0,
            "{}",
            width(&near, 10.0, 14.0)
        );
        assert!(
            width(&near, 52.0, 56.0) < 13.0,
            "{}",
            width(&near, 52.0, 56.0)
        );
        // Flyttet konturen, flytter den brede delen seg.
        let (sk, id) = flat(0.8);
        let far = body_mesh(&sk, id, 1.0).unwrap();
        assert!(width(&far, 46.0, 50.0) > 18.0);
        assert!(width(&far, 4.0, 8.0) < 13.0);
    }
}
