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

/// Hvordan en gren følger forelderen (Sivert 29. oktober 2026).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    /// Festet og grenens retning følger forelderens lokale ramme, så
    /// grenens vinkel mot forelderen bevares når forelderen bøyes.
    #[default]
    FollowParent,
    /// Festet følger forelderen, men grenen beholder retningen i
    /// verdensrommet når forelderen bøyes eller redigeres.
    KeepDirection,
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
    /// Navnet brukeren har gitt splinen. For en rotspline er det lagets navn.
    /// Tomt: visningen lager et navn.
    #[serde(default)]
    pub name: String,
    /// Laget er skjult. Gjelder bare rotspliner, og dermed hele laget.
    #[serde(default)]
    pub hidden: bool,
    /// Laget er låst mot redigering. Gjelder bare rotspliner.
    #[serde(default)]
    pub locked: bool,
    /// Hvordan en gren følger forelderen. Uten betydning for rotspliner.
    #[serde(default)]
    pub orientation: Orientation,
    /// Enden er koblet til et punkt på en annen spline, så spliner kan danne
    /// et nett. Tuppen følger punktet når den andre splinen endres.
    #[serde(default)]
    pub end: Option<Attachment>,
    /// Ferdige 3D-mesher (horn, klør, øyne …) satt inn langs splinen.
    #[serde(default)]
    pub parts: Vec<Part>,
    /// Laget trekkes fra andre lag (negativ form). Gjelder bare rotspliner.
    #[serde(default)]
    pub cut: Option<Cut>,
}

/// Et lag som trekkes fra («− Trekk fra») i stedet for å legges til
/// (issue #12 punkt 4). Formen forblir et redigerbart lag. Hva den skjærer i
/// er en egen relasjon, uavhengig av forelder, feste og rekkefølgen i
/// laglisten, så omorganisering endrer aldri hvilke deler som kuttes.
/// Utskjæringen skjer i avstandsfeltet; skjelettet og festene i målet
/// berøres ikke, så splinefester blir stående selv om materialet rundt dem
/// fjernes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cut {
    /// Lagene (rotsplinene) formen skjærer i.
    #[serde(default)]
    pub targets: Vec<SplineId>,
    /// Utskjæringen er aktiv. Av: kutteren vises, men skjærer ikke.
    /// Om kutteren vises styres separat av lagets synlighet.
    #[serde(default = "default_true")]
    pub active: bool,
    /// Skarp kant mot kuttet i stedet for myk overgang.
    #[serde(default)]
    pub sharp: bool,
}

fn default_true() -> bool {
    true
}

/// Hvor en mesh-del kommer fra: de innebygde formene eller en importert mesh.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartSource {
    Builtin { name: String },
    Stored { id: u32 },
}

/// En mesh-del på en spline. Delen står i rammen ved andelen `at`: langs
/// kurven, eller ut fra overflaten (`outward`) i retningen `rotation` rundt
/// kurven. `size` er høyden i mm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub source: PartSource,
    pub at: f64,
    pub size: f64,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub outward: bool,
}

/// En importert mesh i delregisteret, normalisert til høyde 1 langs +Y med
/// foten i origo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartMesh {
    pub name: String,
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

/// Innstillinger for meshen (Innstillinger → Mesh).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshSettings {
    /// Hvor mange ganger kanter i knekker deles (0 = av).
    pub crease_detail: u32,
    /// Vinkelen mellom nabotrekanter i grader der en knekk begynner.
    pub crease_angle: f64,
}

impl Default for MeshSettings {
    fn default() -> Self {
        MeshSettings {
            crease_detail: 2,
            crease_angle: 25.0,
        }
    }
}

