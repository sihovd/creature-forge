// Meldinger mellom grensesnittet og geometriworkeren (spesifikasjonen kap. 17 og 24).
// Workeren eier dokumentet. Grensesnittet sender kommandoer og får tilbake
// beregnede meshdata; det har ingen egen kopi av dokumentets sannhet.

/** Et grep i mm: [A (x, y, z), overflatenormal n0 (x, y, z), grep B (x, y, z), påvirkningsradius]. */
export type PullParams = [number, number, number, number, number, number, number, number, number, number];

export type Command =
  | { kind: 'addSphere'; center: [number, number, number]; radiusMm: number }
  | { kind: 'previewPull'; shape: number; pull: PullParams }
  | { kind: 'commitPull'; shape: number; pull: PullParams }
  | { kind: 'previewEdit'; shape: number; target: number; pull: PullParams }
  | { kind: 'commitEdit'; shape: number; target: number; pull: PullParams }
  | { kind: 'undo' }
  | { kind: 'redo' }
  | { kind: 'controlPoints'; spacingMm: number }
  | { kind: 'previewSkeleton'; edit: SkeletonEdit }
  | { kind: 'commitSkeleton'; edit: SkeletonEdit };

export type Vec3 = [number, number, number];

/** Redigeringer av spline-skjelettet. Speiler `SkeletonEdit` i kjernen. */
export type SkeletonEdit =
  | { kind: 'addRoot'; start: Vec3; direction: Vec3; length: number; thickness: number }
  | { kind: 'addChild'; parent: number; at: number; direction: Vec3; length: number; thickness: number }
  | { kind: 'moveAnchor'; spline: number; index: number; position: Vec3 }
  | { kind: 'moveTip'; spline: number; position: Vec3 }
  | { kind: 'setHandle'; spline: number; index: number; side: 'in' | 'out'; position: Vec3 }
  | { kind: 'setKind'; spline: number; index: number; anchor_kind: AnchorKind }
  | { kind: 'insertAnchor'; spline: number; at: number }
  | { kind: 'extend'; spline: number; delta: number }
  | { kind: 'bend'; spline: number; from: number; direction: Vec3; angle: number; radius: number }
  | { kind: 'reattach'; spline: number; at: number }
  | { kind: 'setThickness'; spline: number; thickness: number }
  | { kind: 'remove'; spline: number }
  | { kind: 'batch'; edits: SkeletonEdit[] };

export type AnchorKind = 'smooth' | 'corner';

export interface AnchorView {
  position: Vec3;
  /** Håndtakenes endepunkter. */
  handle_in: Vec3;
  handle_out: Vec3;
  kind: AnchorKind;
  /** Andel av buelengden der punktet ligger. */
  at: number;
  tangent: Vec3;
}

/** En spline i skjelettet, i verdenskoordinater. */
export interface SplineView {
  id: number;
  parent: number | null;
  at: number;
  thickness: number;
  length: number;
  points: Vec3[];
  anchors: AnchorView[];
}

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
  /**
   * Senterlinjer, `CENTERLINE_STRIDE` tall per linje: historikkplass, radius,
   * A, n0, B og deretter `CENTERLINE_SAMPLES` punkter langs kurven.
   */
  centerlines: Float64Array;
}

export const CENTERLINE_SAMPLES = 33;
export const CENTERLINE_STRIDE = 11 + CENTERLINE_SAMPLES * 3;

export interface DocumentState {
  revision: number;
  canUndo: boolean;
  canRedo: boolean;
}

export type GeometryResult =
  | { kind: 'document'; state: DocumentState; shapes: ShapeUpdate[]; skeleton: SplineView[]; created: number | null }
  | { kind: 'preview'; shape: number; mesh: MeshResult }
  | { kind: 'skeletonPreview'; skeleton: SplineView[] };

export type GeometryResponse =
  | { kind: 'ready'; geometryVersion: number }
  | { id: number; ok: true; result: GeometryResult; computeMs: number }
  | { id: number; ok: false; message: string };
