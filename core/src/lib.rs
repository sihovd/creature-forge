//! Geometrikjernen i Creature Forge.
//!
//! Kjernen eier dokumentets geometri. Nettleseren får bare beregnede
//! meshdata tilbake (se spesifikasjonen kap. 17). Alle lengder er i
//! millimeter.

pub mod bend;
pub mod curve;
pub mod document;
pub mod mesh;
pub mod profile;
pub mod pull;
pub mod refine;
pub mod skeleton;
pub mod sphere;
pub mod surface;

use document::{control_points, Document};
use pull::Pull;
use skeleton::SkeletonEdit;
use wasm_bindgen::prelude::*;

/// Beregnet trekantmesh slik den sendes til visningen.
#[wasm_bindgen]
pub struct MeshBuffers {
    positions: Vec<f32>,
    normals: Vec<f32>,
    indices: Vec<u32>,
}

#[wasm_bindgen]
impl MeshBuffers {
    /// Posisjoner som x, y, z per punkt, i millimeter.
    #[wasm_bindgen(js_name = takePositions)]
    pub fn take_positions(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.positions)
    }

    #[wasm_bindgen(js_name = takeNormals)]
    pub fn take_normals(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.normals)
    }

    #[wasm_bindgen(js_name = takeIndices)]
    pub fn take_indices(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.indices)
    }
}

impl From<&mesh::TriMesh> for MeshBuffers {
    fn from(m: &mesh::TriMesh) -> Self {
        let normals = m.vertex_normals();
        MeshBuffers {
            positions: m
                .positions
                .iter()
                .flat_map(|p| p.map(|v| v as f32))
                .collect(),
            normals: normals.iter().flat_map(|n| n.map(|v| v as f32)).collect(),
            indices: m.triangles.iter().flatten().copied().collect(),
        }
    }
}

/// Lager en kule med gitt radius (mm) og ønsket kantlengde (mm).
#[wasm_bindgen(js_name = createSphere)]
pub fn create_sphere(radius_mm: f64, target_edge_mm: f64) -> Result<MeshBuffers, JsError> {
    let mesh =
        sphere::icosphere_for_edge(radius_mm, target_edge_mm).map_err(|e| JsError::new(&e))?;
    Ok((&mesh).into())
}

/// Versjonen av geometriberegningene. Lagres i prosjektfiler (kap. 19).
#[wasm_bindgen(js_name = geometryVersion)]
pub fn geometry_version() -> u32 {
    5
}

/// Dokumentet slik grensesnittet ser det gjennom workeren.
#[wasm_bindgen(js_name = Document)]
pub struct JsDocument(Document, Vec<document::ShapePreview>);

#[allow(clippy::new_without_default)]
#[wasm_bindgen(js_class = Document)]
impl JsDocument {
    #[wasm_bindgen(constructor)]
    pub fn new(target_edge_mm: f64) -> JsDocument {
        JsDocument(Document::new(target_edge_mm), Vec::new())
    }

    pub fn revision(&self) -> f64 {
        self.0.revision() as f64
    }

    #[wasm_bindgen(js_name = canUndo)]
    pub fn can_undo(&self) -> bool {
        self.0.can_undo()
    }

    #[wasm_bindgen(js_name = canRedo)]
    pub fn can_redo(&self) -> bool {
        self.0.can_redo()
    }

    #[wasm_bindgen(js_name = shapeIds)]
    pub fn shape_ids(&self) -> Vec<u32> {
        self.0.shape_ids()
    }

    #[wasm_bindgen(js_name = addSphere)]
    pub fn add_sphere(&mut self, x: f64, y: f64, z: f64, radius: f64) -> Result<u32, JsError> {
        self.0
            .add_sphere([x, y, z], radius)
            .map_err(|e| JsError::new(&e))
    }

    /// Meshen til en form, eller `undefined` hvis formen ikke finnes.
    pub fn mesh(&self, shape: u32) -> Option<MeshBuffers> {
        self.0.mesh(shape).map(Into::into)
    }

