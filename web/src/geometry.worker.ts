/// <reference lib="webworker" />
import init, { createSphere, geometryVersion } from './core-pkg/cf_core.js';
import type { GeometryRequest, GeometryResponse } from './protocol';

const scope = self as unknown as DedicatedWorkerGlobalScope;
const post = (msg: GeometryResponse, transfer: Transferable[] = []) => scope.postMessage(msg, transfer);

const ready = init().then(() => post({ kind: 'ready', geometryVersion: geometryVersion() }));

scope.onmessage = async (event: MessageEvent<GeometryRequest>) => {
  await ready;
  const req = event.data;
  try {
    const start = performance.now();
    const buffers = createSphere(req.radiusMm, req.targetEdgeMm);
    const mesh = {
      positions: buffers.takePositions(),
      normals: buffers.takeNormals(),
      indices: buffers.takeIndices(),
    };
    buffers.free();
    const computeMs = performance.now() - start;
    // Bufferne flyttes til hovedtråden uten kopi; workeren mister eierskapet.
    post({ id: req.id, revision: req.revision, kind: 'mesh', mesh, computeMs }, [
      mesh.positions.buffer,
      mesh.normals.buffer,
      mesh.indices.buffer,
    ]);
  } catch (err) {
    post({ id: req.id, revision: req.revision, kind: 'error', message: String(err) });
  }
};
