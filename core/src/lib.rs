//! Geometrikjernen i Creature Forge.
//!
//! Kjernen eier dokumentets geometri. Nettleseren får bare beregnede
//! meshdata tilbake (se spesifikasjonen kap. 17). Alle lengder er i
//! millimeter.

pub mod mesh;
pub mod sphere;

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

impl From<mesh::TriMesh> for MeshBuffers {
    fn from(m: mesh::TriMesh) -> Self {
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
    Ok(mesh.into())
}

/// Versjonen av geometriberegningene. Lagres i prosjektfiler (kap. 19).
#[wasm_bindgen(js_name = geometryVersion)]
pub fn geometry_version() -> u32 {
    1
}
