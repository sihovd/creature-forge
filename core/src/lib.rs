//! Geometrikjernen i Creature Forge.
//!
//! Kjernen eier dokumentets geometri. Nettleseren får bare beregnede
//! meshdata tilbake (se spesifikasjonen kap. 17). Alle lengder er i
//! millimeter.

pub mod document;
pub mod mesh;
pub mod pull;
pub mod refine;
pub mod sphere;

use document::{control_points, Document};
use pull::Pull;
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
    2
}

/// Dokumentet slik grensesnittet ser det gjennom workeren.
#[wasm_bindgen(js_name = Document)]
pub struct JsDocument(Document);

#[allow(clippy::new_without_default)]
#[wasm_bindgen(js_class = Document)]
impl JsDocument {
    #[wasm_bindgen(constructor)]
    pub fn new(target_edge_mm: f64) -> JsDocument {
        JsDocument(Document::new(target_edge_mm))
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

    /// Angrer siste operasjon og returnerer formen som ble endret.
    pub fn undo(&mut self) -> Option<u32> {
        self.0.undo()
    }

    pub fn redo(&mut self) -> Option<u32> {
        self.0.redo()
    }
}

/// Grepet sendes som [ax, ay, az, dx, dy, dz, avstand, radius].
fn read_pull(v: &[f64]) -> Result<Pull, JsError> {
    if v.len() != 8 {
        return Err(JsError::new("Grepet må ha 8 tall"));
    }
    Ok(Pull {
        anchor: [v[0], v[1], v[2]],
        direction: [v[3], v[4], v[5]],
        distance: v[6],
        radius: v[7],
    })
}