impl MeshSettings {
    pub const MAX_DETAIL: u32 = 3;
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
        #[serde(default)]
        orientation: Orientation,
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
    /// Flytter festet til andelen `at`, eventuelt over på en ny forelder.
    /// Grenen beholder vinkelen mot forelderen (følg forelder) eller
    /// retningen i verdensrommet (behold retning), og undergrenene følger.
    Reattach {
        spline: SplineId,
        at: f64,
        #[serde(default)]
        parent: Option<SplineId>,
    },
    /// Fester enden av splinen til andelen `at` av splinen `target`.
    AttachEnd {
        spline: SplineId,
        target: SplineId,
        at: f64,
    },
    /// Løsner enden; tuppen blir stående der den er.
    DetachEnd {
        spline: SplineId,
    },
    /// Flytter en kontur over på en annen spline (eller langs samme).
    MoveProfile {
        spline: SplineId,
        station: usize,
        to: SplineId,
        at: f64,
    },
    /// Setter inn en mesh-del på splinen.
    AddPart {
        spline: SplineId,
        part: Part,
        /// Delen står ut fra overflaten i retningen nærmest denne.
        #[serde(default)]
        toward: Option<Vec3>,
    },
    SetPart {
        spline: SplineId,
        index: usize,
        part: Part,
    },
    RemovePart {
        spline: SplineId,
        index: usize,
    },
    /// Legger en importert mesh i delregisteret.
    ImportPart {
        name: String,
        positions: Vec<Vec3>,
        triangles: Vec<[u32; 3]>,
    },
    /// Fjerner en importert mesh og delene som bruker den.
    RemovePartMesh {
        id: u32,
    },
    /// Speilmodus: alt speiles om planet x = 0.
    SetMirror {
        mirror: bool,
    },
    SetMeshSettings {
        settings: MeshSettings,
    },
    /// Bytter hvordan grenen følger forelderen, uten at den flytter seg.
    SetOrientation {
        spline: SplineId,
        orientation: Orientation,
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
    /// Nytt navn på en spline. For en rotspline er det lagets navn.
    Rename {
        spline: SplineId,
        name: String,
    },
    /// Erstatter alle konturene på splinen, for eksempel med et volum fra
    /// registeret (smal hals, større hode og avsmalnende ende).
    SetStations {
        spline: SplineId,
        stations: Vec<Station>,
    },
    /// Lagrer omrisset til en kontur i registeret under `name`.
    SaveProfile {
        spline: SplineId,
        station: usize,
        name: String,
    },
    /// Lagrer alle konturene på splinen som et volum i registeret.
    SaveVolume {
        spline: SplineId,
        name: String,
    },
    RenameEntry {
        entry: u32,
        name: String,
    },
    RemoveEntry {
        entry: u32,
    },
    /// Gjør laget med rotsplinen `spline` til en negativ form som skjærer i
    /// lagene i `cut.targets`, eller tilbake til en vanlig form (`None`).
    SetCut {
        spline: SplineId,
        cut: Option<Cut>,
    },
    /// Skjuler eller låser laget med rotsplinen `spline`. Tillatt også når
    /// laget er låst, så det kan låses opp.
    SetLayer {
        spline: SplineId,
        hidden: bool,
        locked: bool,
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
        next.prune_cuts();
        *self = next;
        Ok(created)
    }

    fn apply_inner(&mut self, edit: &SkeletonEdit) -> Result<Option<SplineId>, String> {
        use SkeletonEdit as E;
        self.check_unlocked(edit)?;
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
                orientation,
            } => {
                let start = self.world_spline(parent)?.point_at(at);
                let anchors = straight_anchors(start, direction, length)?;
                let id = self.add_child(parent, at, anchors, thickness)?;
                if orientation != Orientation::FollowParent {
                    self.set_orientation(id, orientation)?;
                }
                return Ok(Some(id));
            }
            E::MoveAnchor {
                spline,
                index,
                position,
            } => {
                if index + 1
                    == self
                        .spline(spline)
                        .ok_or("Splinen finnes ikke")?
                        .anchors
                        .len()
                {
                    self.detach_end(spline)?;
                }
                self.move_anchor(spline, index, position)?
            }
            E::MoveTip { spline, position } => {
                self.detach_end(spline)?;
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
                self.detach_end(spline)?;
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
                self.detach_end(spline)?;
                let world = self.world_spline(spline)?;
                let bend = crate::bend::Bend::at_middle(direction, angle, radius);
                let anchors = crate::bend::bend_from(&world, world.anchor_fraction(from), &bend)?;
                self.set_world_anchors(spline, anchors)?
            }
            E::Reattach { spline, at, parent } => self.reattach(spline, parent, at)?,
            E::AttachEnd { spline, target, at } => self.attach_end(spline, target, at)?,
            E::DetachEnd { spline } => self.detach_end(spline)?,
            E::MoveProfile {
                spline,
                station,
                to,
                at,
            } => {
                if self.is_locked(to) {
                    return Err("Laget er låst".into());
                }
                if !(0.0..=1.0).contains(&at) {
                    return Err("Konturen må ligge på splinen".into());
                }
                let st = Station {
                    at,
                    ..self.station(spline, station)?.clone()
                };
                self.splines
                    .get_mut(&spline)
                    .unwrap()
                    .profiles
                    .remove(station);
                self.update(to, |s| {
                    s.profiles.push(st);
                    s.profiles.sort_by(|a, b| a.at.total_cmp(&b.at));
                })?
            }
            E::AddPart {
                spline,
                ref part,
                toward,
            } => {
                self.check_part(part)?;
                let mut part = part.clone();
                if let Some(d) = toward {
                    let f = self.world_spline(spline)?.frame_at(part.at);
                    part.rotation = dot(d, f.v).atan2(dot(d, f.u));
                    part.outward = true;
                }
                self.update(spline, |s| s.parts.push(part))?
            }
            E::SetPart {
                spline,
                index,
                ref part,
            } => {
                self.check_part(part)?;
                let part = part.clone();
                self.update(spline, |s| {
                    if let Some(p) = s.parts.get_mut(index) {
                        *p = part;
                    }
                })?;
                if self.splines[&spline].parts.len() <= index {
                    return Err("Delen finnes ikke".into());
                }
            }
            E::RemovePart { spline, index } => {
                if self
                    .spline(spline)
                    .ok_or("Splinen finnes ikke")?
                    .parts
                    .len()
                    <= index
                {
                    return Err("Delen finnes ikke".into());
                }
                self.update(spline, |s| {
                    s.parts.remove(index);
                })?
            }
            E::ImportPart {
                ref name,
                ref positions,
                ref triangles,
            } => {
                let mesh = PartMesh {
                    name: valid_name(name)?,
                    positions: positions.clone(),
                    triangles: triangles.clone(),
                };
                validate_part_mesh(&mesh)?;
                let id = self.part_meshes.keys().next_back().map_or(1, |k| k + 1);
                self.part_meshes.insert(id, mesh);
            }
            E::RemovePartMesh { id } => {
                self.part_meshes.remove(&id).ok_or("Delen finnes ikke")?;
                let used = PartSource::Stored { id };
                for s in self.splines.values_mut() {
                    s.parts.retain(|p| p.source != used);
                }
            }
            E::SetMirror { mirror } => self.mirror = mirror,
            E::SetMeshSettings { settings } => {
                if settings.crease_detail > MeshSettings::MAX_DETAIL
                    || !(1.0..=90.0).contains(&settings.crease_angle)
                {
                    return Err("Ugyldige meshinnstillinger".into());
                }
                self.settings = settings;
            }
            E::SetOrientation {
                spline,
                orientation,
            } => self.set_orientation(spline, orientation)?,
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
            E::SetStations {
                spline,
                ref stations,
            } => {
                let mut stations = stations.clone();
                for st in &stations {
                    st.validate()?;
                }
                stations.sort_by(|a, b| a.at.total_cmp(&b.at));
                self.update(spline, |s| s.profiles = stations)?
            }
            E::SaveProfile {
                spline,
                station,
                ref name,
            } => {
                let profile = self.station_profile(self.station(spline, station)?)?;
                self.add_entry(name, vec![Station::new(0.5, profile)], false)?
            }
            E::SaveVolume { spline, ref name } => {
                let stations: Vec<Station> = self
                    .resolved_profiles(spline)?
                    .into_iter()
                    .map(|r| Station {
                        scale: r.scale,
                        rotation: r.rotation,
                        ..Station::new(r.at, r.profile)
                    })
                    .collect();
                if stations.len() < 2 {
                    return Err("Et volum trenger minst to konturer langs splinen".into());
                }
                self.add_entry(name, stations, true)?
            }
            E::RenameEntry { entry, ref name } => {
                let name = valid_name(name)?;
                self.registry
                    .get_mut(&entry)
                    .ok_or("Oppføringen finnes ikke")?
                    .name = name;
            }
            E::RemoveEntry { entry } => {
                self.registry
                    .remove(&entry)
                    .ok_or("Oppføringen finnes ikke")?;
            }
            E::Rename { spline, ref name } => {
                let name = name.trim().to_string();
                self.update(spline, |s| s.name = name)?
            }
            E::SetLayer {
                spline,
                hidden,
                locked,
            } => {
                if self
                    .spline(spline)
                    .ok_or("Splinen finnes ikke")?
                    .attachment
                    .is_some()
                {
                    return Err("Bare et helt lag kan skjules eller låses".into());
                }
                self.update(spline, |s| {
                    s.hidden = hidden;
                    s.locked = locked;
                })?
            }
            E::SetCut { spline, ref cut } => self.set_cut(spline, cut.clone())?,
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
    pub name: String,
    /// Rotsplinen i laget splinen hører til.
    pub layer: SplineId,
    /// Laget er skjult eller låst (arves fra rotsplinen).
    pub hidden: bool,
    pub locked: bool,
    pub orientation: Orientation,
    /// Splinen og andelen enden er koblet til, hvis den er det.
    pub end_parent: Option<SplineId>,
    pub end_at: f64,
    pub parts: Vec<PartView>,
    /// Negativ form: lagene laget skjærer i. Bare for rotspliner.
    pub cut: Option<Cut>,
}

/// En mesh-del med foten og høyderetningen i verdenskoordinater.
#[derive(Clone, Debug, Serialize)]
pub struct PartView {
    #[serde(flatten)]
    pub part: Part,
    pub name: String,
    pub base: Vec3,
    pub axis: Vec3,
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
    /// Konturens plan: sentrum på kurven, tangenten (planets normal) og
    /// aksene profilens x og y peker langs (med rotasjonen tatt med).
    pub origin: Vec3,
    pub normal: Vec3,
    pub x_axis: Vec3,
    pub y_axis: Vec3,
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
                            origin: frame.origin,
                            normal: frame.t,
                            x_axis: frame.dir_to_world([cos, sin, 0.0]),
                            y_axis: frame.dir_to_world([-sin, cos, 0.0]),
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
                let root = self.root_of(*id).unwrap_or(*id);
                let layer = &self.splines[&root];
                Ok(SplineView {
                    name: spline.name.clone(),
                    layer: root,
                    hidden: layer.hidden,
                    locked: layer.locked,
                    orientation: spline.orientation,
                    id: *id,
                    parent: spline.attachment.map(|a| a.parent),
                    at: spline.attachment.map_or(0.0, |a| a.at),
                    thickness: spline.thickness,
                    length: w.length,
                    points,
                    anchors,
                    profiles,
                    flat_end: spline.flat_end,
                    end_parent: spline.end.map(|e| e.parent),
                    end_at: spline.end.map_or(0.0, |e| e.at),
                    cut: spline.cut.clone(),
                    parts: spline
                        .parts
                        .iter()
                        .map(|part| {
                            let (base, axes) = crate::parts::placement(
                                &w.frame_at(part.at),
                                part,
                                spline.thickness,
                            );
                            PartView {
                                name: match &part.source {
                                    PartSource::Builtin { name } => name.clone(),
                                    PartSource::Stored { id } => self
                                        .part_meshes
                                        .get(id)
                                        .map_or_else(String::new, |m| m.name.clone()),
                                },
                                part: part.clone(),
                                base,
                                axis: axes[1],
                            }
                        })
                        .collect(),
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
    /// Konturregisteret: lagrede profiler og volumer brukeren kan gjenbruke.
    /// En oppføring kopieres når den brukes og kobles ikke varig.
    #[serde(default)]
    registry: BTreeMap<u32, RegistryEntry>,
    /// Delregisteret: importerte mesher som kan settes inn på spliner.
    #[serde(default)]
    part_meshes: BTreeMap<u32, PartMesh>,
    /// Speilmodus om planet x = 0, for hele modellen.
    #[serde(default)]
    mirror: bool,
    #[serde(default)]
    settings: MeshSettings,
}

/// En lagret profil (én kontur) eller et volum (flere konturer langs en spline).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub name: String,
    /// Konturene, alltid med eget omriss. Én for en profil; for et volum
    /// er `at` plassen langs splinen volumet legges på.
    pub stations: Vec<Station>,
    pub volume: bool,
}

/// Delregisteret og innstillingene slik visningen trenger dem.
#[derive(Clone, Debug, Serialize)]
pub struct LibraryView {
    pub parts: Vec<PartEntryView>,
    pub mirror: bool,
    pub settings: MeshSettings,
}

#[derive(Clone, Debug, Serialize)]
pub struct PartEntryView {
    pub source: PartSource,
    pub name: String,
}

/// Største importerte mesh i delregisteret.
pub const MAX_PART_TRIANGLES: usize = 200_000;

fn validate_part_mesh(m: &PartMesh) -> Result<(), String> {
    if m.triangles.is_empty() || m.triangles.len() > MAX_PART_TRIANGLES {
        return Err(format!(
            "En del må ha mellom 1 og {MAX_PART_TRIANGLES} trekanter"
        ));
    }
    if m.positions.iter().flatten().any(|x| !x.is_finite()) {
        return Err("Delen har ugyldige tall".into());
    }
    let n = m.positions.len() as u32;
    if m.triangles.iter().flatten().any(|&i| i >= n) {
        return Err("Delen har ugyldige trekanter".into());
    }
    Ok(())
}

/// En oppføring i registeret slik visningen trenger den.
#[derive(Clone, Debug, Serialize)]
pub struct RegistryView {
    pub id: u32,
    #[serde(flatten)]
    pub entry: RegistryEntry,
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
            registry: BTreeMap::new(),
            part_meshes: BTreeMap::new(),
            mirror: false,
            settings: MeshSettings::default(),
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
            name: String::new(),
            hidden: false,
            locked: false,
            orientation: Orientation::FollowParent,
            end: None,
            parts: Vec::new(),
            cut: None,
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
        // Splinen er ny og følger forelderen, så festerammen er forelderens ramme.
        let spline = Spline {
            anchors: to_local(&frame, &world_anchors),
            attachment: Some(attachment),
            up: [0.0, 0.0, 1.0],
            thickness,
            profiles: Vec::new(),
            flat_end: false,
            name: String::new(),
            hidden: false,
            locked: false,
            orientation: Orientation::FollowParent,
            end: None,
            parts: Vec::new(),
            cut: None,
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
        let removed = self.descendants(id);
        // Ender koblet til en spline som fjernes, blir stående der de er.
        let pinned: Vec<SplineId> = self
            .splines
            .iter()
            .filter(|(k, s)| {
                !removed.contains(k) && s.end.is_some_and(|e| removed.contains(&e.parent))
            })
            .map(|(k, _)| *k)
            .collect();
        for p in pinned {
            self.detach_end(p)?;
        }
        for d in removed {
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

    /// Rammen grenens punkter er lagret i, og referanseretningen for
    /// grenens egen ramme, ut fra forelderens ramme i festet.
    fn placement(spline: &Spline, parent_frame: Frame) -> (Frame, Vec3) {
        match spline.orientation {
            Orientation::FollowParent => (parent_frame, parent_frame.u),
            // Bare festepunktet følger; aksene er verdensaksene.
            Orientation::KeepDirection => (
                Frame {
                    origin: parent_frame.origin,
                    t: [0.0, 0.0, 1.0],
                    u: [1.0, 0.0, 0.0],
                    v: [0.0, 1.0, 0.0],
                },
                spline.up,
            ),
        }
    }

    /// Rammen punktene til grenen `id` er lagret i.
    fn child_frame(&self, id: SplineId) -> Result<Frame, String> {
        let spline = self.splines.get(&id).ok_or("Splinen finnes ikke")?;
        let a = spline
            .attachment
            .ok_or("Splinen er ikke festet til en forelder")?;
        Ok(Self::placement(spline, self.attachment_frame(&a)?).0)
    }

    /// Splinen i verdenskoordinater. Forfedrene og splinen enden er koblet
    /// til, evalueres først.
    pub fn world_spline(&self, id: SplineId) -> Result<WorldSpline, String> {
        self.world_rec(id, 0)
    }

    fn world_rec(&self, id: SplineId, depth: usize) -> Result<WorldSpline, String> {
        if depth > self.splines.len() {
            return Err("Splinene er koblet i en sløyfe".into());
        }
        let spline = self.splines.get(&id).ok_or("Splinen finnes ikke")?;
        let parent = match spline.attachment {
            Some(a) => Some(self.world_rec(a.parent, depth + 1)?),
            None => None,
        };
        let end = match spline.end {
            Some(e) => Some(self.world_rec(e.parent, depth + 1)?.point_at(e.at)),
            None => None,
        };
        Self::place(spline, parent.as_ref(), end)
    }

    /// Splinen i verdenskoordinater fra forelderen og punktet enden er koblet til.
    fn place(
        spline: &Spline,
        parent: Option<&WorldSpline>,
        end: Option<Vec3>,
    ) -> Result<WorldSpline, String> {
        let (mut anchors, up) = match (spline.attachment, parent) {
            (Some(a), Some(p)) => {
                let (frame, up) = Self::placement(spline, p.frame_at(a.at));
                (to_world(&frame, &spline.anchors), up)
            }
            _ => (spline.anchors.clone(), spline.up),
        };
        if let Some(e) = end {
            anchors.last_mut().unwrap().position = e;
        }
        WorldSpline::new(anchors, up)
    }

    /// Alle spliner i verdenskoordinater, i rekkefølgen de avhenger av
    /// hverandre: forelderen og splinen enden er koblet til kommer først.
    pub fn evaluate(&self) -> Result<BTreeMap<SplineId, WorldSpline>, String> {
        let mut out: BTreeMap<SplineId, WorldSpline> = BTreeMap::new();
        while out.len() < self.splines.len() {
            let before = out.len();
            for (id, s) in &self.splines {
                if out.contains_key(id) {
                    continue;
                }
                let parent = match s.attachment {
                    Some(a) => match out.get(&a.parent) {
                        Some(p) => Some(p),
                        None => continue,
                    },
                    None => None,
                };
                let end = match s.end {
                    Some(e) => match out.get(&e.parent) {
                        Some(t) => Some(t.point_at(e.at)),
                        None => continue,
                    },
                    None => None,
                };
                let world = Self::place(s, parent, end)?;
                out.insert(*id, world);
            }
            if out.len() == before {
                return Err("Splinene er koblet i en sløyfe".into());
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
            Some(_) => to_local(&self.child_frame(id)?, &world_anchors),
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

    /// Flytter festet langs forelderen, eller over på en ny forelder. Grenens
    /// punkter i festerammen beholdes: med «følg forelder» er det vinkelen mot
    /// forelderen, med «behold retning» retningen i verdensrommet. Sløyfer og
    /// feste til seg selv avvises av valideringen.
    pub fn reattach(
        &mut self,
        id: SplineId,
        parent: Option<SplineId>,
        at: f64,
    ) -> Result<(), String> {
        let spline = self.splines.get(&id).ok_or("Splinen finnes ikke")?;
        let Some(a) = spline.attachment else {
            return Err("Splinen er ikke festet til en forelder".into());
        };
        let parent = parent.unwrap_or(a.parent);
        if parent == id || self.descendants(id).contains(&parent) {
            return Err("En gren kan ikke festes til seg selv eller sine egne undergrener".into());
        }
        self.update(id, |s| s.attachment = Some(Attachment { parent, at }))
    }

    /// Kobler enden av splinen til andelen `at` av `target`. Tuppen flyttes
    /// dit og følger etter når `target` endres. En spline kan ikke kobles til
    /// seg selv eller til noe som avhenger av den (valideringen finner sløyfer).
    pub fn attach_end(&mut self, id: SplineId, target: SplineId, at: f64) -> Result<(), String> {
        if !(0.0..=1.0).contains(&at) {
            return Err("Enden må kobles til et punkt på splinen".into());
        }
        if id == target {
            return Err("En spline kan ikke kobles til seg selv".into());
        }
        if !self.splines.contains_key(&target) {
            return Err("Splinen finnes ikke".into());
        }
        let mut next = self.clone();
        next.splines.get_mut(&id).ok_or("Splinen finnes ikke")?.end =
            Some(Attachment { parent: target, at });
        next.validate()
            .map_err(|_| "Enden kan ikke kobles til en spline som henger på denne".to_string())?;
        // Det lagrede punktet settes også til målet, så splinen står likt om
        // koblingen løsnes.
        let world = next.world_anchors(id)?;
        next.set_world_anchors(id, world)?;
        *self = next;
        Ok(())
    }

    /// Løsner enden. Tuppen blir stående der den er nå.
    pub fn detach_end(&mut self, id: SplineId) -> Result<(), String> {
        let spline = self.splines.get(&id).ok_or("Splinen finnes ikke")?;
        if spline.end.is_none() {
            return Ok(());
        }
        let world = self.world_anchors(id)?;
        let mut next = self.clone();
        next.splines.get_mut(&id).unwrap().end = None;
        next.set_world_anchors(id, world)?;
        *self = next;
        Ok(())
    }

    /// Bytter hvordan grenen følger forelderen. Grenen står der den står; bare
    /// lagringen av punktene endres, så det blir ikke noe hopp.
    pub fn set_orientation(
        &mut self,
        id: SplineId,
        orientation: Orientation,
    ) -> Result<(), String> {
        let world = self.world_spline(id)?;
        if self.splines[&id].attachment.is_none() {
            return Err("Bare en gren kan følge eller beholde retningen".into());
        }
        // Med «behold retning» lagres grenens egen referanseretning i
        // verdensrommet, så tverrsnittet ikke vrir seg når forelderen bøyes.
        let up = world.start_frame().u;
        let mut next = self.clone();
        next.update(id, |s| {
            s.orientation = orientation;
            s.up = up;
        })?;
        next.set_world_anchors(id, world.anchors)?;
        *self = next;
        Ok(())
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

    /// Rotsplinen i laget som `id` hører til.
    pub fn root_of(&self, id: SplineId) -> Option<SplineId> {
        let mut cur = id;
        for _ in 0..=self.splines.len() {
            match self.splines.get(&cur)?.attachment {
                Some(a) => cur = a.parent,
                None => return Some(cur),
            }
        }
        None
    }

    /// Laget til `id` er låst.
    pub fn is_locked(&self, id: SplineId) -> bool {
        self.root_of(id)
            .and_then(|r| self.splines.get(&r))
            .is_some_and(|s| s.locked)
    }

    /// Avviser redigeringer av et låst lag. Navn, skjuling og låsing er
    /// tillatt, så laget kan låses opp igjen.
    fn check_unlocked(&self, edit: &SkeletonEdit) -> Result<(), String> {
        use SkeletonEdit as E;
        let target = match *edit {
            E::AddRoot { .. }
            | E::Batch { .. }
            | E::Rename { .. }
            | E::SetLayer { .. }
            | E::RenameEntry { .. }
            | E::RemoveEntry { .. }
            // Å lagre en kontur i registeret endrer ikke laget den ligger i.
            | E::SaveProfile { .. }
            | E::SaveVolume { .. }
            | E::ImportPart { .. }
            | E::RemovePartMesh { .. }
            | E::SetMirror { .. }
            | E::SetMeshSettings { .. } => None,
            E::SetStations { spline, .. } => Some(spline),
            E::AddChild { parent, .. } => Some(parent),
            E::MoveAnchor { spline, .. }
            | E::MoveTip { spline, .. }
            | E::SetHandle { spline, .. }
            | E::SetKind { spline, .. }
            | E::InsertAnchor { spline, .. }
            | E::Extend { spline, .. }
            | E::Bend { spline, .. }
            | E::Reattach { spline, .. }
            | E::AttachEnd { spline, .. }
            | E::DetachEnd { spline }
            | E::MoveProfile { spline, .. }
            | E::AddPart { spline, .. }
            | E::SetPart { spline, .. }
            | E::RemovePart { spline, .. }
            | E::SetOrientation { spline, .. }
            | E::Translate { spline, .. }
            | E::SetThickness { spline, .. }
            | E::Remove { spline }
            | E::AddProfile { spline, .. }
            | E::SetProfile { spline, .. }
            | E::SetStation { spline, .. }
            | E::RemoveProfile { spline, .. }
            | E::MakeMaster { spline, .. }
            | E::LinkMaster { spline, .. }
            | E::Unlink { spline, .. }
            | E::SetFlatEnd { spline, .. }
            | E::SetCut { spline, .. } => Some(spline),
            E::SetMaster { master, .. } => {
                // En master endrer alle koblede konturer, også i andre lag.
                let linked = self.splines.iter().find(|(_, s)| {
                    s.profiles
                        .iter()
                        .any(|st| st.shape == ProfileShape::Master { master })
                });
                linked.map(|(id, _)| *id).filter(|id| self.is_locked(*id))
            }
        };
        match target {
            Some(id) if self.is_locked(id) => Err("Laget er låst".into()),
            _ => Ok(()),
        }
    }

    fn add_entry(
        &mut self,
        name: &str,
        stations: Vec<Station>,
        volume: bool,
    ) -> Result<(), String> {
        let name = valid_name(name)?;
        let id = self.registry.keys().next_back().map_or(1, |k| k + 1);
        self.registry.insert(
            id,
            RegistryEntry {
                name,
                stations,
                volume,
            },
        );
        Ok(())
    }

    /// Konturregisteret.
    pub fn registry(&self) -> Vec<RegistryView> {
        self.registry
            .iter()
            .map(|(id, entry)| RegistryView {
                id: *id,
                entry: entry.clone(),
            })
            .collect()
    }

    /// En rotspline som ikke selv er en negativ form.
    fn is_plain_root(&self, id: SplineId) -> bool {
        self.splines
            .get(&id)
            .is_some_and(|s| s.attachment.is_none() && s.cut.is_none())
    }

    fn set_cut(&mut self, id: SplineId, cut: Option<Cut>) -> Result<(), String> {
        let spline = self.spline(id).ok_or("Splinen finnes ikke")?;
        if spline.attachment.is_some() {
            return Err("Bare et helt lag kan trekkes fra".into());
        }
        let cut = match cut {
            None => None,
            Some(mut c) => {
                if let Some((other, _)) = self
                    .splines
                    .iter()
                    .find(|(_, s)| s.cut.as_ref().is_some_and(|c| c.targets.contains(&id)))
                {
                    return Err(format!(
                        "Laget kuttes av lag {other} og kan ikke selv trekkes fra"
                    ));
                }
                let mut seen = Vec::new();
                for t in c.targets.drain(..) {
                    if t == id || !self.is_plain_root(t) {
                        return Err("En negativ form kan bare skjære i vanlige lag".into());
                    }
                    if !seen.contains(&t) {
                        seen.push(t);
                    }
                }
                c.targets = seen;
                Some(c)
            }
        };
        self.update(id, |s| s.cut = cut)
    }

    /// Fjerner kuttemål som ikke lenger er vanlige lag (slettet eller festet
    /// som gren), og kutt på spliner som ikke lenger er rotspliner.
    fn prune_cuts(&mut self) {
        let plain: Vec<SplineId> = self
            .splines
            .iter()
            .filter(|(_, s)| s.attachment.is_none() && s.cut.is_none())
            .map(|(id, _)| *id)
            .collect();
        for s in self.splines.values_mut() {
            if s.attachment.is_some() {
                s.cut = None;
            }
            if let Some(c) = &mut s.cut {
                c.targets.retain(|t| plain.contains(t));
            }
        }
    }

    /// De aktive negative formene som skjærer i laget `root`.
    pub fn cutters_of(&self, root: SplineId) -> Vec<(SplineId, &Cut)> {
        self.splines
            .iter()
            .filter_map(|(id, s)| s.cut.as_ref().map(|c| (*id, c)))
            .filter(|(_, c)| c.active && c.targets.contains(&root))
            .collect()
    }

    pub fn masters(&self) -> &BTreeMap<MasterId, Profile> {
        &self.masters
    }

    /// Noe som gjelder alle kropper er endret.
    pub fn shared_differs(&self, other: &Skeleton) -> bool {
        self.masters != other.masters
            || self.mirror != other.mirror
            || self.settings != other.settings
            || self.part_meshes != other.part_meshes
    }

    pub fn mirror(&self) -> bool {
        self.mirror
    }

    pub fn settings(&self) -> MeshSettings {
        self.settings
    }

    pub fn part_mesh(&self, id: u32) -> Option<&PartMesh> {
        self.part_meshes.get(&id)
    }

    fn check_part(&self, part: &Part) -> Result<(), String> {
        if !(0.0..=1.0).contains(&part.at) {
            return Err("Delen må ligge på splinen".into());
        }
        if !(part.size.is_finite() && part.size > 0.0) || !part.rotation.is_finite() {
            return Err("Ugyldig størrelse på delen".into());
        }
        match &part.source {
            PartSource::Builtin { name } if crate::parts::builtin(name).is_none() => {
                Err(format!("Ukjent del: {name}"))
            }
            PartSource::Stored { id } if !self.part_meshes.contains_key(id) => {
                Err("Delen finnes ikke i registeret".into())
            }
            _ => Ok(()),
        }
    }

    /// Delregisteret, speilingen og meshinnstillingene for visningen.
    pub fn library(&self) -> LibraryView {
        let mut parts: Vec<PartEntryView> = crate::parts::BUILTIN
            .iter()
            .map(|name| PartEntryView {
                source: PartSource::Builtin {
                    name: (*name).into(),
                },
                name: (*name).into(),
            })
            .collect();
        parts.extend(self.part_meshes.iter().map(|(id, m)| PartEntryView {
            source: PartSource::Stored { id: *id },
            name: m.name.clone(),
        }));
        LibraryView {
            parts,
            mirror: self.mirror,
            settings: self.settings,
        }
    }

    /// Endrer en spline og godtar endringen bare hvis hele skjelettet er gyldig.
    fn update(&mut self, id: SplineId, f: impl FnOnce(&mut Spline)) -> Result<(), String> {
        let mut next = self.clone();
        f(next.splines.get_mut(&id).ok_or("Splinen finnes ikke")?);
        next.prune_cuts();
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
            if s.attachment.is_some() && (s.hidden || s.locked) {
                return Err(format!(
                    "Spline {id} er en gren; bare lag kan skjules eller låses"
                ));
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
            for part in &s.parts {
                self.check_part(part)?;
            }
            if let Some(e) = s.end {
                if e.parent == *id || !self.splines.contains_key(&e.parent) {
                    return Err(format!(
                        "Enden av spline {id} er koblet til en ugyldig spline"
                    ));
                }
                if !(0.0..=1.0).contains(&e.at) {
                    return Err(format!("Enden av spline {id} er koblet utenfor splinen"));
                }
            }
            if let Some(c) = &s.cut {
                if s.attachment.is_some() {
                    return Err(format!("Spline {id} er en gren; bare lag kan trekkes fra"));
                }
                for t in &c.targets {
                    if *t == *id || !self.is_plain_root(*t) {
                        return Err(format!("Lag {id} skjærer i et ugyldig lag"));
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
        for m in self.part_meshes.values() {
            validate_part_mesh(m)?;
        }
        for e in self.registry.values() {
            for st in &e.stations {
                st.validate()?;
                if !matches!(st.shape, ProfileShape::Own { .. }) {
                    return Err("En oppføring i registeret må ha egne omriss".into());
                }
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

fn valid_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Oppføringen trenger et navn".into());
    }
    Ok(name.chars().take(60).collect())
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
        sk.reattach(child, None, 0.8).unwrap();
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
                orientation: Orientation::FollowParent,
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

    /// Bøyer roten i `three_levels` 90° mot +x med pivot 0,3.
    fn bend_root(sk: &mut Skeleton, root: SplineId) {
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
    }

    #[test]
    fn behold_retning_holder_grenen_fast_i_verdensrommet_mens_festet_folger() {
        let (mut sk, root, child, gc) = three_levels();
        sk.set_orientation(child, Orientation::KeepDirection)
            .unwrap();
        let before = sk.evaluate().unwrap();
        bend_root(&mut sk, root);
        let after = sk.evaluate().unwrap();
        // Festet følger forelderen.
        assert!(close(
            after[&child].point_at(0.0),
            after[&root].point_at(0.5),
            1e-9
        ));
        // Grenen er bare flyttet, ikke rotert: alle punkter har samme forskyvning.
        let shift = sub(after[&child].point_at(0.0), before[&child].point_at(0.0));
        assert!(length(shift) > 1.0);
        for (a, b) in before[&child].anchors.iter().zip(&after[&child].anchors) {
            assert!(close(sub(b.position, a.position), shift, 1e-9));
            assert!(close(b.handle_out, a.handle_out, 1e-9));
        }
        // Tverrsnittets ramme vrir seg ikke, og undergrenen følger grenen.
        let (f0, f1) = (before[&child].start_frame(), after[&child].start_frame());
        assert!(close(f0.u, f1.u, 1e-9));
        assert!(close(
            sub(after[&gc].point_at(1.0), before[&gc].point_at(1.0)),
            shift,
            1e-9
        ));
    }

    #[test]
    fn bytte_av_orientering_gir_ikke_hopp() {
        let (mut sk, root, child, gc) = three_levels();
        bend_root(&mut sk, root);
        let before = sk.evaluate().unwrap();
        for o in [
            Orientation::KeepDirection,
            Orientation::FollowParent,
            Orientation::KeepDirection,
        ] {
            sk.apply(&SkeletonEdit::SetOrientation {
                spline: child,
                orientation: o,
            })
            .unwrap();
            let after = sk.evaluate().unwrap();
            for id in [child, gc] {
                for (a, b) in before[&id].anchors.iter().zip(&after[&id].anchors) {
                    assert!(close(a.position, b.position, 1e-9));
                    assert!(close(a.handle_in, b.handle_in, 1e-9));
                }
                assert!(close(
                    before[&id].start_frame().u,
                    after[&id].start_frame().u,
                    1e-9
                ));
            }
        }
        // En rotspline har ingen forelder å følge.
        assert!(sk
            .apply(&SkeletonEdit::SetOrientation {
                spline: root,
                orientation: Orientation::KeepDirection
            })
            .is_err());
    }

    #[test]
    fn festet_flyttes_over_paa_en_annen_spline_med_undergrener() {
        let (mut sk, root, child, gc) = three_levels();
        // En annen gren på roten, ut langs -x fra 0,8.
        let other = sk
            .add_child(
                root,
                0.8,
                straight([0.0, 0.0, 80.0], [-1.0, 0.0, 0.0], 30.0),
                4.0,
            )
            .unwrap();
        let before = sk.evaluate().unwrap();
        let angle = |w: &BTreeMap<SplineId, WorldSpline>, parent: SplineId, at: f64| {
            dot(w[&parent].frame_at(at).t, w[&child].frame_at(0.0).t)
        };
        let a0 = angle(&before, root, 0.5);
        // Følg forelder: vinkelen mot den nye forelderen er den samme.
        sk.apply(&SkeletonEdit::Reattach {
            spline: child,
            at: 0.5,
            parent: Some(other),
        })
        .unwrap();
        let after = sk.evaluate().unwrap();
        assert_eq!(sk.spline(child).unwrap().attachment.unwrap().parent, other);
        assert!(close(
            after[&child].point_at(0.0),
            after[&other].point_at(0.5),
            1e-9
        ));
        assert!((angle(&after, other, 0.5) - a0).abs() < 1e-9);
        assert!((after[&child].length - before[&child].length).abs() < 1e-9);
        // Undergrenen er med.
        assert!(close(
            after[&gc].point_at(0.0),
            after[&child].point_at(1.0),
            1e-9
        ));

        // Behold retning: verdensretningen er den samme etter flyttingen.
        sk.apply(&SkeletonEdit::SetOrientation {
            spline: child,
            orientation: Orientation::KeepDirection,
        })
        .unwrap();
        let dir0 = sk.evaluate().unwrap()[&child].frame_at(0.0).t;
        sk.apply(&SkeletonEdit::Reattach {
            spline: child,
            at: 0.2,
            parent: Some(root),
        })
        .unwrap();
        let w = sk.evaluate().unwrap();
        assert!(close(w[&child].frame_at(0.0).t, dir0, 1e-9));
        assert!(close(w[&child].point_at(0.0), w[&root].point_at(0.2), 1e-9));

        // Ingen feste til seg selv eller til egne undergrener, og ingen sløyfer.
        let rejected = |sk: &mut Skeleton, parent| {
            sk.apply(&SkeletonEdit::Reattach {
                spline: child,
                at: 0.5,
                parent: Some(parent),
            })
            .is_err()
        };
        assert!(rejected(&mut sk, child));
        assert!(rejected(&mut sk, gc));
        assert!(sk.validate().is_ok());
        // Lagring og åpning bevarer festet og orienteringen.
        let opened = Skeleton::from_json(&sk.to_json()).unwrap();
        assert_eq!(opened, sk);
    }

    #[test]
    fn registeret_lagrer_profiler_og_volumer_som_kopier() {
        let (mut sk, root, child, _) = three_levels();
        sk.apply(&SkeletonEdit::Batch {
            edits: vec![
                SkeletonEdit::AddProfile {
                    spline: root,
                    at: 0.2,
                    from: None,
                },
                SkeletonEdit::SetProfile {
                    spline: root,
                    station: 0,
                    profile: Profile::h_shape(),
                },
                SkeletonEdit::AddProfile {
                    spline: root,
                    at: 0.8,
                    from: None,
                },
                SkeletonEdit::MakeMaster {
                    spline: root,
                    station: 1,
                },
            ],
        })
        .unwrap();
        sk.apply(&SkeletonEdit::SaveProfile {
            spline: root,
            station: 0,
            name: " H-profil ".into(),
        })
        .unwrap();
        sk.apply(&SkeletonEdit::SaveVolume {
            spline: root,
            name: "Hode".into(),
        })
        .unwrap();
        let reg = sk.registry();
        assert_eq!(reg.len(), 2);
        assert_eq!(reg[0].entry.name, "H-profil");
        assert!(!reg[0].entry.volume);
        assert_eq!(
            reg[0].entry.stations[0].shape,
            ProfileShape::Own {
                profile: Profile::h_shape()
            }
        );
        // Volumet er lagret med egne omriss, ikke masterkobling.
        assert!(reg[1].entry.volume);
        assert_eq!(reg[1].entry.stations.len(), 2);
        assert!(reg[1]
            .entry
            .stations
            .iter()
            .all(|st| matches!(st.shape, ProfileShape::Own { .. })));
        // Bruk volumet på en annen spline: konturene kopieres.
        sk.apply(&SkeletonEdit::SetStations {
            spline: child,
            stations: reg[1].entry.stations.clone(),
        })
        .unwrap();
        assert_eq!(sk.spline(child).unwrap().profiles.len(), 2);
        // En senere masterendring påvirker ikke kopien.
        sk.apply(&SkeletonEdit::SetMaster {
            master: 1,
            profile: Profile::ellipse(1.0, 0.3),
        })
        .unwrap();
        assert_ne!(
            sk.resolved_profiles(child).unwrap()[1].profile,
            Profile::ellipse(1.0, 0.3)
        );
        // Navn, sletting og lagring.
        sk.apply(&SkeletonEdit::RenameEntry {
            entry: reg[1].id,
            name: "Hode 2".into(),
        })
        .unwrap();
        assert!(sk
            .apply(&SkeletonEdit::RenameEntry {
                entry: reg[1].id,
                name: "  ".into()
            })
            .is_err());
        let opened = Skeleton::from_json(&sk.to_json()).unwrap();
        assert_eq!(opened.registry()[1].entry.name, "Hode 2");
        sk.apply(&SkeletonEdit::RemoveEntry { entry: reg[0].id })
            .unwrap();
        assert_eq!(sk.registry().len(), 1);
        // Et volum trenger minst to konturer.
        assert!(sk
            .apply(&SkeletonEdit::SaveVolume {
                spline: 3,
                name: "x".into()
            })
            .is_err());
    }

    #[test]
    fn enden_kobles_tilbake_til_morsplinen_og_folger_med() {
        let (mut sk, root, child, _) = three_levels();
        // Grenen bøyes tilbake og kobles til morsplinen lenger ute.
        sk.apply(&SkeletonEdit::AttachEnd {
            spline: child,
            target: root,
            at: 0.9,
        })
        .unwrap();
        let tip = |sk: &Skeleton| *world_points(sk, child).last().unwrap();
        let target = sk.world_spline(root).unwrap().point_at(0.9);
        assert!(close(tip(&sk), target, 1e-9));
        // Morsplinen flyttes: festet og enden følger.
        sk.apply(&SkeletonEdit::Translate {
            spline: root,
            delta: [3.0, 0.0, -2.0],
        })
        .unwrap();
        let target = sk.world_spline(root).unwrap().point_at(0.9);
        assert!(close(tip(&sk), target, 1e-9));
        // Lagring og åpning beholder koblingen.
        let opened = Skeleton::from_json(&sk.to_json()).unwrap();
        assert_eq!(opened.spline(child).unwrap().end.unwrap().parent, root);
        // Tuppen kan ikke kobles til seg selv eller noe som henger på den.
        assert!(sk
            .apply(&SkeletonEdit::AttachEnd {
                spline: root,
                target: child,
                at: 0.5
            })
            .is_err());
        assert!(sk
            .apply(&SkeletonEdit::AttachEnd {
                spline: child,
                target: child,
                at: 0.5
            })
            .is_err());
        // Å flytte tuppen løsner koblingen.
        let before = tip(&sk);
        sk.apply(&SkeletonEdit::DetachEnd { spline: child })
            .unwrap();
        assert!(close(tip(&sk), before, 1e-9));
        assert!(sk.spline(child).unwrap().end.is_none());
        sk.apply(&SkeletonEdit::AttachEnd {
            spline: child,
            target: root,
            at: 0.9,
        })
        .unwrap();
        sk.apply(&SkeletonEdit::MoveTip {
            spline: child,
            position: [9.0, 9.0, 9.0],
        })
        .unwrap();
        assert!(sk.spline(child).unwrap().end.is_none());
        assert!(close(tip(&sk), [9.0, 9.0, 9.0], 1e-9));
    }

    #[test]
    fn spliner_fra_to_lag_kan_danne_et_nett() {
        let mut sk = Skeleton::new();
        let a = sk
            .add_root(
                straight([0.0; 3], [1.0, 0.0, 0.0], 10.0),
                [0.0, 1.0, 0.0],
                1.0,
            )
            .unwrap();
        let b = sk
            .add_root(
                straight([0.0, 5.0, 0.0], [1.0, 0.0, 0.0], 10.0),
                [0.0, 1.0, 0.0],
                1.0,
            )
            .unwrap();
        sk.apply(&SkeletonEdit::AttachEnd {
            spline: b,
            target: a,
            at: 1.0,
        })
        .unwrap();
        // b henger på a, så a kan ikke kobles til b.
        assert!(sk
            .apply(&SkeletonEdit::AttachEnd {
                spline: a,
                target: b,
                at: 0.0
            })
            .is_err());
        // Fjernes a, blir enden av b stående der den var.
        let tip = *world_points(&sk, b).last().unwrap();
        sk.apply(&SkeletonEdit::Remove { spline: a }).unwrap();
        assert!(sk.spline(b).unwrap().end.is_none());
        assert!(close(*world_points(&sk, b).last().unwrap(), tip, 1e-9));
    }
}
