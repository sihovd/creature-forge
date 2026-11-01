import type { SplineView } from './protocol';

type Box = { lo: [number, number, number]; hi: [number, number, number] };

/** Boksen rundt alle splinene i laget `layer`, med tykkelsen som rand. */
export function layerBox(splines: SplineView[], layer: number): Box | null {
  let box: Box | null = null;
  for (const s of splines) {
    if (s.layer !== layer) continue;
    // Konturer kan være større enn tykkelsen.
    const scale = Math.max(1, ...s.profiles.map((p) => p.scale));
    const r = s.thickness * scale;
    for (const p of s.points) {
      box ??= { lo: [Infinity, Infinity, Infinity], hi: [-Infinity, -Infinity, -Infinity] };
      for (let a = 0; a < 3; a++) {
        box.lo[a] = Math.min(box.lo[a], p[a] - r);
        box.hi[a] = Math.max(box.hi[a], p[a] + r);
      }
    }
  }
  return box;
}

/** Vanlige lag (ikke negative former) en negativ form kan skjære i. */
export function cuttableLayers(splines: SplineView[], cutter: number) {
  return splines.filter((s) => s.parent === null && s.id !== cutter && !s.cut);
}

/**
 * Forslag til kuttemål når et lag gjøres om til en negativ form: de vanlige
 * lagene den overlapper (med speilbildet når speilmodus er på). Brukeren
 * ser og endrer listen i inspektøren.
 */
export function defaultCutTargets(splines: SplineView[], cutter: number, mirror = false): number[] {
  const own = layerBox(splines, cutter);
  if (!own) return [];
  const boxes = [own];
  if (mirror) boxes.push({ lo: [-own.hi[0], own.lo[1], own.lo[2]], hi: [-own.lo[0], own.hi[1], own.hi[2]] });
  return cuttableLayers(splines, cutter)
    .filter((t) => {
      const b = layerBox(splines, t.id);
      return !!b && boxes.some((c) => [0, 1, 2].every((a) => c.lo[a] <= b.hi[a] && b.lo[a] <= c.hi[a]));
    })
    .map((t) => t.id);
}
