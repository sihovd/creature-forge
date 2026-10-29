//! Dokumentmodellen: varige formoperasjoner med angre og gjør om
//! (spesifikasjonen kap. 19).
//!
//! Dokumentet lagrer operasjonene, ikke bare meshen. En kroppsdels mesh
//! beregnes ved å spille av operasjonene som gjelder den. Hvert ferdig drag
//! er én operasjon og dermed ett angresteg. Redigering av en senterlinje
//! lagres som en egen operasjon som erstatter parameterne til et tidligere
//! uttrekk, slik at angre gjenoppretter de gamle parameterne.

use crate::mesh::{length, normalize, sub, TriMesh, Vec3};
use crate::pull::{apply_pull, Pull};
use crate::skeleton::{Skeleton, SkeletonEdit, SplineId};
use crate::sphere::icosphere_for_edge;
use std::collections::HashMap;

pub type ShapeId = u32;

/// Spline-skjelettet er ett formlag med denne id-en i historikken.
pub const SKELETON_SHAPE: ShapeId = 0;

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    AddSphere {
        shape: ShapeId,
        center: Vec3,
        radius: f64,
    },
    Pull {
        shape: ShapeId,
        pull: Pull,
    },
    /// Nye parametere for uttrekket på plass `target` i historikken.
    EditPull {
        shape: ShapeId,
        target: usize,
        pull: Pull,
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
            Operation::AddSphere { shape, .. }
            | Operation::Pull { shape, .. }
            | Operation::EditPull { shape, .. } => *shape,
            Operation::Skeleton { .. } => SKELETON_SHAPE,
        }
    }
}

pub struct Document {
    /// Ønsket kantlengde i mm for nye og fortettede områder.
    pub target_edge: f64,
    history: Vec<Operation>,
    redo: Vec<Operation>,
    next_shape: ShapeId,
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
            history: Vec::new(),
            redo: Vec::new(),
            next_shape: 1,
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

    /// Skjelettet med en redigering som ikke er lagret ennå (forhåndsvisning under drag).
    pub fn preview_skeleton(&self, edit: &SkeletonEdit) -> Result<Skeleton, String> {
        let mut next = self.skeleton.clone();
        next.apply(edit)?;
        Ok(next)
    }

    /// Utfører en redigering av skjelettet som ett angresteg.
    pub fn commit_skeleton(&mut self, edit: &SkeletonEdit) -> Result<Option<SplineId>, String> {
        let mut next = self.skeleton.clone();
        let created = next.apply(edit)?;
        self.apply(Operation::Skeleton { skeleton: next })?;
        Ok(created)
    }

    pub fn add_sphere(&mut self, center: Vec3, radius: f64) -> Result<ShapeId, String> {
        let shape = self.next_shape;
        self.apply(Operation::AddSphere {
            shape,
            center,
            radius,
        })?;
        self.next_shape += 1;
        Ok(shape)
    }

    /// Beregner resultatet av et uttrekk uten å endre dokumentet (forhåndsvisning under drag).
    pub fn preview_pull(&self, shape: ShapeId, pull: &Pull) -> Result<TriMesh, String> {
        let mesh = self.meshes.get(&shape).ok_or("Ukjent form")?;
        apply_pull(mesh, pull, self.target_edge)
    }

    pub fn commit_pull(&mut self, shape: ShapeId, pull: Pull) -> Result<(), String> {
        self.apply(Operation::Pull { shape, pull })
    }

    /// Senterlinjene til en form: historikkplass og gjeldende parametere per uttrekk.
    pub fn centerlines(&self, shape: ShapeId) -> Vec<(usize, Pull)> {
        self.effective(shape, None)
            .into_iter()
            .filter_map(|(i, op)| match op {
                Operation::Pull { pull, .. } => Some((i, pull)),
                _ => None,
            })
            .collect()
    }

    /// Forhåndsviser formen med nye parametere for et tidligere uttrekk.
    pub fn preview_edit(
        &self,
        shape: ShapeId,
        target: usize,
        pull: &Pull,
    ) -> Result<TriMesh, String> {
        self.check_edit(shape, target)?;
        self.evaluate_shape(shape, Some((target, *pull)))?
            .ok_or_else(|| "Ukjent form".into())
    }

