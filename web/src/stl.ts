import type * as THREE from 'three';

/**
 * Binær STL av trekantmeshene, i millimeter. Scenen har y opp, mens
 * slicere for 3D-utskrift regner z opp, så punktene roteres (x, y, z) →
 * (x, −z, y). Rotasjonen beholder omløpsretningen, så normalene peker ut.
 */
export function binaryStl(meshes: THREE.BufferGeometry[]): ArrayBuffer {
  let count = 0;
  for (const g of meshes) count += (g.index ? g.index.count : g.getAttribute('position').count) / 3;
  const buffer = new ArrayBuffer(84 + count * 50);
  const view = new DataView(buffer);
  const header = 'Creature Forge STL (mm, z opp)';
  for (let i = 0; i < header.length; i++) view.setUint8(i, header.charCodeAt(i));
  view.setUint32(80, count, true);

  let o = 84;
  const p = [0, 0, 0, 0, 0, 0, 0, 0, 0];
  for (const g of meshes) {
    const pos = g.getAttribute('position');
    const index = g.index;
    const n = index ? index.count : pos.count;
    for (let t = 0; t < n; t += 3) {
      for (let k = 0; k < 3; k++) {
        const v = index ? index.getX(t + k) : t + k;
        p[k * 3] = pos.getX(v);
        p[k * 3 + 1] = -pos.getZ(v);
        p[k * 3 + 2] = pos.getY(v);
      }
      const ux = p[3] - p[0], uy = p[4] - p[1], uz = p[5] - p[2];
      const vx = p[6] - p[0], vy = p[7] - p[1], vz = p[8] - p[2];
      let nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
      const len = Math.hypot(nx, ny, nz) || 1;
      nx /= len;
      ny /= len;
      nz /= len;
      for (const v of [nx, ny, nz, ...p]) {
        view.setFloat32(o, v, true);
        o += 4;
      }
      view.setUint16(o, 0, true);
      o += 2;
    }
  }
  return buffer;
}