    /// Kontrollpunkter som flat liste: x, y, z, nx, ny, nz per punkt.
    #[wasm_bindgen(js_name = controlPoints)]
    pub fn control_points(&self, shape: u32, spacing_mm: f64) -> Vec<f32> {
        self.0
            .mesh(shape)
            .map(|m| control_points(m, spacing_mm))
            .unwrap_or_default()
            .into_iter()
            .flat_map(|(p, n)| [p, n])
            .flatten()
            .map(|v| v as f32)
            .collect()
    }

    #[wasm_bindgen(js_name = previewPull)]
    pub fn preview_pull(&self, shape: u32, pull: &[f64]) -> Result<MeshBuffers, JsError> {
        let mesh = self
            .0
            .preview_pull(shape, &read_pull(pull)?)
            .map_err(|e| JsError::new(&e))?;
        Ok((&mesh).into())
    }

    #[wasm_bindgen(js_name = commitPull)]
    pub fn commit_pull(&mut self, shape: u32, pull: &[f64]) -> Result<(), JsError> {
        self.0
            .commit_pull(shape, read_pull(pull)?)
            .map_err(|e| JsError::new(&e))
    }

    /// Skjelettet som JSON for visningen (se `skeleton::SplineView`).
    #[wasm_bindgen(js_name = skeletonView)]
    pub fn skeleton_view(&self) -> Result<String, JsError> {
        view_json(self.0.skeleton())
    }

    /// Visningen av skjelettet med en redigering som ikke lagres. Meshene
    /// til de berørte kroppene hentes etterpå med `previewShapes` og
    /// `previewMesh`.
    #[wasm_bindgen(js_name = previewSkeleton)]
    pub fn preview_skeleton(&mut self, edit_json: &str) -> Result<String, JsError> {
        let (skeleton, meshes) = self
            .0
            .preview_skeleton(&read_edit(edit_json)?)
            .map_err(|e| JsError::new(&e))?;
        self.1 = meshes;
        view_json(&skeleton)
    }

    /// Kroppene i siste forhåndsvisning av skjelettet.
    #[wasm_bindgen(js_name = previewShapes)]
    pub fn preview_shapes(&self) -> Vec<u32> {
        self.1.iter().map(|(id, _)| *id).collect()
    }

    /// Meshen til en kropp i siste forhåndsvisning, eller `undefined` når
    /// kroppen er fjernet.
    #[wasm_bindgen(js_name = previewMesh)]
    pub fn preview_mesh(&self, shape: u32) -> Option<MeshBuffers> {
        self.1
            .iter()
            .find(|(id, _)| *id == shape)
            .and_then(|(_, m)| m.as_ref())
            .map(Into::into)
    }

    /// Utfører en redigering av skjelettet som ett angresteg. Returnerer id-en
    /// til en ny spline, eller 0.
    #[wasm_bindgen(js_name = commitSkeleton)]
    pub fn commit_skeleton(&mut self, edit_json: &str) -> Result<u32, JsError> {
        self.0
            .commit_skeleton(&read_edit(edit_json)?)
            .map(|id| id.unwrap_or(0))
            .map_err(|e| JsError::new(&e))
    }

    /// Angrer siste operasjon og returnerer formen som ble endret
    /// (0 er skjelettet).
    pub fn undo(&mut self) -> Option<u32> {
        self.0.undo()
    }

    pub fn redo(&mut self) -> Option<u32> {
        self.0.redo()
    }
}

fn read_edit(json: &str) -> Result<SkeletonEdit, JsError> {
    serde_json::from_str(json).map_err(|e| JsError::new(&format!("Ugyldig redigering: {e}")))
}

fn view_json(skeleton: &skeleton::Skeleton) -> Result<String, JsError> {
    let view = skeleton.view().map_err(|e| JsError::new(&e))?;
    serde_json::to_string(&view).map_err(|e| JsError::new(&e.to_string()))
}

/// Grepet sendes som [ax, ay, az, nx, ny, nz, bx, by, bz, radius]:
/// startpunkt A, overflatenormal n0, grep B og påvirkningsradius, i mm.
fn read_pull(v: &[f64]) -> Result<Pull, JsError> {
    if v.len() != 10 {
        return Err(JsError::new("Grepet må ha 10 tall"));
    }
    Ok(Pull {
        anchor: [v[0], v[1], v[2]],
        normal: [v[3], v[4], v[5]],
        tip: [v[6], v[7], v[8]],
        radius: v[9],
    })
}
