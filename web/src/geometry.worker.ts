/// <reference lib="webworker" />
import init, { Document, geometryVersion, type MeshBuffers } from './core-pkg/cf_core.js';
import type { GeometryRequest, GeometryResponse, GeometryResult, MeshResult, ShapeUpdate } from './protocol';

const TARGET_EDGE_MM = 1.5;

const scope = self as unknown as DedicatedWorkerGlobalScope;
const post = (msg: GeometryResponse, transfer: Transferable[] = []) => scope.postMessage(msg, transfer);

let doc: Document;
const ready = init().then(() => {
  doc = new Document(TARGET_EDGE_MM);
  post({ kind: 'ready', geometryVersion: geometryVersion() });
});

function take(buffers: MeshBuffers): MeshResult {
  const mesh = {
    positions: buffers.takePositions(),
    normals: buffers.takeNormals(),
    indices: buffers.takeIndices(),
  };
  buffers.free();
  return mesh;
}

function shapeUpdate(shape: number, spacingMm: number): ShapeUpdate {
  const buffers = doc.mesh(shape);
  return {
    shape,
    mesh: buffers ? take(buffers) : null,
    controlPoints: buffers ? doc.controlPoints(shape, spacingMm) : new Float32Array(),
  };
}

/** Angre og gjør om returnerer 0 når skjelettet ble endret. */
const SKELETON_SHAPE = 0;

/** Kroppene før og etter en endring av skjelettet, så fjernede kropper også meldes. */
function withShapes(change: () => number | undefined): number[] {
  const before = Array.from(doc.shapeIds());
  const shape = change();
  if (shape === undefined) return [];
  if (shape !== SKELETON_SHAPE) return [shape];
  return [...new Set([...before, ...Array.from(doc.shapeIds())])];
}

function documentResult(shapes: number[], spacingMm: number, created: number | null = null): GeometryResult {
  return {
    kind: 'document',
    state: { revision: doc.revision(), canUndo: doc.canUndo(), canRedo: doc.canRedo() },
    shapes: shapes.filter((s) => s !== SKELETON_SHAPE).map((s) => shapeUpdate(s, spacingMm)),
    skeleton: JSON.parse(doc.skeletonView()),
    created,
  };
}

function run(req: GeometryRequest): GeometryResult {
  switch (req.kind) {
    case 'addSphere': {
      const [x, y, z] = req.center;
      const id = doc.addSphere(x, y, z, req.radiusMm);
      return documentResult([id], req.spacingMm, id);
    }
    case 'previewPull':
      return { kind: 'preview', shape: req.shape, mesh: take(doc.previewPull(req.shape, Float64Array.from(req.pull))) };
    case 'commitPull':
      doc.commitPull(req.shape, Float64Array.from(req.pull));
      return documentResult([req.shape], req.spacingMm);
    case 'undo':
    case 'redo':
      return documentResult(
        withShapes(() => (req.kind === 'undo' ? doc.undo() : doc.redo())),
        req.spacingMm,
      );
    case 'controlPoints':
      return documentResult(Array.from(doc.shapeIds()), req.spacingMm);
    case 'previewSkeleton': {
      const skeleton = JSON.parse(doc.previewSkeleton(JSON.stringify(req.edit)));
      const shapes = Array.from(doc.previewShapes()).map((shape) => {
        const buffers = doc.previewMesh(shape);
        return { shape, mesh: buffers ? take(buffers) : null };
      });
      return { kind: 'skeletonPreview', skeleton, shapes };
    }
    case 'saveProject':
      return { kind: 'project', text: doc.toProject() };
    case 'openProject': {
      const before = Array.from(doc.shapeIds());
      const next = Document.openProject(req.text, TARGET_EDGE_MM);
      doc.free();
      doc = next;
      // Formene fra forrige dokument meldes også, så visningen fjerner dem.
      return documentResult([...new Set([...before, ...Array.from(doc.shapeIds())])], req.spacingMm);
    }
    case 'commitSkeleton': {
      let created = 0;
      const shapes = withShapes(() => {
        created = doc.commitSkeleton(JSON.stringify(req.edit));
        return SKELETON_SHAPE;
      });
      return documentResult(shapes, req.spacingMm, created || null);
    }
  }
}

function transferables(result: GeometryResult): Transferable[] {
  if (result.kind === 'project') return [];
  const meshes = result.kind === 'preview' ? [result.mesh] : result.shapes.flatMap((s) => (s.mesh ? [s.mesh] : []));
  const points = result.kind === 'document' ? result.shapes.map((s) => s.controlPoints.buffer) : [];
  return [...meshes.flatMap((m) => [m.positions.buffer, m.normals.buffer, m.indices.buffer]), ...points];
}

scope.onmessage = async (event: MessageEvent<GeometryRequest>) => {
  await ready;
  const req = event.data;
  try {
    const start = performance.now();
    const result = run(req);
    // Bufferne flyttes til hovedtråden uten kopi; workeren mister eierskapet.
    post({ id: req.id, ok: true, result, computeMs: performance.now() - start }, transferables(result));
  } catch (err) {
    post({ id: req.id, ok: false, message: String(err) });
  }
};
