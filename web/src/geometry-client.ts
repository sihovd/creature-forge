import type { Command, GeometryResponse, GeometryResult } from './protocol';

type Pending = { resolve: (r: { result: GeometryResult; computeMs: number }) => void; reject: (e: Error) => void };

/** Snakker med geometrikjernen i en egen worker. Forespørsler behandles i rekkefølge. */
export class GeometryClient {
  readonly ready: Promise<number>;
  /** Punktavstand i mm som sendes med alle forespørsler. */
  spacingMm = 6;
  private worker = new Worker(new URL('./geometry.worker.ts', import.meta.url), { type: 'module' });
  private nextId = 1;
  private pending = new Map<number, Pending>();

  constructor() {
    let onReady!: (v: number) => void;
    this.ready = new Promise((resolve) => (onReady = resolve));
    this.worker.onmessage = (event: MessageEvent<GeometryResponse>) => {
      const msg = event.data;
      if ('kind' in msg) return onReady(msg.geometryVersion);
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      if (msg.ok) p.resolve({ result: msg.result, computeMs: msg.computeMs });
      else p.reject(new Error(msg.message));
    };
  }

  get busy() {
    return this.pending.size > 0;
  }

  send(command: Command) {
    const id = this.nextId++;
    return new Promise<{ result: GeometryResult; computeMs: number }>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...command, id, spacingMm: this.spacingMm });
    });
  }
}
