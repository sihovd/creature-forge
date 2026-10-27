// Meldinger mellom grensesnittet og geometriworkeren (spesifikasjonen kap. 17 og 24).
// Hver forespørsel har en id og dokumentrevisjonen den gjelder for, slik at et
// gammelt resultat aldri kan overskrive en nyere redigering.

export type GeometryRequest =
  | { id: number; revision: number; kind: 'createSphere'; radiusMm: number; targetEdgeMm: number };

export interface MeshResult {
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
}

export type GeometryResponse =
  | { kind: 'ready'; geometryVersion: number }
  | { id: number; revision: number; kind: 'mesh'; mesh: MeshResult; computeMs: number }
  | { id: number; revision: number; kind: 'error'; message: string };
