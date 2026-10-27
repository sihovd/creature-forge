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

function documentResult(shapes: number[], spacingMm: number): GeometryResult {
  return {
    kind: 'document',
    state: { revision: doc.revision(), canUndo: doc.canUndo(), canRedo: doc.canRedo() },
    shapes: shapes.map((s) => shapeUpdate(s, spacingMm)),
  };
}

function run(req: GeometryRequest): GeometryResult {
  switch (req.kind) {
    case 'addSphere': {
      const [x, y, z] = req.center;
      return documentResult([doc.addSphere(x, y, z, req.radiusMm)], req.spacingMm);
    }
    case 'previewPull':
      return { kind: 'preview', shape: req.shape, mesh: take(doc.previewPull(req.shape, Float64Array.from(req.pull))) };
    case 'commitPull':
      doc.commitPull(req.shape, Float64Array.from(req.pull));
      return documentResult([req.shape], req.spacingMm);
    case 'undo':
    case 'redo': {
      const shape = req.kind === 'undo' ? doc.undo() : doc.redo();
      return documentResult(shape === undefined ? [] : [shape], req.spacingMm);
    }
    case 'controlPoints':
      return documentResult(Array.from(doc.shapeIds()), req.spacingMm);
  }
}

function transferables(result: GeometryResult): Transferable[] {
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
