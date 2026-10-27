//! Dokumentmodellen: varige formoperasjoner med angre og gjør om
//! (spesifikasjonen kap. 19).
//!
//! Dokumentet lagrer operasjonene, ikke bare meshen. En kroppsdels mesh
//! beregnes ved å spille av operasjonene som gjelder den. Hvert ferdig drag
//! er én operasjon og dermed ett angresteg.

use crate::mesh::{length, normalize, sub, TriMesh, Vec3};
use crate::pull::{apply_pull, Pull};
use crate::sphere::icosphere_for_edge;
use std::collections::HashMap;

pub type ShapeId = u32;

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
}

impl Operation {
    pub fn shape(&self) -> ShapeId {
        match self {
            Operation::AddSphere { shape, .. } | Operation::Pull { shape, .. } => *shape,
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
        let mesh = self.evaluate(self.meshes.get(&shape), &op).ok()?;
        self.meshes.insert(shape, mesh);
        self.history.push(op);
        self.revision += 1;
        Some(shape)
    }

    fn apply(&mut self, op: Operation) -> Result<(), String> {
        let shape = op.shape();
        let mesh = self.evaluate(self.meshes.get(&shape), &op)?;
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
        }
    }

    /// Beregner en forms mesh på nytt fra operasjonene i historikken.
    fn rebuild(&mut self, shape: ShapeId) {
        let mut mesh: Option<TriMesh> = None;
        for op in self.history.iter().filter(|op| op.shape() == shape) {
            // Operasjonene var gyldige da de ble lagt inn, og beregningen er deterministisk.
            mesh = self.evaluate(mesh.as_ref(), op).ok();
        }
        match mesh {
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
        Pull {
            anchor,
            direction,
            distance,
            radius: 10.0,
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
}
