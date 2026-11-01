// Import av ferdige 3D-mesher til delregisteret (OBJ og STL). Meshen gjøres
// om til trekanter og normaliseres til høyde 1 langs +Y med foten i origo,
// slik kjernen tar imot den.
import type { Vec3 } from './protocol';

export interface ImportedMesh {
  positions: Vec3[];
  triangles: [number, number, number][];
}

/** OBJ: `v`-linjer og `f`-linjer (polygoner deles i vifter). */
export function parseObj(text: string): ImportedMesh {
  const positions: Vec3[] = [];
  const triangles: [number, number, number][] = [];
  for (const raw of text.split('\n')) {
    const line = raw.trim().split(/\s+/);
    if (line[0] === 'v' && line.length >= 4) {
      positions.push([Number(line[1]), Number(line[2]), Number(line[3])]);
    } else if (line[0] === 'f' && line.length >= 4) {
      // «7/1/3» → 7; negative indekser teller bakfra.
      const idx = line.slice(1).map((t) => {
        const i = parseInt(t.split('/')[0], 10);
        return i < 0 ? positions.length + i : i - 1;
      });
      for (let k = 1; k + 1 < idx.length; k++) triangles.push([idx[0], idx[k], idx[k + 1]]);
    }
  }
  return { positions, triangles };
}

/** STL i tekst- eller binærformat. */
export function parseStl(data: ArrayBuffer): ImportedMesh {
  const bytes = new Uint8Array(data);
  const head = new TextDecoder().decode(bytes.subarray(0, Math.min(bytes.length, 512)));
  const view = new DataView(data);
  const count = bytes.length >= 84 ? view.getUint32(80, true) : 0;
  const binary = bytes.length === 84 + count * 50;
  const positions: Vec3[] = [];
  if (binary || !/^\s*solid[\s\S]*facet/.test(head)) {
    for (let f = 0; f < count; f++) {
      const o = 84 + f * 50 + 12;
      for (let v = 0; v < 3; v++) {
        const p = o + v * 12;
        positions.push([view.getFloat32(p, true), view.getFloat32(p + 4, true), view.getFloat32(p + 8, true)]);
      }
    }
  } else {
    const text = new TextDecoder().decode(bytes);
    for (const m of text.matchAll(/vertex\s+(\S+)\s+(\S+)\s+(\S+)/g)) positions.push([Number(m[1]), Number(m[2]), Number(m[3])]);
  }
  const triangles: [number, number, number][] = [];
  for (let i = 0; i + 2 < positions.length; i += 3) triangles.push([i, i + 1, i + 2]);
  return { positions, triangles };
}

/** Høyde 1 langs +Y, foten i origo og midten på Y-aksen. */
export function normalize(mesh: ImportedMesh): ImportedMesh {
  if (!mesh.positions.length || !mesh.triangles.length) throw new Error('Filen inneholder ingen trekanter');
  if (mesh.positions.some((p) => p.some((x) => !Number.isFinite(x)))) throw new Error('Filen har ugyldige tall');
  const n = mesh.positions.length;
  if (mesh.triangles.some((t) => t.some((i) => !(i >= 0 && i < n)))) throw new Error('Filen har ugyldige trekanter');
  const lo = [Infinity, Infinity, Infinity];
  const hi = [-Infinity, -Infinity, -Infinity];
  for (const p of mesh.positions) {
    for (let a = 0; a < 3; a++) {
      lo[a] = Math.min(lo[a], p[a]);
      hi[a] = Math.max(hi[a], p[a]);
    }
  }
  const height = hi[1] - lo[1];
  const size = height > 1e-9 ? height : Math.max(hi[0] - lo[0], hi[2] - lo[2], 1e-9);
  const cx = (lo[0] + hi[0]) / 2;
  const cz = (lo[2] + hi[2]) / 2;
  return {
    positions: mesh.positions.map((p) => [(p[0] - cx) / size, (p[1] - lo[1]) / size, (p[2] - cz) / size]),
    triangles: mesh.triangles,
  };
}

/** Leser en OBJ- eller STL-fil og normaliserer den. */
export async function readPartFile(file: File): Promise<ImportedMesh> {
  const name = file.name.toLowerCase();
  if (name.endsWith('.obj')) return normalize(parseObj(await file.text()));
  if (name.endsWith('.stl')) return normalize(parseStl(await file.arrayBuffer()));
  throw new Error('Bare OBJ- og STL-filer kan importeres');
}
