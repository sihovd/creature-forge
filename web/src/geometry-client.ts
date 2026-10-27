import type { GeometryRequest, GeometryResponse, MeshResult } from './protocol';

type DistributiveOmit<T, K extends keyof any> = T extends unknown ? Omit<T, K> : never;
type Pending = { resolve: (r: { mesh: MeshResult; computeMs: number }) => void; reject: (e: Error) => void };

/** Snakker med geometrikjernen i en egen worker. */
export class GeometryClient {
  readonly ready: Promise<number>;
  private worker = new Worker(new URL('./geometry.worker.ts', import.meta.url), { type: 'module' });
  private nextId = 1;
  private pending = new Map<number, Pending>();

  constructor() {
    let onReady!: (v: number) => void;
    this.ready = new Promise((resolve) => (onReady = resolve));
    this.worker.onmessage = (event: MessageEvent<GeometryResponse>) => {
      const msg = event.data;
      if (msg.kind === 'ready') return onReady(msg.geometryVersion);
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      if (msg.kind === 'mesh') p.resolve({ mesh: msg.mesh, computeMs: msg.computeMs });
      else p.reject(new Error(msg.message));
    };
  }

  request(req: DistributiveOmit<GeometryRequest, 'id'>) {
    const id = this.nextId++;
    return new Promise<{ mesh: MeshResult; computeMs: number }>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...req, id });
    });
  }
}
