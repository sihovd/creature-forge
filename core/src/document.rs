//! Dokumentmodellen: varige formoperasjoner med angre og gjør om
//! (spesifikasjonen kap. 19).
//!
//! Dokumentet lagrer operasjonene, ikke bare meshen. Alle former bygges
//! rundt spline-skjelettet: hver rot med sine grener er én kropp, og
//! kroppens mesh genereres rundt splinene (se `surface`). En kule er en kort
//! spline med kulens radius som tykkelse. Uttrekk og innpressing med
//! kontrollpunktene deformerer meshen direkte og lager ingen spliner. De
//! lagres i rammen til nærmeste spline og spilles av på nytt oppå meshen
//! når skjelettet endres, så de følger med når formen flyttes eller bøyes.
//! Hvert ferdig drag er én operasjon og dermed ett angresteg.
//!
//! Et lag kan være en negativ form (`Cut`) som trekkes fra lagene den
//! skjærer i. Kuttet skjer i avstandsfeltet før uttrekkene spilles av;
//! mesh-deler i målet kuttes ikke.

use crate::mesh::{length, normalize, sub, TriMesh, Vec3};
use crate::pull::{apply_pull, Pull};
use crate::skeleton::{bezier_point, Skeleton, SkeletonEdit, SplineId};
use crate::surface::{body_mesh, refine_creases, BodyField};
use std::collections::HashMap;

/// En kropp har samme id som rotsplinen sin.
pub type ShapeId = u32;

/// En kropp i en forhåndsvisning; `None` når kroppen er fjernet.
pub type ShapePreview = (ShapeId, Option<TriMesh>);

/// Returneres av angre og gjør om når skjelettet ble endret.
pub const SKELETON_SHAPE: ShapeId = 0;

/// Lengden på splinen i midten av en ny kule, i mm.
pub const SPHERE_SPLINE_LENGTH: f64 = 0.5;

/// Et uttrekk lagret i rammen til en spline i kroppen. Grepet, normalen og
/// endepunktet er lokale koordinater i rammen i andelen `at` av splinens
/// buelengde, så uttrekket følger når splinen flyttes, bøyes eller endres.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BoundPull {
    pub spline: SplineId,
    pub at: f64,
    pub anchor: Vec3,
    pub normal: Vec3,
    pub tip: Vec3,
    pub radius: f64,
    /// Splinens tykkelse da uttrekket ble gjort.
    pub thickness: f64,
}

/// Punkter per segment når nærmeste spline til et grep finnes.
const BIND_POINTS_PER_SEGMENT: usize = 24;

impl BoundPull {
    /// Knytter et uttrekk til splinen i kroppen `root` som ligger nærmest
    /// grepets overflate.
    pub fn bind(skeleton: &Skeleton, root: SplineId, pull: &Pull) -> Result<BoundPull, String> {
        let (_, spline, approx) = nearest(skeleton, root, pull.anchor)?;
        // Andelen er bare et referansepunkt; buelengdeandelen gjøres eksakt
        // av rammen selv, så samme andel gir samme ramme ved avspilling.
        let at = approx.clamp(0.0, 1.0);
        let frame = skeleton.world_spline(spline)?.frame_at(at);
        Ok(BoundPull {
            spline,
            at,
            anchor: frame.to_local(pull.anchor),
            normal: frame.dir_to_local(pull.normal),
            tip: frame.to_local(pull.tip),
            radius: pull.radius,
            thickness: skeleton.spline(spline).map_or(0.0, |s| s.thickness),
        })
    }

    /// Uttrekket i verdenskoordinater for skjelettet slik det er nå, eller
    /// `None` når splinen er fjernet. Endret tykkelse flytter grepet langs
    /// normalen, så uttrekket blir like langt fra den nye overflaten.
    pub fn resolve(&self, skeleton: &Skeleton) -> Option<Pull> {
        let spline = skeleton.spline(self.spline)?;
        let frame = skeleton.world_spline(self.spline).ok()?.frame_at(self.at);
        let shift = crate::curve::scale(self.normal, spline.thickness - self.thickness);
        let anchor = crate::curve::add(self.anchor, shift);
        let tip = crate::curve::add(self.tip, shift);
        Some(Pull {
            anchor: frame.to_world(anchor),
            normal: normalize(frame.dir_to_world(self.normal)),
            tip: frame.to_world(tip),
            radius: self.radius,
        })
    }
}

/// Nærmeste spline i kroppen `root` til overflaten ved `p`: avstanden,
/// splinen og andelen langs den.
fn nearest(skeleton: &Skeleton, root: SplineId, p: Vec3) -> Result<(f64, SplineId, f64), String> {
    let mut best: Option<(f64, SplineId, f64)> = None;
    for id in skeleton.descendants(root) {
        let w = skeleton.world_spline(id)?;
        let thickness = skeleton.spline(id).map_or(0.0, |s| s.thickness);
        let n = w.segments.len() * BIND_POINTS_PER_SEGMENT;
        for i in 0..=n {
            let u = i as f64 / n as f64;
            let k = (i / BIND_POINTS_PER_SEGMENT).min(w.segments.len() - 1);
            let t = (i - k * BIND_POINTS_PER_SEGMENT) as f64 / BIND_POINTS_PER_SEGMENT as f64;
            let d = length(sub(p, bezier_point(&w.segments[k], t))) - thickness;
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, id, u));
            }
        }
    }
    best.ok_or_else(|| "Kroppen har ingen spline".into())
}

