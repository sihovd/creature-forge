//! Spline-skjelettet: senterlinjer som kubiske Bézier-kurver i et hierarki
//! der en barnespline er festet til et punkt på forelderens kurve (issue #4).
//!
//! En rotspline lagres i verdenskoordinater. En barnespline lagres i den
//! lokale rammen i festepunktet på forelderen. Flyttes eller bøyes
//! forelderen, følger barnet og dets undergrener med, mens redigering av
//! barnet aldri endrer forelderen. Festet er en andel av forelderens
//! buelengde, så det ligger på kurven også når forelderen får andre
//! ankerpunkter eller bøyes med låst lengde. Festet er knyttet til
//! skjelettet, ikke til mesh eller overflate, og tykkelse påvirker det ikke.

use crate::curve::{add, mat_vec, rotation_between, scale};
use crate::mesh::{cross, dot, length, normalize, sub, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type SplineId = u32;

/// Myke punkter har håndtak på linje; hjørnepunkter har uavhengige håndtak.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    Smooth,
    Corner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleSide {
    In,
    Out,
}

/// Ankerpunkt med håndtak for innkommende og utgående kurveretning.
/// Håndtakene lagres som forskyvninger fra punktet, så de følger når
/// punktet flyttes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub position: Vec3,
    pub handle_in: Vec3,
    pub handle_out: Vec3,
    pub kind: AnchorKind,
}

impl Anchor {
    pub fn new(position: Vec3, handle_in: Vec3, handle_out: Vec3, kind: AnchorKind) -> Anchor {
        Anchor {
            position,
            handle_in,
            handle_out,
            kind,
        }
    }

    /// Hjørnepunkt uten håndtak.
    pub fn corner(position: Vec3) -> Anchor {
        Anchor::new(position, [0.0; 3], [0.0; 3], AnchorKind::Corner)
    }
}

/// Festet til en barnespline: forelderen og en andel av dens buelengde.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    pub parent: SplineId,
    /// Andel av forelderens buelengde i [0, 1].
    pub at: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spline {
    /// Ankerpunktene. For en barnespline er de i festets lokale ramme
    /// (x langs U, y langs V, z langs tangenten), og første punkt er festet.
    pub anchors: Vec<Anchor>,
    pub attachment: Option<Attachment>,
    /// Referanseretning for rammen i starten av en rotspline.
    pub up: Vec3,
    /// Tykkelse i mm. Foreløpig radien til et sirkeltverrsnitt.
    pub thickness: f64,
}

/// Ortonormal ramme: tangent T og tverretningene U og V.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
    pub t: Vec3,
    pub u: Vec3,
    pub v: Vec3,
}

impl Frame {
    pub fn to_world(&self, local: Vec3) -> Vec3 {
        add(self.origin, self.dir_to_world(local))
    }

    pub fn dir_to_world(&self, local: Vec3) -> Vec3 {
        add(
            add(scale(self.u, local[0]), scale(self.v, local[1])),
            scale(self.t, local[2]),
        )
    }

    pub fn to_local(&self, world: Vec3) -> Vec3 {
        self.dir_to_local(sub(world, self.origin))
    }

    pub fn dir_to_local(&self, d: Vec3) -> Vec3 {
        [dot(d, self.u), dot(d, self.v), dot(d, self.t)]
    }
}

/// Kubisk Bézier-segment med kontrollpunktene P0..P3.
pub type Segment = [Vec3; 4];

pub fn bezier_point(s: &Segment, t: f64) -> Vec3 {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    (0..4).fold([0.0; 3], |acc, i| add(acc, scale(s[i], w[i])))
}

pub fn bezier_derivative(s: &Segment, t: f64) -> Vec3 {
    let u = 1.0 - t;
    let d = [sub(s[1], s[0]), sub(s[2], s[1]), sub(s[3], s[2])];
    let w = [3.0 * u * u, 6.0 * u * t, 3.0 * t * t];
    (0..3).fold([0.0; 3], |acc, i| add(acc, scale(d[i], w[i])))
}

/// Enhetstangenten, også der et håndtak har lengde null.
pub fn bezier_tangent(s: &Segment, t: f64) -> Option<Vec3> {
    let d = bezier_derivative(s, t);
    if length(d) > 1e-9 {
        return Some(normalize(d));
    }
    // Nulltangent i et endepunkt: retningen er grensen innover i segmentet.
    let inward = if t < 0.5 { t + 1e-4 } else { t - 1e-4 };
    let d = bezier_derivative(s, inward);
    if length(d) > 1e-12 {
        return Some(normalize(d));
    }
    let chord = sub(s[3], s[0]);
    (length(chord) > 1e-12).then(|| normalize(chord))
}

