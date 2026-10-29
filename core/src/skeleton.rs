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
use crate::profile::{MasterId, Profile, ProfileShape, ResolvedStation, Station};
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
    /// Tykkelse i mm: radien til et sirkeltverrsnitt, og målestokken for
    /// profilene.
    pub thickness: f64,
    /// Tverrsnittsprofiler sortert etter plass langs kurven. Tom: sirkel.
    #[serde(default)]
    pub profiles: Vec<Station>,
    /// Flat ende med hele endeprofilen i stedet for en avrundet ende.
    #[serde(default)]
    pub flat_end: bool,
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

    /// Ankerpunktene for delen fra starten til andelen `u` av buelengden.
    /// Segmentet der `u` ligger, deles med de Casteljau, så formen er uendret.
    pub fn head_until(&self, u: f64) -> Vec<Anchor> {
        let (k, t) = self.locate(u);
        if t > 1.0 - 1e-9 {
            // Delingen ligger i ankerpunktet etter segmentet.
            let mut out = self.anchors[..=k + 1].to_vec();
            out.last_mut().unwrap().handle_out = [0.0; 3];
            return out;
        }
        let mut out: Vec<Anchor> = self.anchors[..=k].to_vec();
        let [p0, p1, p2, p3] = self.segments[k];
        let lerp = |a: Vec3, b: Vec3| add(a, scale(sub(b, a), t));
        let (q0, q1, q2) = (lerp(p0, p1), lerp(p1, p2), lerp(p2, p3));
        let (r0, r1) = (lerp(q0, q1), lerp(q1, q2));
        let end = lerp(r0, r1);
        let last = out.last_mut().unwrap();
        last.handle_out = sub(q0, p0);
        if t < 1e-12 {
            // Delingen ligger i et ankerpunkt.
            last.handle_out = [0.0; 3];
            return out;
        }
        out.push(Anchor::new(end, sub(r0, end), [0.0; 3], AnchorKind::Smooth));
        out
    }

    /// Andelen av buelengden der ankerpunkt `k` ligger.
    pub fn anchor_fraction(&self, k: usize) -> f64 {
        if k == 0 || self.length <= 0.0 {
            return 0.0;
        }
        let s = self
            .samples
            .iter()
            .rev()
            .find(|x| x.segment + 1 == k)
            .map_or(self.length, |x| x.s);
        (s / self.length).clamp(0.0, 1.0)
    }

    /// Alle ankerpunktene med et nytt punkt lagt inn i andelen `u` av
    /// buelengden, og plassen til det nye punktet. Formen er uendret.
    pub fn with_anchor_at(&self, u: f64) -> (Vec<Anchor>, usize) {
        let (k, t) = self.locate(u);
        if t < 1e-9 {
            return (self.anchors.clone(), k);
        }
        if t > 1.0 - 1e-9 {
            return (self.anchors.clone(), k + 1);
        }
        let [p0, p1, p2, p3] = self.segments[k];
        let lerp = |a: Vec3, b: Vec3| add(a, scale(sub(b, a), t));
        let (q0, q1, q2) = (lerp(p0, p1), lerp(p1, p2), lerp(p2, p3));
        let (r0, r1) = (lerp(q0, q1), lerp(q1, q2));
        let mid = lerp(r0, r1);
        let mut out = self.anchors.clone();
        out[k].handle_out = sub(q0, p0);
        out[k + 1].handle_in = sub(q2, p3);
        out.insert(
            k + 1,
            Anchor::new(mid, sub(r0, mid), sub(r1, mid), AnchorKind::Smooth),
        );
        (out, k + 1)
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

/// En redigering av skjelettet slik grensesnittet sender den. Et drag kan
/// bestå av flere redigeringer etter hverandre (`Batch`) og er likevel ett
/// angresteg.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SkeletonEdit {
    /// Ny rett rotspline fra `start` i retningen `direction`.
    AddRoot {
        start: Vec3,
        direction: Vec3,
        length: f64,
        thickness: f64,
    },
    /// Ny rett gren fra andelen `at` av forelderen.
    AddChild {
        parent: SplineId,
        at: f64,
        direction: Vec3,
        length: f64,
        thickness: f64,
    },
    MoveAnchor {
        spline: SplineId,
        index: usize,
        position: Vec3,
    },
    /// Flytter siste punkt, uansett hvor mange punkter splinen har fått.
    MoveTip {
        spline: SplineId,
        position: Vec3,
    },
    SetHandle {
        spline: SplineId,
        index: usize,
        side: Side,
        position: Vec3,
    },
    SetKind {
        spline: SplineId,
        index: usize,
        anchor_kind: AnchorKind,
    },
    /// Legger inn et punkt i andelen `at` uten å endre formen.
    InsertAnchor {
        spline: SplineId,
        at: f64,
    },
    /// Forlenger (eller forkorter) siste del langs endens retning, i mm.
    Extend {
        spline: SplineId,
        delta: f64,
    },
    /// Bøyer resten av splinen fra ankerpunkt `from` med låst lengde.
    Bend {
        spline: SplineId,
        from: usize,
        direction: Vec3,
        angle: f64,
        radius: f64,
    },
    Reattach {
        spline: SplineId,
        at: f64,
    },
    /// Flytter en rotspline med alle grener.
    Translate {
        spline: SplineId,
        delta: Vec3,
    },
    SetThickness {
        spline: SplineId,
        thickness: f64,
    },
    Remove {
        spline: SplineId,
    },
    /// Ny profilstasjon i andelen `at`. Med `from` limes en kopi av den
    /// stasjonen inn (omriss, størrelse, rotasjon og masterkobling); ellers
    /// en kopi av profilen slik den er der i dag.
    AddProfile {
        spline: SplineId,
        at: f64,
        from: Option<usize>,
    },
    /// Nytt eget omriss for en stasjon. En koblet stasjon blir individuell.
    SetProfile {
        spline: SplineId,
        station: usize,
        profile: Profile,
    },
    /// Plass, størrelse og rotasjon for en stasjon.
    SetStation {
        spline: SplineId,
        station: usize,
        at: f64,
        scale: f64,
        rotation: f64,
    },
    RemoveProfile {
        spline: SplineId,
        station: usize,
    },
    /// Gjør omrisset til stasjonen til en ny master og kobler stasjonen til den.
    MakeMaster {
        spline: SplineId,
        station: usize,
    },
    LinkMaster {
        spline: SplineId,
        station: usize,
        master: MasterId,
    },
    /// Kobler fra masteren og beholder gjeldende omriss.
    Unlink {
        spline: SplineId,
        station: usize,
    },
    /// Endrer masterens omriss; alle koblede stasjoner følger.
    SetMaster {
        master: MasterId,
        profile: Profile,
    },
    SetFlatEnd {
        spline: SplineId,
        flat: bool,
    },
    Batch {
        edits: Vec<SkeletonEdit>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    In,
    Out,
}

/// Rett spline med to myke ankerpunkter.
fn straight_anchors(start: Vec3, direction: Vec3, length: f64) -> Result<Vec<Anchor>, String> {
    let d = normalize(direction);
    if !(length.is_finite() && length > 0.0) || (crate::mesh::length(d) - 1.0).abs() > 1e-9 {
        return Err("En ny spline trenger retning og lengde".into());
    }
    let h = scale(d, length / 3.0);
    Ok(vec![
        Anchor::new(start, [0.0; 3], h, AnchorKind::Smooth),
        Anchor::new(
            add(start, scale(d, length)),
            scale(h, -1.0),
            [0.0; 3],
            AnchorKind::Smooth,
        ),
    ])
}

impl Skeleton {
    /// Utfører en redigering. Ved feil er skjelettet uendret. Returnerer id-en
    /// til en ny spline når redigeringen lager en.
    pub fn apply(&mut self, edit: &SkeletonEdit) -> Result<Option<SplineId>, String> {
        let mut next = self.clone();
        let created = next.apply_inner(edit)?;
        *self = next;
        Ok(created)
    }

    fn apply_inner(&mut self, edit: &SkeletonEdit) -> Result<Option<SplineId>, String> {
        use SkeletonEdit as E;
        match *edit {
            E::AddRoot {
                start,
                direction,
                length,
                thickness,
            } => {
                let anchors = straight_anchors(start, direction, length)?;
                // Rammens U står vinkelrett på retningen; velg den mest stabile aksen.
                let up = if direction[1].abs() < 0.9 * crate::mesh::length(direction) {
                    [0.0, 1.0, 0.0]
                } else {
                    [0.0, 0.0, 1.0]
                };
                return self.add_root(anchors, up, thickness).map(Some);
            }
            E::AddChild {
                parent,
                at,
                direction,
                length,
                thickness,
            } => {
                let start = self.world_spline(parent)?.point_at(at);
                let anchors = straight_anchors(start, direction, length)?;
                return self.add_child(parent, at, anchors, thickness).map(Some);
            }
            E::MoveAnchor {
                spline,
                index,
                position,
            } => self.move_anchor(spline, index, position)?,
            E::MoveTip { spline, position } => {
                let last = self
                    .spline(spline)
                    .ok_or("Splinen finnes ikke")?
                    .anchors
                    .len()
                    - 1;
                self.move_anchor(spline, last, position)?
            }
            E::SetHandle {
                spline,
                index,
                side,
                position,
            } => {
                let side = match side {
                    Side::In => HandleSide::In,
                    Side::Out => HandleSide::Out,
                };
                self.set_handle(spline, index, side, position)?
            }
            E::SetKind {
                spline,
                index,
                anchor_kind,
            } => self.set_anchor_kind(spline, index, anchor_kind)?,
            E::InsertAnchor { spline, at } => {
                let (anchors, _) = self.world_spline(spline)?.with_anchor_at(at);
                self.set_world_anchors(spline, anchors)?
            }
            E::Extend { spline, delta } => {
                let world = self.world_spline(spline)?;
                let last = world.segments.last().unwrap();
                let t = bezier_tangent(last, 1.0).ok_or("Enden mangler retning")?;
                let mut anchors = world.anchors;
                let end = anchors.last_mut().unwrap();
                end.position = add(end.position, scale(t, delta));
                self.set_world_anchors(spline, anchors)?
            }
            E::Bend {
                spline,
                from,
                direction,
                angle,
                radius,
            } => {
                let world = self.world_spline(spline)?;
                if from + 1 >= world.anchors.len() {
                    return Err("Bøyen må starte før siste punkt".into());
                }
                let bend = crate::bend::Bend::at_middle(direction, angle, radius);
                let anchors = crate::bend::bend_from(&world, world.anchor_fraction(from), &bend)?;
                self.set_world_anchors(spline, anchors)?
            }
            E::Reattach { spline, at } => self.reattach(spline, at)?,
            E::Translate { spline, delta } => self.translate(spline, delta)?,
            E::SetThickness { spline, thickness } => self.set_thickness(spline, thickness)?,
            E::Remove { spline } => self.remove(spline)?,
            E::AddProfile { spline, at, from } => {
                if !(0.0..=1.0).contains(&at) {
                    return Err("Profilen må ligge på splinen".into());
                }
                let station = match from {
                    Some(i) => Station {
                        at,
                        ..self.station(spline, i)?.clone()
                    },
                    None => {
                        let current = self.resolved_profiles(spline)?;
                        let near = current
                            .iter()
                            .min_by(|a, b| (a.at - at).abs().total_cmp(&(b.at - at).abs()));
                        match near {
                            Some(s) => Station {
                                at,
                                shape: ProfileShape::Own {
                                    profile: s.profile.clone(),
                                },
                                scale: s.scale,
                                rotation: s.rotation,
                            },
                            None => Station::new(at, Profile::circle()),
                        }
                    }
                };
                self.update(spline, |s| {
                    s.profiles.push(station);
                    s.profiles.sort_by(|a, b| a.at.total_cmp(&b.at));
                })?
            }
            E::SetProfile {
                spline,
                station,
                ref profile,
            } => {
                self.station(spline, station)?;
                let profile = profile.clone();
                self.update(spline, |s| {
                    s.profiles[station].shape = ProfileShape::Own { profile }
                })?
            }
            E::SetStation {
                spline,
                station,
                at,
                scale,
                rotation,
            } => {
                self.station(spline, station)?;
                self.update(spline, |s| {
                    let st = &mut s.profiles[station];
                    st.at = at;
                    st.scale = scale;
                    st.rotation = rotation;
                    s.profiles.sort_by(|a, b| a.at.total_cmp(&b.at));
                })?
            }
            E::RemoveProfile { spline, station } => {
                self.station(spline, station)?;
                self.update(spline, |s| {
                    s.profiles.remove(station);
                })?
            }
            E::MakeMaster { spline, station } => {
                let profile = self.station_profile(self.station(spline, station)?)?;
                let master = self.masters.keys().next_back().map_or(1, |k| k + 1);
                self.masters.insert(master, profile);
                self.update(spline, |s| {
                    s.profiles[station].shape = ProfileShape::Master { master }
                })?
            }
            E::LinkMaster {
                spline,
                station,
                master,
            } => {
                self.station(spline, station)?;
                if !self.masters.contains_key(&master) {
                    return Err("Masteren finnes ikke".into());
                }
                self.update(spline, |s| {
                    s.profiles[station].shape = ProfileShape::Master { master }
                })?
            }
            E::Unlink { spline, station } => {
                let profile = self.station_profile(self.station(spline, station)?)?;
                self.update(spline, |s| {
                    s.profiles[station].shape = ProfileShape::Own { profile }
                })?
            }
            E::SetMaster {
                master,
                ref profile,
            } => {
                if !self.masters.contains_key(&master) {
                    return Err("Masteren finnes ikke".into());
                }
                profile.validate()?;
                self.masters.insert(master, profile.clone());
            }
            E::SetFlatEnd { spline, flat } => self.update(spline, |s| s.flat_end = flat)?,
            E::Batch { ref edits } => {
                let mut created = None;
                for e in edits {
                    created = self.apply_inner(e)?.or(created);
                }
                return Ok(created);
            }
        }
        Ok(None)
    }
}

/// Skjelettet slik visningen trenger det, i verdenskoordinater.
#[derive(Clone, Debug, Serialize)]
pub struct SplineView {
    pub id: SplineId,
    pub parent: Option<SplineId>,
    pub at: f64,
    pub thickness: f64,
    pub length: f64,
    /// Punkter langs kurven for tegning.
    pub points: Vec<Vec3>,
    pub anchors: Vec<AnchorView>,
    pub profiles: Vec<StationView>,
    pub flat_end: bool,
}

/// En profilstasjon slik visningen og profileditoren trenger den.
#[derive(Clone, Debug, Serialize)]
pub struct StationView {
    pub at: f64,
    pub scale: f64,
    pub rotation: f64,
    pub master: Option<MasterId>,
    /// Omrisset i enheter av tykkelsen, uten størrelse og rotasjon.
    pub profile: Profile,
    /// Omrisset i verdenskoordinater, for tegning rundt kurven.
    pub ring: Vec<Vec3>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AnchorView {
    pub position: Vec3,
    /// Håndtakenes endepunkter (ikke forskyvninger).
    pub handle_in: Vec3,
    pub handle_out: Vec3,
    pub kind: AnchorKind,
    /// Andel av buelengden der punktet ligger.
    pub at: f64,
    /// Kurvens retning ut fra punktet (inn mot punktet for siste punkt).
    pub tangent: Vec3,
}

/// Punkter per segment i tegningen av en spline.
pub const VIEW_POINTS_PER_SEGMENT: usize = 24;

impl Skeleton {
    pub fn view(&self) -> Result<Vec<SplineView>, String> {
        let world = self.evaluate()?;
        world
            .iter()
            .map(|(id, w)| {
                let spline = &self.splines[id];
                let n = VIEW_POINTS_PER_SEGMENT;
                let mut points = vec![w.segments[0][0]];
                for seg in &w.segments {
                    points.extend((1..=n).map(|i| bezier_point(seg, i as f64 / n as f64)));
                }
                let last = w.anchors.len() - 1;
                let anchors = w
                    .anchors
                    .iter()
                    .enumerate()
                    .map(|(k, a)| {
                        let tangent = if k < last {
                            bezier_tangent(&w.segments[k], 0.0)
                        } else {
                            bezier_tangent(&w.segments[k - 1], 1.0)
                        };
                        AnchorView {
                            position: a.position,
                            handle_in: add(a.position, a.handle_in),
                            handle_out: add(a.position, a.handle_out),
                            kind: a.kind,
                            at: w.anchor_fraction(k),
                            tangent: tangent.unwrap_or([0.0; 3]),
                        }
                    })
                    .collect();
                let profiles = spline
                    .profiles
                    .iter()
                    .map(|st| {
                        let profile = self.station_profile(st)?;
                        let frame = w.frame_at(st.at);
                        let (sin, cos) = st.rotation.sin_cos();
                        let k = st.scale * spline.thickness;
                        let ring = profile
                            .polygon()
                            .iter()
                            .map(|p| {
                                let (x, y) = (cos * p[0] - sin * p[1], sin * p[0] + cos * p[1]);
                                frame.to_world([x * k, y * k, 0.0])
                            })
                            .collect();
                        Ok(StationView {
                            at: st.at,
                            scale: st.scale,
                            rotation: st.rotation,
                            master: match st.shape {
                                ProfileShape::Master { master } => Some(master),
                                ProfileShape::Own { .. } => None,
                            },
                            profile,
                            ring,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(SplineView {
                    id: *id,
                    parent: spline.attachment.map(|a| a.parent),
                    at: spline.attachment.map_or(0.0, |a| a.at),
                    thickness: spline.thickness,
                    length: w.length,
                    points,
                    anchors,
                    profiles,
                    flat_end: spline.flat_end,
                })
            })
            .collect()
    }
}

/// Et sammenkoblet spline-hierarki. Hele hierarkiet er ett formlag.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Skeleton {
    next_id: SplineId,
    splines: BTreeMap<SplineId, Spline>,
    /// Masterprofiler som stasjoner kan kobles til.
    #[serde(default)]
    masters: BTreeMap<MasterId, Profile>,
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
            masters: BTreeMap::new(),
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
            profiles: Vec::new(),
            flat_end: false,
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
            profiles: Vec::new(),
            flat_end: false,
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

    fn station(&self, spline: SplineId, i: usize) -> Result<&Station, String> {
        self.splines
            .get(&spline)
            .ok_or("Splinen finnes ikke")?
            .profiles
            .get(i)
            .ok_or_else(|| "Profilen finnes ikke".into())
    }

    /// Omrisset til en stasjon, med masteren slått opp.
    pub fn station_profile(&self, station: &Station) -> Result<Profile, String> {
        match &station.shape {
            ProfileShape::Own { profile } => Ok(profile.clone()),
            ProfileShape::Master { master } => self
                .masters
                .get(master)
                .cloned()
                .ok_or_else(|| "Masteren finnes ikke".into()),
        }
    }

    /// Stasjonene til en spline med omrissene slått opp, sortert etter plass.
    pub fn resolved_profiles(&self, spline: SplineId) -> Result<Vec<ResolvedStation>, String> {
        let s = self.splines.get(&spline).ok_or("Splinen finnes ikke")?;
        s.profiles
            .iter()
            .map(|st| {
                Ok(ResolvedStation {
                    at: st.at,
                    profile: self.station_profile(st)?,
                    scale: st.scale,
                    rotation: st.rotation,
                })
            })
            .collect()
    }

    pub fn masters(&self) -> &BTreeMap<MasterId, Profile> {
        &self.masters
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
            for st in &s.profiles {
                st.validate()?;
                self.station_profile(st)?;
            }
            if s.profiles.windows(2).any(|w| w[0].at > w[1].at) {
                return Err(format!("Profilene i spline {id} er ikke sortert"));
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
        for m in self.masters.values() {
            m.validate()?;
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

    #[test]
    fn redigeringer_fra_grensesnittet() {
        use SkeletonEdit as E;
        let mut sk = Skeleton::new();
        let root = sk
            .apply(&E::AddRoot {
                start: [0.0; 3],
                direction: [0.0, 1.0, 0.0],
                length: 60.0,
                thickness: 4.0,
            })
            .unwrap()
            .unwrap();
        let child = sk
            .apply(&E::AddChild {
                parent: root,
                at: 0.25,
                direction: [1.0, 0.0, 0.0],
                length: 20.0,
                thickness: 2.0,
            })
            .unwrap()
            .unwrap();
        // Nytt punkt endrer ikke formen eller festet.
        let before = sk.evaluate().unwrap();
        sk.apply(&E::InsertAnchor {
            spline: root,
            at: 0.5,
        })
        .unwrap();
        let after = sk.evaluate().unwrap();
        assert_eq!(after[&root].anchors.len(), 3);
        for i in 0..=10 {
            let u = i as f64 / 10.0;
            assert!(close(
                before[&root].point_at(u),
                after[&root].point_at(u),
                1e-6
            ));
        }
        assert!(close(
            after[&child].point_at(1.0),
            before[&child].point_at(1.0),
            1e-6
        ));
        // Forleng langs enden.
        sk.apply(&E::Extend {
            spline: root,
            delta: 15.0,
        })
        .unwrap();
        assert!((sk.world_spline(root).unwrap().length - 75.0).abs() < 1e-6);
        // Bøy fra det nye punktet: delen før er uendret, lengden er låst.
        let w = sk.world_spline(root).unwrap();
        let (len, head) = (w.length, w.point_at(0.2));
        sk.apply(&E::Bend {
            spline: root,
            from: 1,
            direction: [1.0, 0.0, 0.0],
            angle: PI,
            radius: 5.0,
        })
        .unwrap();
        let w = sk.world_spline(root).unwrap();
        assert!((w.length - len).abs() / len < 1e-6);
        assert!(close(w.point_at(0.2), head, 1e-4));
        // En samling er ett steg: feiler én del, endres ingenting.
        let snapshot = sk.clone();
        let bad = E::Batch {
            edits: vec![
                E::Extend {
                    spline: root,
                    delta: 5.0,
                },
                E::Remove { spline: 77 },
            ],
        };
        assert!(sk.apply(&bad).is_err());
        assert_eq!(sk, snapshot);
        // JSON fra grensesnittet tolkes.
        let edit: SkeletonEdit = serde_json::from_str(
            r#"{"kind":"setKind","spline":1,"index":1,"anchor_kind":"corner"}"#,
        )
        .unwrap();
        sk.apply(&edit).unwrap();
        assert_eq!(sk.spline(root).unwrap().anchors[1].kind, AnchorKind::Corner);
    }

    #[test]
    fn master_styrer_koblede_profiler_og_lokal_storrelse_bevares() {
        use crate::profile::Profile;
        let mut sk = Skeleton::new();
        let id = sk
            .apply(&SkeletonEdit::AddRoot {
                start: [0.0; 3],
                direction: [1.0, 0.0, 0.0],
                length: 50.0,
                thickness: 5.0,
            })
            .unwrap()
            .unwrap();
        let e = |edit: SkeletonEdit, sk: &mut Skeleton| sk.apply(&edit).unwrap();
        e(
            SkeletonEdit::AddProfile {
                spline: id,
                at: 0.2,
                from: None,
            },
            &mut sk,
        );
        e(
            SkeletonEdit::SetProfile {
                spline: id,
                station: 0,
                profile: Profile::h_shape(),
            },
            &mut sk,
        );
        e(
            SkeletonEdit::MakeMaster {
                spline: id,
                station: 0,
            },
            &mut sk,
        );
        // Kopier og lim inn lenger ut: kopien er koblet til samme master.
        e(
            SkeletonEdit::AddProfile {
                spline: id,
                at: 0.8,
                from: Some(0),
            },
            &mut sk,
        );
        e(
            SkeletonEdit::SetStation {
                spline: id,
                station: 1,
                at: 0.8,
                scale: 2.0,
                rotation: 0.3,
            },
            &mut sk,
        );
        let view = sk.view().unwrap();
        let st = &view[0].profiles;
        assert_eq!(st.len(), 2);
        assert_eq!(st[0].master, Some(1));
        assert_eq!(st[1].master, Some(1));
        assert_eq!(st[0].scale, 1.0);

        // Endre masteren: begge følger, og lokal størrelse er uendret.
        e(
            SkeletonEdit::SetMaster {
                master: 1,
                profile: Profile::ellipse(1.0, 0.5),
            },
            &mut sk,
        );
        let view = sk.view().unwrap();
        for s in &view[0].profiles {
            assert_eq!(s.profile, Profile::ellipse(1.0, 0.5));
        }
        assert_eq!(view[0].profiles[1].scale, 2.0);

        // Gjør individuell: beholder formen, senere masterendringer påvirker den ikke.
        e(
            SkeletonEdit::Unlink {
                spline: id,
                station: 1,
            },
            &mut sk,
        );
        e(
            SkeletonEdit::SetMaster {
                master: 1,
                profile: Profile::circle(),
            },
            &mut sk,
        );
        let view = sk.view().unwrap();
        assert_eq!(view[0].profiles[0].profile, Profile::circle());
        assert_eq!(view[0].profiles[1].profile, Profile::ellipse(1.0, 0.5));
        assert_eq!(view[0].profiles[1].master, None);

        // Lagring og åpning beholder profiler og masterkoblinger.
        let back = Skeleton::from_json(&sk.to_json()).unwrap();
        assert_eq!(back, sk);
        // Et selvkryssende omriss avvises og endrer ingenting.
        let bow = Profile {
            points: [[1.0, 1.0], [-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0]]
                .iter()
                .map(|p| crate::profile::ProfilePoint::corner(*p))
                .collect(),
        };
        assert!(sk
            .apply(&SkeletonEdit::SetProfile {
                spline: id,
                station: 0,
                profile: bow
            })
            .is_err());
        assert_eq!(sk, back);
    }

    #[test]
    fn eldre_filer_uten_profiler_kan_aapnes() {
        let mut sk = Skeleton::new();
        sk.apply(&SkeletonEdit::AddRoot {
            start: [0.0; 3],
            direction: [0.0, 1.0, 0.0],
            length: 10.0,
            thickness: 2.0,
        })
        .unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&sk.to_json()).unwrap();
        let obj = json["skeleton"].as_object_mut().unwrap();
        obj.remove("masters");
        for (_, s) in obj["splines"].as_object_mut().unwrap() {
            let s = s.as_object_mut().unwrap();
            s.remove("profiles");
            s.remove("flat_end");
        }
        assert_eq!(Skeleton::from_json(&json.to_string()).unwrap(), sk);
    }
}