    /// Endrer et tidligere uttrekk (redigering av senterlinjen). Ett angresteg.
    pub fn commit_edit(&mut self, shape: ShapeId, target: usize, pull: Pull) -> Result<(), String> {
        self.check_edit(shape, target)?;
        self.apply(Operation::EditPull {
            shape,
            target,
            pull,
        })
    }

    fn check_edit(&self, shape: ShapeId, target: usize) -> Result<(), String> {
        match self.history.get(target) {
            Some(Operation::Pull { shape: s, .. }) if *s == shape => Ok(()),
            _ => Err("Senterlinjen finnes ikke".into()),
        }
    }

    /// Angrer siste operasjon. Returnerer formen som ble endret.
    pub fn undo(&mut self) -> Option<ShapeId> {
        let op = self.history.pop()?;
        let shape = op.shape();
        self.redo.push(op);
        self.rebuild(shape);
        self.revision += 1;
        Some(shape)
    }

    pub fn redo(&mut self) -> Option<ShapeId> {
        let op = self.redo.pop()?;
        let shape = op.shape();
        self.history.push(op);
        self.rebuild(shape);
        self.revision += 1;
        Some(shape)
    }

    fn apply(&mut self, op: Operation) -> Result<(), String> {
        let shape = op.shape();
        if let Operation::Skeleton { skeleton } = &op {
            self.skeleton = skeleton.clone();
            self.history.push(op);
            self.redo.clear();
            self.revision += 1;
            return Ok(());
        }
        let mesh = match &op {
            // En redigering endrer en tidligere operasjon, så formen beregnes fra starten.
            Operation::EditPull { target, pull, .. } => self
                .evaluate_shape(shape, Some((*target, *pull)))?
                .ok_or("Ukjent form")?,
            _ => self.evaluate(self.meshes.get(&shape), &op)?,
        };
        self.meshes.insert(shape, mesh);
        self.history.push(op);
        self.redo.clear();
        self.revision += 1;
        Ok(())
    }

    fn evaluate(&self, current: Option<&TriMesh>, op: &Operation) -> Result<TriMesh, String> {
        match op {
            Operation::AddSphere { center, radius, .. } => {
                let mut m = icosphere_for_edge(*radius, self.target_edge)?;
                for p in m.positions.iter_mut() {
                    *p = [p[0] + center[0], p[1] + center[1], p[2] + center[2]];
                }
                Ok(m)
            }
            Operation::Pull { pull, .. } => apply_pull(
                current.ok_or("Uttrekk på en form som ikke finnes")?,
                pull,
                self.target_edge,
            ),
            Operation::EditPull { .. } => Err("En redigering beregnes via historikken".into()),
            Operation::Skeleton { .. } => Err("Skjelettet har ingen mesh ennå".into()),
        }
    }

    /// Operasjonene som gjelder formen, med redigeringer lagt inn i det
    /// uttrekket de endrer. `extra` er en redigering som ikke er lagret ennå.
    fn effective(&self, shape: ShapeId, extra: Option<(usize, Pull)>) -> Vec<(usize, Operation)> {
        let mut ops: Vec<(usize, Operation)> = Vec::new();
        let edits = self
            .history
            .iter()
            .enumerate()
            .filter(|(_, op)| op.shape() == shape);
        for (i, op) in edits {
            match op {
                Operation::EditPull { target, pull, .. } => {
                    if let Some((_, Operation::Pull { pull: p, .. })) =
                        ops.iter_mut().find(|(j, _)| j == target)
                    {
                        *p = *pull;
                    }
                }
                _ => ops.push((i, op.clone())),
            }
        }
        if let Some((target, pull)) = extra {
            if let Some((_, Operation::Pull { pull: p, .. })) =
                ops.iter_mut().find(|(j, _)| *j == target)
            {
                *p = pull;
            }
        }
        ops
    }

    fn evaluate_shape(
        &self,
        shape: ShapeId,
        extra: Option<(usize, Pull)>,
    ) -> Result<Option<TriMesh>, String> {
        let mut mesh: Option<TriMesh> = None;
        for (_, op) in self.effective(shape, extra) {
            mesh = Some(self.evaluate(mesh.as_ref(), &op)?);
        }
        Ok(mesh)
    }