/// Buelengden fra t0 til t1 med 5-punkts Gauss–Legendre.
fn bezier_length(s: &Segment, t0: f64, t1: f64) -> f64 {
    const X: [f64; 5] = [
        0.0,
        -0.538_469_310_105_683,
        0.538_469_310_105_683,
        -0.906_179_845_938_664,
        0.906_179_845_938_664,
    ];
    const W: [f64; 5] = [
        0.568_888_888_888_889,
        0.478_628_670_499_366,
        0.478_628_670_499_366,
        0.236_926_885_056_189,
        0.236_926_885_056_189,
    ];
    let (m, h) = ((t0 + t1) / 2.0, (t1 - t0) / 2.0);
    (0..5)
        .map(|i| W[i] * length(bezier_derivative(s, m + h * X[i])))
        .sum::<f64>()
        * h
}

pub fn segments(anchors: &[Anchor]) -> Vec<Segment> {
    anchors
        .windows(2)
        .map(|w| {
            [
                w[0].position,
                add(w[0].position, w[0].handle_out),
                add(w[1].position, w[1].handle_in),
                w[1].position,
            ]
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    segment: usize,
    t: f64,
    /// Buelengden fra starten av splinen, i mm.
    s: f64,
    frame: Frame,
}

/// En spline evaluert i verdenskoordinater, med buelengde og
/// rotasjonsminimerende rammer (dobbel refleksjon, Wang mfl. 2008).
#[derive(Clone, Debug)]
pub struct WorldSpline {
    pub anchors: Vec<Anchor>,
    pub segments: Vec<Segment>,
    pub length: f64,
    samples: Vec<Sample>,
}

impl WorldSpline {
    pub const SAMPLES_PER_SEGMENT: usize = 64;

    /// `up` er en retning som bestemmer U i starten; den projiseres vinkelrett
    /// på starttangenten.
    pub fn new(anchors: Vec<Anchor>, up: Vec3) -> Result<WorldSpline, String> {
        if anchors.len() < 2 {
            return Err("En spline må ha minst to ankerpunkter".into());
        }
        let segments = segments(&anchors);
        let n = Self::SAMPLES_PER_SEGMENT;
        let mut samples: Vec<Sample> = Vec::with_capacity(segments.len() * (n + 1));
        let mut s_total = 0.0;
        for (k, seg) in segments.iter().enumerate() {
            let tangent0 = bezier_tangent(seg, 0.0);
            let Some(t0) = tangent0 else {
                return Err(format!("Segment {k} har lengde null"));
            };
            for i in 0..=n {
                let t = i as f64 / n as f64;
                if i > 0 {
                    s_total += bezier_length(seg, (i - 1) as f64 / n as f64, t);
                }
                let p = bezier_point(seg, t);
                let tan = bezier_tangent(seg, t).unwrap_or(t0);
                let frame = match samples.last() {
                    None => start_frame(p, tan, up),
                    Some(prev) => transport(&prev.frame, p, tan),
                };
                samples.push(Sample {
                    segment: k,
                    t,
                    s: s_total,
                    frame,
                });
            }
        }
        Ok(WorldSpline {
            anchors,
            segments,
            length: s_total,
            samples,
        })
    }

    /// Segment og parameter t for en andel `u` av buelengden.
    pub fn locate(&self, u: f64) -> (usize, f64) {
        let target = u.clamp(0.0, 1.0) * self.length;
        let k = self
            .samples
            .partition_point(|x| x.s < target)
            .clamp(1, self.samples.len() - 1);
        let (a, b) = (&self.samples[k - 1], &self.samples[k]);
        if a.segment != b.segment || b.s - a.s < 1e-12 {
            return (b.segment, b.t);
        }
        // Newtonsteg på buelengden innenfor intervallet.
        let seg = &self.segments[a.segment];
        let mut t = a.t + (b.t - a.t) * (target - a.s) / (b.s - a.s);
        for _ in 0..4 {
            let f = a.s + bezier_length(seg, a.t, t) - target;
            let d = length(bezier_derivative(seg, t));
            if d < 1e-12 {
                break;
            }
            t = (t - f / d).clamp(a.t, b.t);
        }
        (a.segment, t)
    }

    pub fn point_at(&self, u: f64) -> Vec3 {
        let (k, t) = self.locate(u);
        bezier_point(&self.segments[k], t)
    }

    /// Rammen i andelen `u` av buelengden.
    pub fn frame_at(&self, u: f64) -> Frame {
        let (k, t) = self.locate(u);
        let seg = &self.segments[k];
        let i = self
            .samples
            .iter()
            .rposition(|x| x.segment == k && x.t <= t)
            .unwrap_or(0);
        let base = self.samples[i].frame;
        let tan = bezier_tangent(seg, t).unwrap_or(base.t);
        let r = rotation_between(base.t, tan);
        let u_axis = normalize(mat_vec(&r, base.u));
        Frame {
            origin: bezier_point(seg, t),
            t: tan,
            u: u_axis,
            v: cross(tan, u_axis),
        }
    }

    pub fn start_frame(&self) -> Frame {
        self.samples[0].frame
    }
}

fn start_frame(origin: Vec3, t: Vec3, up: Vec3) -> Frame {
    let mut u = sub(up, scale(t, dot(up, t)));
    if length(u) < 1e-6 {
        u = if t[0].abs() < 0.9 {
            cross(t, [1.0, 0.0, 0.0])
        } else {
            cross(t, [0.0, 1.0, 0.0])
        };
    }
    let u = normalize(u);
    Frame {
        origin,
        t,
        u,
        v: cross(t, u),
    }
}

/// Fører rammen videre til neste prøve med dobbel refleksjon. Ved et
/// hjørne (samme punkt, ny tangent) roteres rammen minst mulig.
fn transport(prev: &Frame, p: Vec3, t: Vec3) -> Frame {
    let v1 = sub(p, prev.origin);
    let c1 = dot(v1, v1);
    let u = if c1 < 1e-20 {
        if dot(prev.t, t) < -1.0 + 1e-9 {
            // Full vending: roter om U, så U beholdes.
            prev.u
        } else {
            mat_vec(&rotation_between(prev.t, t), prev.u)
        }
    } else {
        let r_l = sub(prev.u, scale(v1, 2.0 / c1 * dot(v1, prev.u)));
        let t_l = sub(prev.t, scale(v1, 2.0 / c1 * dot(v1, prev.t)));
        let v2 = sub(t, t_l);
        let c2 = dot(v2, v2);
        if c2 < 1e-20 {
            r_l
        } else {
            sub(r_l, scale(v2, 2.0 / c2 * dot(v2, r_l)))
        }
    };
    // Fjern avrundingsfeil så rammen holdes ortonormal.
    let u = normalize(sub(u, scale(t, dot(u, t))));
    Frame {
        origin: p,
        t,
        u,
        v: cross(t, u),
    }
}

/// Et sammenkoblet spline-hierarki. Hele hierarkiet er ett formlag.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Skeleton {
    next_id: SplineId,
    splines: BTreeMap<SplineId, Spline>,
}

/// Filformatet for et skjelett. Inngår senere i prosjektfilen.
#[derive(Serialize, Deserialize)]
struct SkeletonFile {
    format: String,
    version: u32,
    skeleton: Skeleton,
}

const FORMAT: &str = "creature-forge-skeleton";
const VERSION: u32 = 1;

impl Skeleton {
    pub fn new() -> Skeleton {
        Skeleton {
            next_id: 1,
            splines: BTreeMap::new(),
        }
    }

    pub fn ids(&self) -> Vec<SplineId> {
        self.splines.keys().copied().collect()
    }

    pub fn spline(&self, id: SplineId) -> Option<&Spline> {
        self.splines.get(&id)
    }

    pub fn children(&self, id: SplineId) -> Vec<SplineId> {
        self.splines
            .iter()
            .filter(|(_, s)| s.attachment.is_some_and(|a| a.parent == id))
            .map(|(k, _)| *k)
            .collect()
    }

    /// Splinen og alle undergrener under den.
    pub fn descendants(&self, id: SplineId) -> Vec<SplineId> {
        let mut out = vec![id];
        let mut i = 0;
        while i < out.len() {
            out.extend(self.children(out[i]));
            i += 1;
        }
        out
    }

    pub fn add_root(
        &mut self,
        anchors: Vec<Anchor>,
        up: Vec3,
        thickness: f64,
    ) -> Result<SplineId, String> {
        let spline = Spline {
            anchors,
            attachment: None,
            up,
            thickness,
        };
        self.insert(spline)
    }

    /// Fester en ny spline i andelen `at` av forelderens buelengde. Punktene
    /// gis i verdenskoordinater; første punkt flyttes til festet.
    pub fn add_child(
        &mut self,
        parent: SplineId,
        at: f64,
        world_anchors: Vec<Anchor>,
        thickness: f64,
    ) -> Result<SplineId, String> {
        let attachment = Attachment { parent, at };
        let frame = self.attachment_frame(&attachment)?;
        let spline = Spline {
            anchors: to_local(&frame, &world_anchors),
            attachment: Some(attachment),
            up: [0.0, 0.0, 1.0],
            thickness,
        };
        self.insert(spline)
    }

    fn insert(&mut self, spline: Spline) -> Result<SplineId, String> {
        let id = self.next_id.max(1);
        let mut next = self.clone();
        next.splines.insert(id, spline);
        next.next_id = id + 1;
        next.validate()?;
        *self = next;
        Ok(id)
    }

    /// Fjerner splinen og alle undergrener.
    pub fn remove(&mut self, id: SplineId) -> Result<(), String> {
        if !self.splines.contains_key(&id) {
            return Err("Splinen finnes ikke".into());
        }
        for d in self.descendants(id) {
            self.splines.remove(&d);
        }
        Ok(())
    }

    fn attachment_frame(&self, a: &Attachment) -> Result<Frame, String> {
        if !(0.0..=1.0).contains(&a.at) {
            return Err(format!("Festet må ligge i [0, 1], fikk {}", a.at));
        }
        Ok(self.world_spline(a.parent)?.frame_at(a.at))
    }

    /// Splinen i verdenskoordinater. Forfedrene evalueres først.
    pub fn world_spline(&self, id: SplineId) -> Result<WorldSpline, String> {
        let spline = self.splines.get(&id).ok_or("Splinen finnes ikke")?;
        match spline.attachment {
            None => WorldSpline::new(spline.anchors.clone(), spline.up),
            Some(a) => {
                let frame = self.attachment_frame(&a)?;
                WorldSpline::new(to_world(&frame, &spline.anchors), frame.u)
            }
        }
    }

    /// Alle spliner i verdenskoordinater.
    pub fn evaluate(&self) -> Result<BTreeMap<SplineId, WorldSpline>, String> {
        let mut out: BTreeMap<SplineId, WorldSpline> = BTreeMap::new();
        for root in self.splines.iter().filter(|(_, s)| s.attachment.is_none()) {
            for id in self.descendants(*root.0) {
                let s = &self.splines[&id];
                let world = match s.attachment {
                    None => WorldSpline::new(s.anchors.clone(), s.up)?,
                    Some(a) => {
                        let frame = out[&a.parent].frame_at(a.at);
                        WorldSpline::new(to_world(&frame, &s.anchors), frame.u)?
                    }
                };
                out.insert(id, world);
            }
        }
        Ok(out)
    }

    /// Erstatter ankerpunktene med punkter gitt i verdenskoordinater.
    /// For en barnespline beholdes festet; første punkt flyttes dit.
    pub fn set_world_anchors(
        &mut self,
        id: SplineId,
        world_anchors: Vec<Anchor>,
    ) -> Result<(), String> {
        let spline = self.splines.get(&id).ok_or("Splinen finnes ikke")?;
        let anchors = match spline.attachment {
            None => world_anchors,
            Some(a) => to_local(&self.attachment_frame(&a)?, &world_anchors),
        };
        self.update(id, |s| s.anchors = anchors)
    }

    pub fn world_anchors(&self, id: SplineId) -> Result<Vec<Anchor>, String> {
        Ok(self.world_spline(id)?.anchors)
    }

    /// Flytter et ankerpunkt med håndtakene. Første punkt på en barnespline
    /// er festet og flyttes med `reattach`.
    pub fn move_anchor(
        &mut self,
        id: SplineId,
        index: usize,
        world_pos: Vec3,
    ) -> Result<(), String> {
        let mut anchors = self.world_anchors(id)?;
        let attached = self.splines[&id].attachment.is_some();
        if attached && index == 0 {
            return Err("Festepunktet flyttes langs forelderen, ikke fritt".into());
        }
        anchors
            .get_mut(index)
            .ok_or("Ankerpunktet finnes ikke")?
            .position = world_pos;
        self.set_world_anchors(id, anchors)
    }

    /// Setter et håndtak til et punkt i verdenskoordinater. Et mykt punkt
    /// retter det motsatte håndtaket inn på linje og beholder dets lengde.
    pub fn set_handle(
        &mut self,
        id: SplineId,
        index: usize,
        side: HandleSide,
        world_handle: Vec3,
    ) -> Result<(), String> {
        let mut anchors = self.world_anchors(id)?;
        let a = anchors.get_mut(index).ok_or("Ankerpunktet finnes ikke")?;
        let offset = sub(world_handle, a.position);
        let (this, other) = match side {
            HandleSide::In => (&mut a.handle_in, &mut a.handle_out),
            HandleSide::Out => (&mut a.handle_out, &mut a.handle_in),
        };
        *this = offset;
        if a.kind == AnchorKind::Smooth && length(offset) > 1e-12 {
            *other = scale(normalize(offset), -length(*other));
        }
        self.set_world_anchors(id, anchors)
    }

    /// Bytter mellom mykt punkt og hjørnepunkt. Et mykt punkt får det
    /// innkommende håndtaket på linje med det utgående.
    pub fn set_anchor_kind(
        &mut self,
        id: SplineId,
        index: usize,
        kind: AnchorKind,
    ) -> Result<(), String> {
        let spline = self.splines.get(&id).ok_or("Splinen finnes ikke")?;
        let a = *spline
            .anchors
            .get(index)
            .ok_or("Ankerpunktet finnes ikke")?;
        let mut a2 = Anchor { kind, ..a };
        if kind == AnchorKind::Smooth {
            let (lin, lout) = (length(a.handle_in), length(a.handle_out));
            if lout > 1e-12 {
                a2.handle_in = scale(normalize(a.handle_out), -lin);
            } else if lin > 1e-12 {
                a2.handle_out = scale(normalize(a.handle_in), -lout);
            }
        }
        // Håndtakene er retninger, så endringen kan gjøres i lokale koordinater.
        self.update(id, |s| s.anchors[index] = a2)
    }

    /// Flytter en rotspline med alle undergrener.
    pub fn translate(&mut self, id: SplineId, delta: Vec3) -> Result<(), String> {
        let spline = self.splines.get(&id).ok_or("Splinen finnes ikke")?;
        if spline.attachment.is_some() {
            return Err("En barnespline flyttes ved å flytte festet langs forelderen".into());
        }
        self.update(id, |s| {
            for a in s.anchors.iter_mut() {
                a.position = add(a.position, delta);
            }
        })
    }

    /// Flytter festet langs forelderen. Barnets form i festets ramme beholdes.
    pub fn reattach(&mut self, id: SplineId, at: f64) -> Result<(), String> {
        let spline = self.splines.get(&id).ok_or("Splinen finnes ikke")?;
        let Some(a) = spline.attachment else {
            return Err("Splinen er ikke festet til en forelder".into());
        };
        self.update(id, |s| s.attachment = Some(Attachment { at, ..a }))
    }

    /// Tykkelsen endrer ikke skjelettet eller festene.
    pub fn set_thickness(&mut self, id: SplineId, thickness: f64) -> Result<(), String> {
        self.update(id, |s| s.thickness = thickness)
    }

    /// Endrer en spline og godtar endringen bare hvis hele skjelettet er gyldig.
    fn update(&mut self, id: SplineId, f: impl FnOnce(&mut Spline)) -> Result<(), String> {
        let mut next = self.clone();
        f(next.splines.get_mut(&id).ok_or("Splinen finnes ikke")?);
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        let finite = |v: Vec3| v.iter().all(|x| x.is_finite());
        for (id, s) in &self.splines {
            if s.anchors.len() < 2 {
                return Err(format!("Spline {id} har færre enn to ankerpunkter"));
            }
            if s.anchors
                .iter()
                .any(|a| !finite(a.position) || !finite(a.handle_in) || !finite(a.handle_out))
                || !finite(s.up)
            {
                return Err(format!("Spline {id} har ugyldige tall"));
            }
            if !(s.thickness.is_finite() && s.thickness >= 0.0) {
                return Err(format!("Spline {id} har ugyldig tykkelse"));
            }
            if let Some(a) = s.attachment {
                if !self.splines.contains_key(&a.parent) {
                    return Err(format!(
                        "Spline {id} er festet til en spline som ikke finnes"
                    ));
                }
                if !(0.0..=1.0).contains(&a.at) {
                    return Err(format!("Spline {id} har feste utenfor forelderen"));
                }
                if length(s.anchors[0].position) > 1e-9 {
                    return Err(format!("Første punkt i spline {id} ligger ikke i festet"));
                }
                // Kjeden av foreldre må ende i en rot.
                let mut cur = a.parent;
                for _ in 0..=self.splines.len() {
                    match self.splines.get(&cur).and_then(|s| s.attachment) {
                        Some(p) if p.parent == *id => {
                            return Err(format!("Spline {id} er festet i en sløyfe"))
                        }
                        Some(p) => cur = p.parent,
                        None => break,
                    }
                }
            }
            if *id >= self.next_id {
                return Err("Ugyldig id i skjelettet".into());
            }
        }
        self.evaluate().map(|_| ())
    }

    pub fn to_json(&self) -> String {
        let file = SkeletonFile {
            format: FORMAT.into(),
            version: VERSION,
            skeleton: self.clone(),
        };
        serde_json::to_string_pretty(&file).expect("skjelettet kan alltid serialiseres")
    }

    pub fn from_json(text: &str) -> Result<Skeleton, String> {
        let file: SkeletonFile =
            serde_json::from_str(text).map_err(|e| format!("Kunne ikke lese skjelettet: {e}"))?;
        if file.format != FORMAT {
            return Err(format!("Ukjent format: {}", file.format));
        }
        if file.version != VERSION {
            return Err(format!("Ukjent skjelettversjon: {}", file.version));
        }
        file.skeleton.validate()?;
        Ok(file.skeleton)
    }
}

fn to_local(frame: &Frame, world: &[Anchor]) -> Vec<Anchor> {
    world
        .iter()
        .enumerate()
        .map(|(i, a)| Anchor {
            // Første punkt i en barnespline er festet.
            position: if i == 0 {
                [0.0; 3]
            } else {
                frame.to_local(a.position)
            },
            handle_in: frame.dir_to_local(a.handle_in),
            handle_out: frame.dir_to_local(a.handle_out),
            kind: a.kind,
        })
        .collect()
}

fn to_world(frame: &Frame, local: &[Anchor]) -> Vec<Anchor> {
    local
        .iter()
        .map(|a| Anchor {
            position: frame.to_world(a.position),
            handle_in: frame.dir_to_world(a.handle_in),
            handle_out: frame.dir_to_world(a.handle_out),
            kind: a.kind,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bend::{bent_anchors, Bend};
    use std::f64::consts::PI;

    fn close(a: Vec3, b: Vec3, eps: f64) -> bool {
        length(sub(a, b)) < eps
    }

    /// Rett spline langs `dir` fra `start` med lengde `len`, delt i to segmenter.
    fn straight(start: Vec3, dir: Vec3, len: f64) -> Vec<Anchor> {
        let d = normalize(dir);
        let h = scale(d, len / 6.0);
        let mid = add(start, scale(d, len / 2.0));
        vec![
            Anchor::new(start, [0.0; 3], h, AnchorKind::Smooth),
            Anchor::new(mid, scale(h, -1.0), h, AnchorKind::Smooth),
            Anchor::new(
                add(start, scale(d, len)),
                scale(h, -1.0),
                [0.0; 3],
                AnchorKind::Smooth,
            ),
        ]
    }

    /// Rot langs z, barn ut langs x fra midten, barnebarn ut langs y fra barnets ende.
    fn three_levels() -> (Skeleton, SplineId, SplineId, SplineId) {
        let mut sk = Skeleton::new();
        let root = sk
            .add_root(
                straight([0.0; 3], [0.0, 0.0, 1.0], 100.0),
                [1.0, 0.0, 0.0],
                8.0,
            )
            .unwrap();
        let child = sk
            .add_child(
                root,
                0.5,
                straight([0.0, 0.0, 50.0], [1.0, 0.0, 0.0], 40.0),
                5.0,
            )
            .unwrap();
        let grandchild = sk
            .add_child(
                child,
                1.0,
                straight([40.0, 0.0, 50.0], [0.0, 1.0, 0.0], 20.0),
                3.0,
            )
            .unwrap();
        (sk, root, child, grandchild)
    }

    fn world_points(sk: &Skeleton, id: SplineId) -> Vec<Vec3> {
        sk.world_anchors(id)
            .unwrap()
            .iter()
            .map(|a| a.position)
            .collect()
    }

    #[test]
    fn barn_starter_i_festet_og_beholder_formen() {
        let (sk, root, child, gc) = three_levels();
        let world = sk.evaluate().unwrap();
        assert!(close(
            world[&child].point_at(0.0),
            world[&root].point_at(0.5),
            1e-9
        ));
        assert!(close(
            world[&gc].point_at(0.0),
            world[&child].point_at(1.0),
            1e-9
        ));
        assert!(close(world[&child].point_at(1.0), [40.0, 0.0, 50.0], 1e-6));
        assert!(close(world[&gc].point_at(1.0), [40.0, 20.0, 50.0], 1e-6));
        assert!((world[&child].length - 40.0).abs() < 1e-6);
    }

    #[test]
    fn flytting_av_forelder_flytter_alle_undergrener() {
        let (mut sk, root, child, gc) = three_levels();
        let (c0, g0) = (world_points(&sk, child), world_points(&sk, gc));
        let delta = [5.0, -3.0, 12.0];
        sk.translate(root, delta).unwrap();
        for (a, b) in c0.iter().zip(world_points(&sk, child)) {
            assert!(close(add(*a, delta), b, 1e-9));
        }
        for (a, b) in g0.iter().zip(world_points(&sk, gc)) {
            assert!(close(add(*a, delta), b, 1e-9));
        }
        // Et barn kan ikke flyttes fritt bort fra festet.
        assert!(sk.translate(child, delta).is_err());
    }

    #[test]
    fn boying_av_forelder_tar_med_barna_og_bevarer_formen_deres() {
        let (mut sk, root, child, gc) = three_levels();
        let before = sk.evaluate().unwrap();
        let bend = Bend {
            direction: [1.0, 0.0, 0.0],
            angle: PI / 2.0,
            radius: 30.0,
            pivot: 0.3,
        };
        sk.set_world_anchors(
            root,
            bent_anchors([0.0; 3], [0.0, 0.0, 1.0], 100.0, &bend).unwrap(),
        )
        .unwrap();
        let after = sk.evaluate().unwrap();
        // Forelderens lengde er låst, og barna sitter på samme sted langs kurven.
        assert!((after[&root].length - 100.0).abs() < 1e-3);
        assert!(close(
            after[&child].point_at(0.0),
            after[&root].point_at(0.5),
            1e-9
        ));
        assert!(close(
            after[&gc].point_at(0.0),
            after[&child].point_at(1.0),
            1e-9
        ));
        // Barnet er stivt med i festets ramme: lengder og vinkel mot forelderen beholdes.
        for id in [child, gc] {
            assert!((after[&id].length - before[&id].length).abs() < 1e-9);
        }
        let angle = |w: &BTreeMap<SplineId, WorldSpline>| {
            dot(w[&root].frame_at(0.5).t, w[&child].frame_at(0.0).t)
        };
        assert!((angle(&before) - angle(&after)).abs() < 1e-9);
        // Og barnet har faktisk flyttet seg.
        assert!(!close(
            before[&gc].point_at(1.0),
            after[&gc].point_at(1.0),
            1.0
        ));
    }

    #[test]
    fn redigering_av_barn_flytter_ikke_forelderen() {
        let (mut sk, root, child, gc) = three_levels();
        let parent_before = sk.world_anchors(root).unwrap();
        sk.move_anchor(child, 2, [45.0, 10.0, 60.0]).unwrap();
        sk.set_handle(child, 1, HandleSide::Out, [30.0, 15.0, 55.0])
            .unwrap();
        sk.set_anchor_kind(child, 1, AnchorKind::Corner).unwrap();
        sk.reattach(child, 0.8).unwrap();
        assert_eq!(sk.world_anchors(root).unwrap(), parent_before);
        // Barnebarnet følger barnets nye ende.
        let w = sk.evaluate().unwrap();
        assert!(close(w[&gc].point_at(0.0), w[&child].point_at(1.0), 1e-9));
        assert!(close(w[&child].point_at(0.0), w[&root].point_at(0.8), 1e-9));
        // Festepunktet kan ikke dras fritt.
        assert!(sk.move_anchor(child, 0, [1.0, 2.0, 3.0]).is_err());
    }

    #[test]
    fn tykkelse_endrer_ikke_festene() {
        let (mut sk, root, child, gc) = three_levels();
        let before = sk.evaluate().unwrap();
        sk.set_thickness(root, 20.0).unwrap();
        sk.set_thickness(child, 0.5).unwrap();
        let after = sk.evaluate().unwrap();
        for id in [root, child, gc] {
            assert_eq!(before[&id].anchors, after[&id].anchors);
        }
        assert!(sk.set_thickness(root, -1.0).is_err());
    }

    #[test]
    fn bezier_haandtak_paa_forelder_beholder_hierarkiet() {
        let (mut sk, root, child, gc) = three_levels();
        // Mykt punkt: det motsatte håndtaket rettes inn og beholder lengden.
        sk.set_handle(root, 1, HandleSide::Out, [20.0, 0.0, 60.0])
            .unwrap();
        let a = sk.world_anchors(root).unwrap()[1];
        assert!(close(
            normalize(a.handle_in),
            scale(normalize(a.handle_out), -1.0),
            1e-9
        ));
        assert!((length(a.handle_in) - 100.0 / 6.0).abs() < 1e-9);
        // Hjørnepunkt: håndtakene er uavhengige.
        sk.set_anchor_kind(root, 1, AnchorKind::Corner).unwrap();
        sk.set_handle(root, 1, HandleSide::In, [-10.0, 5.0, 40.0])
            .unwrap();
        let b = sk.world_anchors(root).unwrap()[1];
        assert_eq!(b.handle_out, a.handle_out);
        let w = sk.evaluate().unwrap();
        assert!(close(w[&child].point_at(0.0), w[&root].point_at(0.5), 1e-9));
        assert!(close(w[&gc].point_at(0.0), w[&child].point_at(1.0), 1e-9));
    }

    #[test]
    fn rammene_er_ortonormale_ogsaa_gjennom_en_skarp_knekk() {
        let mut sk = Skeleton::new();
        let bend = Bend {
            direction: [0.0, 1.0, 0.0],
            angle: PI,
            radius: 0.0,
            pivot: 0.5,
        };
        let root = sk
            .add_root(
                bent_anchors([0.0; 3], [0.0, 0.0, 1.0], 60.0, &bend).unwrap(),
                [1.0, 0.0, 0.0],
                4.0,
            )
            .unwrap();
        let child = sk
            .add_child(root, 0.9, straight([0.0; 3], [1.0, 0.0, 0.0], 10.0), 2.0)
            .unwrap();
        let w = sk.evaluate().unwrap();
        for i in 0..=50 {
            let f = w[&root].frame_at(i as f64 / 50.0);
            assert!((length(f.t) - 1.0).abs() < 1e-9);
            assert!((length(f.u) - 1.0).abs() < 1e-9);
            assert!(dot(f.t, f.u).abs() < 1e-9);
        }
        assert!(close(w[&child].point_at(0.0), w[&root].point_at(0.9), 1e-9));
    }

    #[test]
    fn lagring_og_aapning_bevarer_hierarki_kurver_og_fester() {
        let (mut sk, root, child, _) = three_levels();
        sk.set_anchor_kind(child, 1, AnchorKind::Corner).unwrap();
        sk.set_handle(root, 1, HandleSide::Out, [10.0, 3.0, 60.0])
            .unwrap();
        let text = sk.to_json();
        let back = Skeleton::from_json(&text).unwrap();
        assert_eq!(back, sk);
        let (a, b) = (sk.evaluate().unwrap(), back.evaluate().unwrap());
        for id in sk.ids() {
            assert_eq!(a[&id].anchors, b[&id].anchors);
        }
        // Neste id fortsetter etter de lagrede.
        let mut back = back;
        let new = back.add_root(
            straight([0.0; 3], [1.0, 0.0, 0.0], 5.0),
            [0.0, 0.0, 1.0],
            1.0,
        );
        assert!(!sk.ids().contains(&new.unwrap()));
    }

    #[test]
    fn ugyldige_filer_avvises() {
        let (sk, _, child, _) = three_levels();
        let text = sk.to_json();
        assert!(Skeleton::from_json(&text.replace(FORMAT, "noe-annet")).is_err());
        // Sløyfe: roten festes til sitt eget barn.
        let mut bad = sk.clone();
        bad.splines.get_mut(&1).unwrap().attachment = Some(Attachment {
            parent: child,
            at: 0.5,
        });
        assert!(Skeleton::from_json(&bad.to_json()).is_err());
        // Manglende forelder.
        let mut bad = sk.clone();
        bad.splines.remove(&1);
        assert!(Skeleton::from_json(&bad.to_json()).is_err());
    }

    #[test]
    fn fjerning_tar_med_undergrener() {
        let (mut sk, root, child, gc) = three_levels();
        sk.remove(child).unwrap();
        assert_eq!(sk.ids(), vec![root]);
        assert!(sk.spline(gc).is_none());
    }
}
