// Meldinger mellom grensesnittet og geometriworkeren (spesifikasjonen kap. 17 og 24).
// Workeren eier dokumentet. Grensesnittet sender kommandoer og får tilbake
// beregnede meshdata; det har ingen egen kopi av dokumentets sannhet.

/** Et grep i mm: [A (x, y, z), overflatenormal n0 (x, y, z), grep B (x, y, z), påvirkningsradius]. */
export type PullParams = [number, number, number, number, number, number, number, number, number, number];

export type Command =
  | { kind: 'addSphere'; center: [number, number, number]; radiusMm: number }
  | { kind: 'previewPull'; shape: number; pull: PullParams }
  | { kind: 'commitPull'; shape: number; pull: PullParams }
  | { kind: 'undo' }
  | { kind: 'redo' }
  | { kind: 'controlPoints'; spacingMm: number }
  | { kind: 'previewSkeleton'; edit: SkeletonEdit }
  | { kind: 'commitSkeleton'; edit: SkeletonEdit }
  | { kind: 'saveProject' }
  | { kind: 'openProject'; text: string };

export type Vec3 = [number, number, number];

/** Redigeringer av spline-skjelettet. Speiler `SkeletonEdit` i kjernen. */
export type SkeletonEdit =
  | { kind: 'addRoot'; start: Vec3; direction: Vec3; length: number; thickness: number }
  | { kind: 'addChild'; parent: number; at: number; direction: Vec3; length: number; thickness: number; orientation?: Orientation }
  | { kind: 'moveAnchor'; spline: number; index: number; position: Vec3 }
  | { kind: 'moveTip'; spline: number; position: Vec3 }
  | { kind: 'setHandle'; spline: number; index: number; side: 'in' | 'out'; position: Vec3 }
  | { kind: 'setKind'; spline: number; index: number; anchor_kind: AnchorKind }
  | { kind: 'insertAnchor'; spline: number; at: number }
  | { kind: 'extend'; spline: number; delta: number }
  | { kind: 'bend'; spline: number; from: number; direction: Vec3; angle: number; radius: number }
  | { kind: 'reattach'; spline: number; at: number; parent?: number }
  | { kind: 'setOrientation'; spline: number; orientation: Orientation }
  | { kind: 'translate'; spline: number; delta: Vec3 }
  | { kind: 'setThickness'; spline: number; thickness: number }
  | { kind: 'remove'; spline: number }
  | { kind: 'addProfile'; spline: number; at: number; from: number | null }
  | { kind: 'setProfile'; spline: number; station: number; profile: Profile }
  | { kind: 'setStation'; spline: number; station: number; at: number; scale: number; rotation: number }
  | { kind: 'removeProfile'; spline: number; station: number }
  | { kind: 'makeMaster'; spline: number; station: number }
  | { kind: 'linkMaster'; spline: number; station: number; master: number }
  | { kind: 'unlink'; spline: number; station: number }
  | { kind: 'setMaster'; master: number; profile: Profile }
  | { kind: 'setFlatEnd'; spline: number; flat: boolean }
  | { kind: 'rename'; spline: number; name: string }
  | { kind: 'setLayer'; spline: number; hidden: boolean; locked: boolean }
  | { kind: 'batch'; edits: SkeletonEdit[] };

export type AnchorKind = 'smooth' | 'corner';

/** Hvordan en gren følger forelderen: forelderens rotasjon eller fast retning i verdensrommet. */
export type Orientation = 'follow_parent' | 'keep_direction';

export type Vec2 = [number, number];

/** Et punkt i et tverrsnittsomriss. Håndtakene er forskyvninger fra punktet. */
export interface ProfilePoint {
  position: Vec2;
  handle_in: Vec2;
  handle_out: Vec2;
  kind: AnchorKind;
}

/** Lukket omriss i tverrsnittsplanet, i enheter av splinens tykkelse (sirkel = radius 1). */
export interface Profile {
  points: ProfilePoint[];
}

/** En tverrsnittsprofil på et sted langs splinen. */
export interface StationView {
  at: number;
  scale: number;
  rotation: number;
  master: number | null;
  profile: Profile;
  /** Omrisset rundt kurven i verdenskoordinater. */
  ring: Vec3[];
  /** Konturens plan: sentrum, normal (kurvens tangent) og profilens x- og y-akse. */
  origin: Vec3;
  normal: Vec3;
  x_axis: Vec3;
  y_axis: Vec3;
}

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
  profiles: StationView[];
  flat_end: boolean;
  /** Navnet brukeren har gitt splinen; tomt når den ikke har fått navn. */
  name: string;
  /** Rotsplinen i laget splinen hører til. */
  layer: number;
  /** Laget er skjult eller låst. */
  hidden: boolean;
  locked: boolean;
  orientation: Orientation;
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
}

export interface DocumentState {
  revision: number;
  canUndo: boolean;
  canRedo: boolean;
}

export type GeometryResult =
  | { kind: 'document'; state: DocumentState; shapes: ShapeUpdate[]; skeleton: SplineView[]; created: number | null }
  | { kind: 'preview'; shape: number; mesh: MeshResult }
  | { kind: 'project'; text: string }
  | { kind: 'skeletonPreview'; skeleton: SplineView[]; shapes: { shape: number; mesh: MeshResult | null }[] };

export type GeometryResponse =
  | { kind: 'ready'; geometryVersion: number }
  | { id: number; ok: true; result: GeometryResult; computeMs: number }
  | { id: number; ok: false; message: string };
