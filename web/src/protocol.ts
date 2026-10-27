// Meldinger mellom grensesnittet og geometriworkeren (spesifikasjonen kap. 17 og 24).
// Workeren eier dokumentet. Grensesnittet sender kommandoer og får tilbake
// beregnede meshdata; det har ingen egen kopi av dokumentets sannhet.

/** Et grep: [ax, ay, az, dx, dy, dz, avstand mm, påvirkningsradius mm]. */
export type PullParams = [number, number, number, number, number, number, number, number];

export type Command =
  | { kind: 'addSphere'; center: [number, number, number]; radiusMm: number }
  | { kind: 'previewPull'; shape: number; pull: PullParams }
  | { kind: 'commitPull'; shape: number; pull: PullParams }
  | { kind: 'undo' }
  | { kind: 'redo' }
  | { kind: 'controlPoints'; spacingMm: number };

export type GeometryRequest = Command & { id: number; spacingMm: number };

export interface MeshResult {
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
}

export interface ShapeUpdate {
  shape: number;
  /** Ny mesh, eller null når formen er fjernet (for eksempel ved angre). */
  mesh: MeshResult | null;
  /** Kontrollpunkter: x, y, z, nx, ny, nz per punkt. */
  controlPoints: Float32Array;
}

export interface DocumentState {
  revision: number;
  canUndo: boolean;
  canRedo: boolean;
}

export type GeometryResult =
  | { kind: 'document'; state: DocumentState; shapes: ShapeUpdate[] }
  | { kind: 'preview'; shape: number; mesh: MeshResult };

export type GeometryResponse =
  | { kind: 'ready'; geometryVersion: number }
  | { id: number; ok: true; result: GeometryResult; computeMs: number }
  | { id: number; ok: false; message: string };