    /// Beregner en forms mesh på nytt fra operasjonene i historikken.
    fn rebuild(&mut self, shape: ShapeId) {
        if shape == SKELETON_SHAPE {
            self.skeleton = self
                .history
                .iter()
                .rev()
                .find_map(|op| match op {
                    Operation::Skeleton { skeleton } => Some(skeleton.clone()),
                    _ => None,
                })
                .unwrap_or_else(Skeleton::new);
            return;
        }
        // Operasjonene var gyldige da de ble lagt inn, og beregningen er deterministisk.
        match self.evaluate_shape(shape, None).ok().flatten() {
            Some(m) => self.meshes.insert(shape, m),
            None => self.meshes.remove(&shape),
        };
    }
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

    fn pull_on(doc: &Document, shape: ShapeId, distance: f64) -> Pull {
        let anchor = doc.mesh(shape).unwrap().positions[0];
        let direction = normalize(sub(anchor, [0.0, 25.0, 0.0]));
        Pull::straight(anchor, direction, distance, 10.0)
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
    fn angre_av_ny_kule_fjerner_formen_og_ny_operasjon_toemmer_gjoer_om() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 25.0).unwrap();
        doc.undo();
        assert!(doc.mesh(s).is_none());
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
        assert_eq!(doc.revision(), rev);
        assert_eq!(doc.operations().len(), 1);
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
    fn redigering_av_senterlinjen_kan_angres() {
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 25.0).unwrap();
        let pull = pull_on(&doc, s, 15.0);
        doc.commit_pull(s, pull).unwrap();
        let straight = doc.mesh(s).unwrap().clone();
        let (target, _) = doc.centerlines(s)[0];

        let bent = Pull {
            tip: [pull.tip[0] + 10.0, pull.tip[1], pull.tip[2]],
            ..pull
        };
        let preview = doc.preview_edit(s, target, &bent).unwrap();
        doc.commit_edit(s, target, bent).unwrap();
        assert_eq!(doc.mesh(s).unwrap().positions, preview.positions);
        assert_eq!(doc.centerlines(s), vec![(target, bent)]);
        assert_ne!(doc.mesh(s).unwrap().positions, straight.positions);

        doc.undo();
        assert_eq!(doc.centerlines(s), vec![(target, pull)]);
        assert_eq!(doc.mesh(s).unwrap().positions, straight.positions);
        doc.redo();
        assert_eq!(doc.centerlines(s), vec![(target, bent)]);
        assert!(doc.commit_edit(s, 0, bent).is_err());
    }

    #[test]
    fn skjelettet_er_ett_formlag_med_angre() {
        use crate::skeleton::SkeletonEdit as E;
        let mut doc = Document::new(2.0);
        let s = doc.add_sphere([0.0, 25.0, 0.0], 25.0).unwrap();
        let root = doc
            .commit_skeleton(&E::AddRoot {
                start: [0.0; 3],
                direction: [0.0, 1.0, 0.0],
                length: 40.0,
                thickness: 5.0,
            })
            .unwrap()
            .unwrap();
        let preview = doc
            .preview_skeleton(&E::Extend {
                spline: root,
                delta: 10.0,
            })
            .unwrap();
        // Forhåndsvisning endrer ikke dokumentet.
        assert_eq!(doc.skeleton().ids(), vec![root]);
        assert!((preview.world_spline(root).unwrap().length - 50.0).abs() < 1e-6);
        let child = doc
            .commit_skeleton(&E::AddChild {
                parent: root,
                at: 0.5,
                direction: [1.0, 0.0, 0.0],
                length: 20.0,
                thickness: 3.0,
            })
            .unwrap()
            .unwrap();
        assert_eq!(doc.skeleton().children(root), vec![child]);
        // Angre fjerner grenen, men ikke kula eller roten.
        assert_eq!(doc.undo(), Some(SKELETON_SHAPE));
        assert_eq!(doc.skeleton().ids(), vec![root]);
        assert!(doc.mesh(s).is_some());
        doc.redo();
        assert_eq!(doc.skeleton().ids(), vec![root, child]);
        doc.undo();
        doc.undo();
        assert!(doc.skeleton().ids().is_empty());
        // En ugyldig redigering lagres ikke.
        let rev = doc.revision();
        assert!(doc.commit_skeleton(&E::Remove { spline: 99 }).is_err());
        assert_eq!(doc.revision(), rev);
    }
}