/// Speilbildet av et uttrekk om x = 0 når speilmodus er på. Et uttrekk på
/// selve speilplanet speiles ikke, så det ikke trekkes ut to ganger.
fn mirrored_pull(skeleton: &Skeleton, p: &Pull) -> Option<Pull> {
    if !skeleton.mirror() || p.anchor[0].abs() < p.radius * 0.5 {
        return None;
    }
    let m = |v: Vec3| [-v[0], v[1], v[2]];
    Some(Pull {
        anchor: m(p.anchor),
        normal: m(p.normal),
        tip: m(p.tip),
        radius: p.radius,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Pull {
        shape: ShapeId,
        pull: BoundPull,
    },
    /// Skjelettet etter en redigering. Hele tilstanden lagres, så angre
    /// gjenoppretter forrige skjelett nøyaktig.
    Skeleton {
        skeleton: Skeleton,
    },
}

impl Operation {
    pub fn shape(&self) -> ShapeId {
        match self {
            Operation::Pull { shape, .. } => *shape,
            Operation::Skeleton { .. } => SKELETON_SHAPE,
        }
    }
}

pub struct Document {
    /// Ønsket kantlengde i mm for nye og fortettede områder.
    pub target_edge: f64,
    /// Tilstanden fra en åpnet prosjektfil. Historikken bygger på den, så
    /// åpningen i seg selv kan ikke angres.
    base_skeleton: Skeleton,
    base_pulls: Vec<(ShapeId, BoundPull)>,
    history: Vec<Operation>,
    redo: Vec<Operation>,
    meshes: HashMap<ShapeId, TriMesh>,
    skeleton: Skeleton,
    revision: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self::new(1.5)
    }
}

impl Document {
    pub fn new(target_edge: f64) -> Self {
        Document {
            target_edge,
            base_skeleton: Skeleton::new(),
            base_pulls: Vec::new(),
            history: Vec::new(),
            redo: Vec::new(),
            meshes: HashMap::new(),
            skeleton: Skeleton::new(),
            revision: 0,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn operations(&self) -> &[Operation] {
        &self.history
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn mesh(&self, shape: ShapeId) -> Option<&TriMesh> {
        self.meshes.get(&shape)
    }

    pub fn shape_ids(&self) -> Vec<ShapeId> {
        let mut ids: Vec<_> = self.meshes.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    /// Ny kule: en kort, loddrett spline i sentrum med radien som tykkelse.
    pub fn add_sphere(&mut self, center: Vec3, radius: f64) -> Result<ShapeId, String> {
        let half = SPHERE_SPLINE_LENGTH / 2.0;
        let id = self.commit_skeleton(&SkeletonEdit::AddRoot {
            start: [center[0], center[1] - half, center[2]],
            direction: [0.0, 1.0, 0.0],
            length: SPHERE_SPLINE_LENGTH,
            thickness: radius,
        })?;
        id.ok_or_else(|| "Kula fikk ingen spline".into())
    }

    /// Skjelettet og de berørte kroppene med en redigering som ikke lagres
    /// (forhåndsvisning under drag). Meshene beregnes med grovere oppløsning.
    pub fn preview_skeleton(
        &self,
        edit: &SkeletonEdit,
    ) -> Result<(Skeleton, Vec<ShapePreview>), String> {
        let mut next = self.skeleton.clone();
        next.apply(edit)?;
        let h = self.target_edge * PREVIEW_EDGE_FACTOR;
        let mut meshes = Vec::new();
        for root in changed_roots(&self.skeleton, &next) {
            let mesh = if next.spline(root).is_some() {
                Some(self.shape_mesh(&next, root, h, self.history.len(), false)?)
            } else {
                None
            };
            meshes.push((root, mesh));
        }
        Ok((next, meshes))
    }

    /// Utfører en redigering av skjelettet som ett angresteg.
    pub fn commit_skeleton(&mut self, edit: &SkeletonEdit) -> Result<Option<SplineId>, String> {
        let mut next = self.skeleton.clone();
        let created = next.apply(edit)?;
        let old = std::mem::replace(&mut self.skeleton, next.clone());
        if let Err(e) = self.rebuild_changed(&old) {
            self.skeleton = old;
            self.rebuild_changed(&next)?;
            return Err(e);
        }
        self.history.push(Operation::Skeleton { skeleton: next });
        self.redo.clear();
        self.revision += 1;
        Ok(created)
    }

    /// Beregner resultatet av et uttrekk uten å endre dokumentet (forhåndsvisning under drag).
    pub fn preview_pull(&self, shape: ShapeId, pull: &Pull) -> Result<TriMesh, String> {
        let mesh = self.meshes.get(&shape).ok_or("Ukjent form")?;
        apply_pull(mesh, pull, self.target_edge)
    }

    /// Uttrekk eller innpressing med et kontrollpunkt. Lager ingen spline.
    pub fn commit_pull(&mut self, shape: ShapeId, pull: Pull) -> Result<(), String> {
        pull.validate()?;
        if self.skeleton.is_locked(shape) {
            return Err("Laget er låst".into());
        }
        // Med speiling knyttes uttrekket til siden der skjelettet er, så det
        // følger splinene på begge sider.
        let mut pull = pull;
        if let Some(m) = mirrored_pull(&self.skeleton, &pull) {
            let d = |p: &Pull| nearest(&self.skeleton, shape, p.anchor).map(|n| n.0);
            if d(&m)? < d(&pull)? {
                pull = m;
            }
        }
        let bound = BoundPull::bind(&self.skeleton, shape, &pull)?;
        // Bruk uttrekket slik det spilles av, så angre og gjør om gir samme mesh.
        let resolved = bound.resolve(&self.skeleton).ok_or("Ukjent form")?;
        let mut mesh = self.preview_pull(shape, &resolved)?;
        if let Some(m) = mirrored_pull(&self.skeleton, &resolved) {
            mesh = apply_pull(&mesh, &m, self.target_edge)?;
        }
        let pull = bound;
        self.meshes.insert(shape, mesh);
        self.history.push(Operation::Pull { shape, pull });
        self.redo.clear();
        self.revision += 1;
        Ok(())
    }

    /// Angrer siste operasjon. Returnerer formen som ble endret, eller
    /// `SKELETON_SHAPE` når skjelettet ble endret.
    pub fn undo(&mut self) -> Option<ShapeId> {
        let op = self.history.pop()?;
        let shape = op.shape();
        self.redo.push(op);
        self.after_history_change(shape);
        Some(shape)
    }

    pub fn redo(&mut self) -> Option<ShapeId> {
        let op = self.redo.pop()?;
        let shape = op.shape();
        self.history.push(op);
        self.after_history_change(shape);
        Some(shape)
    }

    fn after_history_change(&mut self, shape: ShapeId) {
        // Operasjonene var gyldige da de ble lagt inn, og beregningen er deterministisk.
        if shape == SKELETON_SHAPE {
            let current = self.current_skeleton();
            let old = std::mem::replace(&mut self.skeleton, current);
            let _ = self.rebuild_changed(&old);
        } else if let Ok(m) = self.shape_mesh(
            &self.skeleton,
            shape,
            self.target_edge,
            self.history.len(),
            true,
        ) {
            self.meshes.insert(shape, m);
        }
        self.revision += 1;
    }

    fn current_skeleton(&self) -> Skeleton {
        self.history
            .iter()
            .rev()
            .find_map(|op| match op {
                Operation::Skeleton { skeleton } => Some(skeleton.clone()),
                _ => None,
            })
            .unwrap_or_else(|| self.base_skeleton.clone())
    }

    /// Prosjektfilen: skjelettet med lag, konturer og mastere, og uttrekkene
    /// slik de står nå. Historikken lagres ikke.
    pub fn to_project_json(&self) -> String {
        let pulls = self
            .base_pulls
            .iter()
            .copied()
            .chain(self.history.iter().filter_map(|op| match op {
                Operation::Pull { shape, pull } => Some((*shape, *pull)),
                _ => None,
            }))
            .map(|(shape, pull)| SavedPull { shape, pull })
            .collect();
        let file = ProjectFile {
            format: PROJECT_FORMAT.into(),
            version: PROJECT_VERSION,
            skeleton: self.skeleton.clone(),
            pulls,
        };
        serde_json::to_string_pretty(&file).expect("prosjektet kan alltid serialiseres")
    }

    /// Åpner en prosjektfil i et nytt dokument med tom historikk.
    pub fn open_project(text: &str, target_edge: f64) -> Result<Document, String> {
        let file: ProjectFile =
            serde_json::from_str(text).map_err(|e| format!("Kunne ikke lese prosjektet: {e}"))?;
        if file.format != PROJECT_FORMAT {
            return Err(format!("Ukjent format: {}", file.format));
        }
        if file.version != PROJECT_VERSION {
            return Err(format!("Ukjent prosjektversjon: {}", file.version));
        }
        file.skeleton.validate()?;
        let mut doc = Document::new(target_edge);
        doc.base_skeleton = file.skeleton.clone();
        doc.base_pulls = file.pulls.into_iter().map(|p| (p.shape, p.pull)).collect();
        doc.skeleton = file.skeleton;
        doc.rebuild_changed(&Skeleton::new())?;
        Ok(doc)
    }

    /// Meshen til kroppen `root`: generert rundt skjelettet, med uttrekkene
    /// blant de første `upto` operasjonene spilt av oppå.
    fn shape_mesh(
        &self,
        skeleton: &Skeleton,
        root: ShapeId,
        h: f64,
        upto: usize,
        refine: bool,
    ) -> Result<TriMesh, String> {
        let mut mesh = body_mesh(skeleton, root, h)?;
        let settings = skeleton.settings();
        if refine && settings.crease_detail > 0 {
            let field = BodyField::new(skeleton, root, h)?;
            refine_creases(
                &mut mesh,
                &field,
                h,
                settings.crease_detail,
                settings.crease_angle,
            );
        }
        let world = skeleton.evaluate()?;
        let mut parts = crate::parts::body_parts(skeleton, &world, root);
        if skeleton.mirror() {
            parts = {
                let mut both = parts.clone();
                crate::parts::append(&mut both, &crate::parts::mirrored(&parts));
                both
            };
        }
        crate::parts::append(&mut mesh, &parts);
        let history = self.history[..upto].iter().filter_map(|op| match op {
            Operation::Pull { shape, pull } => Some((shape, pull)),
            _ => None,
        });
        for (shape, pull) in self.base_pulls.iter().map(|(s, p)| (s, p)).chain(history) {
            if *shape == root {
                if let Some(p) = pull.resolve(skeleton) {
                    mesh = apply_pull(&mesh, &p, self.target_edge)?;
                    if let Some(m) = mirrored_pull(skeleton, &p) {
                        mesh = apply_pull(&mesh, &m, self.target_edge)?;
                    }
                }
            }
        }
        Ok(mesh)
    }

    /// Beregner kroppene som er endret fra `old` til gjeldende skjelett.
    fn rebuild_changed(&mut self, old: &Skeleton) -> Result<(), String> {
        let skeleton = self.skeleton.clone();
        for root in changed_roots(old, &skeleton) {
            if skeleton.spline(root).is_some() {
                let m =
                    self.shape_mesh(&skeleton, root, self.target_edge, self.history.len(), true)?;
                self.meshes.insert(root, m);
            } else {
                self.meshes.remove(&root);
            }
        }
        Ok(())
    }
}

const PROJECT_FORMAT: &str = "creature-forge-project";
const PROJECT_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedPull {
    shape: ShapeId,
    pull: BoundPull,
}

/// Prosjektfilen (.cfproj).
#[derive(serde::Serialize, serde::Deserialize)]
struct ProjectFile {
    format: String,
    version: u32,
    skeleton: Skeleton,
    pulls: Vec<SavedPull>,
}

/// Grovere rutenett under drag, så forhåndsvisningen holder følge med pekeren.
const PREVIEW_EDGE_FACTOR: f64 = 2.0;

/// Røttene der hierarkiet er nytt, fjernet eller endret. En kropp er også
/// endret når en spline i den flytter seg fordi en annen kropp endres (en
/// ende koblet dit), og alle er endret når noe felles endres (mastere,
/// speiling, mesh-deler).
fn changed_roots(old: &Skeleton, new: &Skeleton) -> Vec<SplineId> {
    let roots = |s: &Skeleton| -> Vec<SplineId> {
        s.ids()
            .into_iter()
            .filter(|id| s.spline(*id).is_some_and(|sp| sp.attachment.is_none()))
            .collect()
    };
    let (wold, wnew) = (
        old.evaluate().unwrap_or_default(),
        new.evaluate().unwrap_or_default(),
    );
    type Body = Vec<(
        SplineId,
        crate::skeleton::Spline,
        Option<Vec<crate::skeleton::Anchor>>,
    )>;
    let body = |s: &Skeleton,
                w: &std::collections::BTreeMap<SplineId, crate::skeleton::WorldSpline>,
                root: SplineId|
     -> Body {
        s.descendants(root)
            .into_iter()
            .filter_map(|id| {
                s.spline(id)
                    .map(|sp| (id, sp.clone(), w.get(&id).map(|x| x.anchors.clone())))
            })
            .collect()
    };
    let shared = old.shared_differs(new);
    let mut out: Vec<SplineId> = roots(old);
    out.extend(roots(new));
    out.sort_unstable();
    out.dedup();
    out.retain(|r| shared || body(old, &wold, *r) != body(new, &wnew, *r));
    // En negativ form som endres, endrer lagene den skjærer i, før og etter.
    let targets = |s: &Skeleton, r: SplineId| -> Vec<SplineId> {
        s.spline(r)
            .and_then(|sp| sp.cut.as_ref())
            .map_or_else(Vec::new, |c| c.targets.clone())
    };
    let mut cut: Vec<SplineId> = out
        .iter()
        .flat_map(|r| {
            let mut t = targets(old, *r);
            t.extend(targets(new, *r));
            t
        })
        .filter(|t| new.spline(*t).is_some())
        .collect();
    out.append(&mut cut);
    out.sort_unstable();
    out.dedup();
    out
}

/// Kontrollpunkter fordelt over overflaten med omtrent `spacing` mm mellom seg.
/// Grådig utvalg av meshpunkter med et romlig rutenett; returnerer (posisjon, normal).
pub fn control_points(mesh: &TriMesh, spacing: f64) -> Vec<(Vec3, Vec3)> {
    let normals = mesh.vertex_normals();
    let cell = spacing.max(1e-3);
    let key = |p: Vec3| {
        (
            (p[0] / cell).floor() as i64,
            (p[1] / cell).floor() as i64,
            (p[2] / cell).floor() as i64,
        )
    };
    let mut grid: HashMap<(i64, i64, i64), Vec<Vec3>> = HashMap::new();
    let mut out = Vec::new();
    for (i, &p) in mesh.positions.iter().enumerate() {
        let (x, y, z) = key(p);
        let near = (-1..=1).any(|dx| {
            (-1..=1).any(|dy| {
                (-1..=1).any(|dz| {
                    grid.get(&(x + dx, y + dy, z + dz))
                        .is_some_and(|v| v.iter().any(|&q| length(sub(p, q)) < spacing))
                })
            })
        });
        if !near {
            grid.entry((x, y, z)).or_default().push(p);
            out.push((p, normalize(normals[i])));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::{Orientation, SkeletonEdit as E};
    use crate::sphere::icosphere_for_edge;

    fn pull_on(doc: &Document, shape: ShapeId, distance: f64) -> Pull {
        let anchor = doc.mesh(shape).unwrap().positions[0];
        let direction = normalize(sub(anchor, [0.0, 25.0, 0.0]));
        Pull::straight(anchor, direction, distance, 10.0)
    }

    #[test]
    fn kula_er_en_mesh_rundt_en_spline_i_midten() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 25.0).unwrap();
        assert_eq!(doc.skeleton().ids(), vec![s]);
        let w = doc.skeleton().world_spline(s).unwrap();
        assert!(length(sub(w.point_at(0.5), [0.0, 25.0, 0.0])) < 1e-9);
        let m = doc.mesh(s).unwrap();
        assert!(m.is_closed_manifold());
        for p in &m.positions {
            assert!((length(sub(*p, [0.0, 25.0, 0.0])) - 25.0).abs() < 1.2);
        }
    }

    #[test]
    fn angre_og_gjoer_om_gjenoppretter_samme_geometri() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 25.0).unwrap();
        let before = doc.mesh(s).unwrap().clone();
        doc.commit_pull(s, pull_on(&doc, s, 15.0)).unwrap();
        let after = doc.mesh(s).unwrap().clone();
        assert_ne!(before.positions.len(), after.positions.len());

        assert_eq!(doc.undo(), Some(s));
        assert_eq!(doc.mesh(s).unwrap().positions, before.positions);
        assert_eq!(doc.redo(), Some(s));
        assert_eq!(doc.mesh(s).unwrap().positions, after.positions);
        assert_eq!(doc.mesh(s).unwrap().triangles, after.triangles);
    }

    #[test]
    fn prikkene_lager_ingen_spliner() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 25.0).unwrap();
        let skeleton = doc.skeleton().clone();
        doc.commit_pull(s, pull_on(&doc, s, 15.0)).unwrap();
        doc.commit_pull(s, pull_on(&doc, s, -4.0)).unwrap();
        assert_eq!(doc.skeleton(), &skeleton);
    }

    #[test]
    fn angre_av_ny_kule_fjerner_formen_og_ny_operasjon_toemmer_gjoer_om() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 25.0).unwrap();
        doc.undo();
        assert!(doc.mesh(s).is_none());
        assert!(doc.skeleton().ids().is_empty());
        assert!(doc.can_redo());
        doc.add_sphere([0.0, 25.0, 0.0], 10.0).unwrap();
        assert!(!doc.can_redo());
    }

    #[test]
    fn forhaandsvisning_endrer_ikke_dokumentet() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 25.0).unwrap();
        let rev = doc.revision();
        let preview = doc.preview_pull(s, &pull_on(&doc, s, 10.0)).unwrap();
        assert!(preview.is_closed_manifold());
        let (_, meshes) = doc
            .preview_skeleton(&E::Extend {
                spline: s,
                delta: 20.0,
            })
            .unwrap();
        assert_eq!(meshes.len(), 1);
        assert_eq!(doc.revision(), rev);
        assert_eq!(doc.operations().len(), 1);
    }

    #[test]
    fn endring_av_splinen_endrer_meshen_og_beholder_uttrekkene() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 20.0).unwrap();
        let pull = pull_on(&doc, s, 12.0);
        doc.commit_pull(s, pull).unwrap();
        let pulled = doc.mesh(s).unwrap().signed_volume();
        // Trekk kulas spline ut i en avlang kropp.
        doc.commit_skeleton(&E::Extend {
            spline: s,
            delta: 30.0,
        })
        .unwrap();
        let m = doc.mesh(s).unwrap();
        let top = m.positions.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        assert!(top > 70.0, "{top}");
        assert!(m.signed_volume() > pulled);
        // Uttrekket spilles av på nytt: angre splinen gir samme mesh som før.
        let with_pull = doc.mesh(s).unwrap().clone();
        doc.undo();
        assert!((doc.mesh(s).unwrap().signed_volume() - pulled).abs() < 1e-6);
        doc.redo();
        assert_eq!(doc.mesh(s).unwrap().positions, with_pull.positions);
    }

    #[test]
    fn uttrekkene_folger_formen_naar_den_flyttes_og_boeyes() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 20.0).unwrap();
        // Uttrekk rett ut til siden (+x) på kula.
        let side = [20.0, 25.25, 0.0];
        doc.commit_pull(s, Pull::straight(side, [1.0, 0.0, 0.0], 15.0, 10.0))
            .unwrap();
        let tip_x = |m: &TriMesh| m.positions.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        let before = doc.mesh(s).unwrap().clone();
        assert!((tip_x(&before) - 35.0).abs() < 1.5, "{}", tip_x(&before));

        // Flytt hele formen 30 mm i z: uttrekket følger med.
        let w = doc.skeleton().world_spline(s).unwrap();
        let moved: Vec<_> = w
            .anchors
            .iter()
            .map(|a| [a.position[0], a.position[1], a.position[2] + 30.0])
            .collect();
        doc.commit_skeleton(&E::Batch {
            edits: moved
                .iter()
                .enumerate()
                .map(|(index, position)| E::MoveAnchor {
                    spline: s,
                    index,
                    position: *position,
                })
                .collect(),
        })
        .unwrap();
        let after = doc.mesh(s).unwrap();
        let bbox = |m: &TriMesh| {
            let mut lo = [f64::MAX; 3];
            let mut hi = [f64::MIN; 3];
            for p in &m.positions {
                for a in 0..3 {
                    lo[a] = lo[a].min(p[a]);
                    hi[a] = hi[a].max(p[a]);
                }
            }
            (lo, hi)
        };
        let ((lo0, hi0), (lo1, hi1)) = (bbox(&before), bbox(after));
        for a in 0..3 {
            let shift = if a == 2 { 30.0 } else { 0.0 };
            assert!((lo1[a] - lo0[a] - shift).abs() < 0.5, "{a}");
            assert!((hi1[a] - hi0[a] - shift).abs() < 0.5, "{a}");
        }
        assert!(
            (after.signed_volume() - before.signed_volume()).abs() / before.signed_volume() < 0.01
        );

        // Tykkere spline: uttrekket står like langt ut fra den nye overflaten.
        doc.commit_skeleton(&E::SetThickness {
            spline: s,
            thickness: 25.0,
        })
        .unwrap();
        let m = doc.mesh(s).unwrap();
        assert!((tip_x(m) - 40.0).abs() < 1.5, "{}", tip_x(m));
    }

    #[test]
    fn kontrollpunkter_holder_avstand_og_dekker_overflaten() {
        let mesh = icosphere_for_edge(25.0, 1.5).unwrap();
        let spacing = 6.0;
        let pts = control_points(&mesh, spacing);
        for (i, a) in pts.iter().enumerate() {
            for b in &pts[i + 1..] {
                assert!(length(sub(a.0, b.0)) >= spacing);
            }
        }
        // Hvert meshpunkt har et kontrollpunkt innenfor avstanden.
        for p in &mesh.positions {
            assert!(pts.iter().any(|c| length(sub(*p, c.0)) < spacing));
        }
        // Færre punkter med større avstand (CF02: avstanden styrer bare grepene).
        assert!(control_points(&mesh, 12.0).len() < pts.len());
    }

    #[test]
    fn grener_er_del_av_samme_kropp_og_kan_angres() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 20.0).unwrap();
        let volume = doc.mesh(s).unwrap().signed_volume();
        let child = doc
            .commit_skeleton(&E::AddChild {
                parent: s,
                at: 1.0,
                direction: [1.0, 0.0, 0.0],
                length: 35.0,
                thickness: 5.0,
                orientation: Orientation::FollowParent,
            })
            .unwrap()
            .unwrap();
        assert_eq!(doc.shape_ids(), vec![s]);
        assert!(doc.mesh(s).unwrap().signed_volume() > volume);
        assert_eq!(doc.undo(), Some(SKELETON_SHAPE));
        assert_eq!(doc.skeleton().ids(), vec![s]);
        assert!((doc.mesh(s).unwrap().signed_volume() - volume).abs() < 1e-6);
        doc.redo();
        assert_eq!(doc.skeleton().ids(), vec![s, child]);
        // En ugyldig redigering lagres ikke.
        let rev = doc.revision();
        assert!(doc.commit_skeleton(&E::Remove { spline: 99 }).is_err());
        assert_eq!(doc.revision(), rev);
    }

    #[test]
    fn prosjektfil_bevarer_lag_hierarki_konturer_og_uttrekk() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 20.0).unwrap();
        let child = doc
            .commit_skeleton(&E::AddChild {
                parent: s,
                at: 1.0,
                direction: [1.0, 0.0, 0.0],
                length: 35.0,
                thickness: 5.0,
                orientation: Orientation::FollowParent,
            })
            .unwrap()
            .unwrap();
        doc.commit_skeleton(&E::Batch {
            edits: vec![
                E::Rename {
                    spline: s,
                    name: "Kropp".into(),
                },
                E::Rename {
                    spline: child,
                    name: "Hals".into(),
                },
                E::AddProfile {
                    spline: child,
                    at: 0.5,
                    from: None,
                },
                E::MakeMaster {
                    spline: child,
                    station: 0,
                },
            ],
        })
        .unwrap();
        doc.commit_pull(s, pull_on(&doc, s, 8.0)).unwrap();
        doc.commit_skeleton(&E::SetLayer {
            spline: s,
            hidden: true,
            locked: true,
        })
        .unwrap();
        let text = doc.to_project_json();
        let opened = Document::open_project(&text, 2.0).unwrap();
        assert_eq!(opened.skeleton(), doc.skeleton());
        assert_eq!(
            opened.mesh(s).unwrap().positions,
            doc.mesh(s).unwrap().positions
        );
        assert!(!opened.can_undo());
        assert_eq!(opened.skeleton().spline(child).unwrap().name, "Hals");
        assert!(opened.skeleton().is_locked(child));
        // Uttrekket er med, og en ny endring etter åpning kan angres tilbake til filen.
        let mut opened = opened;
        opened
            .commit_skeleton(&E::SetLayer {
                spline: s,
                hidden: false,
                locked: false,
            })
            .unwrap();
        opened.commit_pull(s, pull_on(&opened, s, 4.0)).unwrap();
        opened.undo();
        opened.undo();
        assert_eq!(opened.skeleton(), doc.skeleton());
        assert_eq!(
            opened.mesh(s).unwrap().positions,
            doc.mesh(s).unwrap().positions
        );
        assert!(Document::open_project("{}", 2.0).is_err());
    }

    #[test]
    fn laast_lag_kan_ikke_redigeres_men_kan_laases_opp() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 20.0).unwrap();
        let other = doc.add_sphere([60.0, 25.0, 0.0], 10.0).unwrap();
        doc.commit_skeleton(&E::SetLayer {
            spline: s,
            hidden: false,
            locked: true,
        })
        .unwrap();
        let rev = doc.revision();
        assert!(doc
            .commit_skeleton(&E::Extend {
                spline: s,
                delta: 10.0
            })
            .is_err());
        assert!(doc
            .commit_skeleton(&E::AddChild {
                parent: s,
                at: 0.5,
                direction: [1.0, 0.0, 0.0],
                length: 30.0,
                thickness: 4.0,
                orientation: Orientation::FollowParent,
            })
            .is_err());
        assert!(doc.commit_pull(s, pull_on(&doc, s, 5.0)).is_err());
        // En samlet redigering med et låst lag i seg avvises i sin helhet.
        assert!(doc
            .commit_skeleton(&E::Batch {
                edits: vec![
                    E::Extend {
                        spline: other,
                        delta: 5.0
                    },
                    E::Remove { spline: s },
                ],
            })
            .is_err());
        assert_eq!(doc.revision(), rev);
        // Andre lag kan fortsatt redigeres, og laget kan låses opp og gis navn.
        doc.commit_skeleton(&E::Extend {
            spline: other,
            delta: 5.0,
        })
        .unwrap();
        doc.commit_skeleton(&E::Rename {
            spline: s,
            name: "Kropp".into(),
        })
        .unwrap();
        doc.commit_skeleton(&E::SetLayer {
            spline: s,
            hidden: false,
            locked: false,
        })
        .unwrap();
        doc.commit_skeleton(&E::Extend {
            spline: s,
            delta: 10.0,
        })
        .unwrap();
        // Grener kan ikke skjules eller låses hver for seg.
        let child = doc
            .commit_skeleton(&E::AddChild {
                parent: s,
                at: 0.5,
                direction: [1.0, 0.0, 0.0],
                length: 30.0,
                thickness: 4.0,
                orientation: Orientation::FollowParent,
            })
            .unwrap()
            .unwrap();
        assert!(doc
            .commit_skeleton(&E::SetLayer {
                spline: child,
                hidden: true,
                locked: false
            })
            .is_err());
    }

    #[test]
    fn mesh_deler_settes_inn_paa_splinen_og_kan_importeres() {
        use crate::skeleton::{Part, PartSource};
        let mut doc = Document::new(2.0);
        let id = doc
            .commit_skeleton(&E::AddRoot {
                start: [0.0; 3],
                direction: [0.0, 1.0, 0.0],
                length: 40.0,
                thickness: 6.0,
            })
            .unwrap()
            .unwrap();
        let plain = doc.mesh(id).unwrap().triangles.len();
        let horn = Part {
            source: PartSource::Builtin {
                name: "Horn".into(),
            },
            at: 0.5,
            size: 15.0,
            rotation: 0.0,
            outward: true,
        };
        doc.commit_skeleton(&E::AddPart {
            spline: id,
            part: horn.clone(),
            toward: Some([0.0, 0.0, 1.0]),
        })
        .unwrap();
        let m = doc.mesh(id).unwrap();
        assert!(m.triangles.len() > plain);
        // Hornet stikker ut fra overflaten, mot +z.
        let reach = m.positions.iter().map(|p| p[2]).fold(0.0, f64::max);
        assert!(reach > 6.0 + 10.0, "{reach}");
        // Importert mesh: et tetraeder i registeret, brukt på splinen.
        doc.commit_skeleton(&E::ImportPart {
            name: "Tetra".into(),
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
            triangles: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        })
        .unwrap();
        let lib = doc.skeleton().library();
        assert_eq!(lib.parts.len(), crate::parts::BUILTIN.len() + 1);
        let stored = lib.parts.last().unwrap().source.clone();
        doc.commit_skeleton(&E::AddPart {
            spline: id,
            part: Part {
                source: stored.clone(),
                ..horn.clone()
            },
            toward: None,
        })
        .unwrap();
        assert!(doc
            .commit_skeleton(&E::AddPart {
                spline: id,
                part: Part {
                    source: PartSource::Builtin {
                        name: "Vinge".into()
                    },
                    ..horn
                },
                toward: None,
            })
            .is_err());
        // Lagres i prosjektfilen.
        let opened = Document::open_project(&doc.to_project_json(), 2.0).unwrap();
        assert_eq!(opened.skeleton().spline(id).unwrap().parts.len(), 2);
        // Fjernes meshen fra registeret, forsvinner delene som bruker den.
        let PartSource::Stored { id: mesh_id } = stored else {
            panic!()
        };
        doc.commit_skeleton(&E::RemovePartMesh { id: mesh_id })
            .unwrap();
        assert_eq!(doc.skeleton().spline(id).unwrap().parts.len(), 1);
    }

    #[test]
    fn speilmodus_speiler_uttrekk_og_kan_angres() {
        let mut doc = Document::new(2.0);
        let id = doc
            .commit_skeleton(&E::AddRoot {
                start: [15.0, 0.0, 0.0],
                direction: [0.0, 1.0, 0.0],
                length: 20.0,
                thickness: 6.0,
            })
            .unwrap()
            .unwrap();
        let one = doc.mesh(id).unwrap().signed_volume();
        doc.commit_skeleton(&E::SetMirror { mirror: true }).unwrap();
        let two = doc.mesh(id).unwrap().signed_volume();
        assert!((two - 2.0 * one).abs() < 0.03 * two);
        // Et uttrekk på originalsiden gir et likt uttrekk på speilsiden.
        let pull = Pull::straight([21.0, 10.0, 0.0], [1.0, 0.0, 0.0], 5.0, 4.0);
        doc.commit_pull(id, pull).unwrap();
        let m = doc.mesh(id).unwrap();
        let max_x = m.positions.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        let min_x = m.positions.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        assert!(max_x > 23.0 && min_x < -23.0, "{min_x} {max_x}");
        assert!((max_x + min_x).abs() < 0.5, "{min_x} {max_x}");
        doc.undo();
        doc.undo();
        assert!(!doc.skeleton().mirror());
        assert!((doc.mesh(id).unwrap().signed_volume() - one).abs() < 1e-9);
    }

    #[test]
    fn konturen_kan_flyttes_over_paa_en_annen_spline() {
        let mut doc = Document::new(2.0);
        let root = doc
            .commit_skeleton(&E::AddRoot {
                start: [0.0; 3],
                direction: [0.0, 1.0, 0.0],
                length: 40.0,
                thickness: 5.0,
            })
            .unwrap()
            .unwrap();
        let child = doc
            .commit_skeleton(&E::AddChild {
                parent: root,
                at: 0.5,
                direction: [1.0, 0.0, 0.0],
                length: 20.0,
                thickness: 3.0,
                orientation: Orientation::FollowParent,
            })
            .unwrap()
            .unwrap();
        doc.commit_skeleton(&E::AddProfile {
            spline: root,
            at: 0.2,
            from: None,
        })
        .unwrap();
        doc.commit_skeleton(&E::MoveProfile {
            spline: root,
            station: 0,
            to: child,
            at: 0.6,
        })
        .unwrap();
        assert!(doc.skeleton().spline(root).unwrap().profiles.is_empty());
        assert_eq!(doc.skeleton().spline(child).unwrap().profiles[0].at, 0.6);
    }

    fn cut(targets: Vec<ShapeId>) -> crate::skeleton::Cut {
        crate::skeleton::Cut {
            targets,
            active: true,
            sharp: false,
        }
    }

    /// To kuler side om side og en kutter som overlapper den første.
    fn two_balls_and_cutter(doc: &mut Document) -> (ShapeId, ShapeId, ShapeId) {
        let a = doc.add_sphere([0.0, 25.0, 0.0], 15.0).unwrap();
        let b = doc.add_sphere([60.0, 25.0, 0.0], 15.0).unwrap();
        let c = doc.add_sphere([14.0, 25.0, 0.0], 8.0).unwrap();
        (a, b, c)
    }

    #[test]
    fn negativ_form_kutter_bare_maalet_og_kan_angres_og_lagres() {
        let mut doc = Document::new(1.5);
        let (a, b, c) = two_balls_and_cutter(&mut doc);
        let (va, vb) = (
            doc.mesh(a).unwrap().signed_volume(),
            doc.mesh(b).unwrap().signed_volume(),
        );
        doc.commit_skeleton(&E::SetCut {
            spline: c,
            cut: Some(cut(vec![a])),
        })
        .unwrap();
        let cut_a = doc.mesh(a).unwrap().signed_volume();
        assert!(cut_a < va * 0.95, "{cut_a} {va}");
        assert_eq!(doc.mesh(b).unwrap().signed_volume(), vb);
        // Kutteren er fortsatt et eget, redigerbart lag med egen mesh.
        assert!(doc.mesh(c).is_some());

        // Flyttes kutteren, beregnes målet på nytt.
        doc.commit_skeleton(&E::Translate {
            spline: c,
            delta: [10.0, 0.0, 0.0],
        })
        .unwrap();
        let moved = doc.mesh(a).unwrap().signed_volume();
        assert!(moved > cut_a, "{moved} {cut_a}");
        assert_eq!(doc.undo(), Some(SKELETON_SHAPE));
        assert!((doc.mesh(a).unwrap().signed_volume() - cut_a).abs() < 1e-9);

        // Utskjæringen slås av uten at kutteren skjules eller endres.
        doc.commit_skeleton(&E::SetCut {
            spline: c,
            cut: Some(crate::skeleton::Cut {
                active: false,
                ..cut(vec![a])
            }),
        })
        .unwrap();
        assert!((doc.mesh(a).unwrap().signed_volume() - va).abs() < 1e-9);
        assert!(!doc.skeleton().spline(c).unwrap().hidden);
        doc.undo();

        // Prosjektfilen bevarer kutteren og målet.
        let text = doc.to_project_json();
        let opened = Document::open_project(&text, 1.5).unwrap();
        assert_eq!(opened.skeleton().spline(c).unwrap().cut, Some(cut(vec![a])));
        assert!((opened.mesh(a).unwrap().signed_volume() - cut_a).abs() < 1e-6);
    }

    #[test]
    fn kuttemaal_er_eksplisitte_og_ryddes_naar_maalet_slettes() {
        let mut doc = Document::new(2.0);
        let (a, b, c) = two_balls_and_cutter(&mut doc);
        doc.commit_skeleton(&E::SetCut {
            spline: c,
            cut: Some(cut(vec![a, b, a])),
        })
        .unwrap();
        assert_eq!(doc.skeleton().spline(c).unwrap().cut, Some(cut(vec![a, b])));
        // En kutter kan ikke skjære i seg selv, i en annen kutter eller i en gren.
        assert!(doc
            .commit_skeleton(&E::SetCut {
                spline: c,
                cut: Some(cut(vec![c])),
            })
            .is_err());
        assert!(
            doc.commit_skeleton(&E::SetCut {
                spline: a,
                cut: Some(cut(vec![b])),
            })
            .is_err(),
            "a kuttes av c og kan ikke selv være kutter"
        );
        let branch = doc
            .commit_skeleton(&E::AddChild {
                parent: b,
                at: 1.0,
                direction: [1.0, 0.0, 0.0],
                length: 20.0,
                thickness: 4.0,
                orientation: Orientation::FollowParent,
            })
            .unwrap()
            .unwrap();
        assert!(doc
            .commit_skeleton(&E::SetCut {
                spline: c,
                cut: Some(cut(vec![branch])),
            })
            .is_err());
        assert!(doc
            .commit_skeleton(&E::SetCut {
                spline: branch,
                cut: Some(cut(vec![a])),
            })
            .is_err());
        // Festet til grenen på b er urørt av kuttet.
        assert_eq!(
            doc.skeleton()
                .spline(branch)
                .unwrap()
                .attachment
                .unwrap()
                .parent,
            b
        );
        // Slettes et mål, fjernes det fra kutteren; resten står.
        doc.commit_skeleton(&E::Remove { spline: b }).unwrap();
        assert_eq!(doc.skeleton().spline(c).unwrap().cut, Some(cut(vec![a])));
        doc.undo();
        assert_eq!(doc.skeleton().spline(c).unwrap().cut, Some(cut(vec![a, b])));
        // Tilbake til vanlig form: målet blir helt igjen.
        let whole = {
            let mut d = Document::new(2.0);
            let (a2, _, _) = two_balls_and_cutter(&mut d);
            d.mesh(a2).unwrap().signed_volume()
        };
        doc.commit_skeleton(&E::SetCut {
            spline: c,
            cut: None,
        })
        .unwrap();
        assert!((doc.mesh(a).unwrap().signed_volume() - whole).abs() < 1e-9);
    }
}
