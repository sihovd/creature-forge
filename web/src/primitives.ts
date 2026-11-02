// Grunnformene i paletten (kule, kapsel, kjegle og boks). Alle former bygges
// rundt en spline med mesh rundt (Sivert 29. oktober 2026), så en grunnform er
// en ny rotspline med konturer og ender som gir formen. Hele formen lages i én
// `batch`, så den er ett angresteg.
import { ellipse, PRESETS, station } from './profile-math';
import type { SkeletonEdit, Station, Vec3 } from './protocol';

export type PrimitiveKind = 'sphere' | 'capsule' | 'cone' | 'box';

export const PRIMITIVE_NAMES: Record<PrimitiveKind, string> = {
  sphere: 'Kule',
  capsule: 'Kapsel',
  cone: 'Kjegle',
  box: 'Boks',
};

/** Splinen formen bygges rundt, i mm. Formene står på rutenettet (y = 0). */
interface Recipe {
  /** Startpunkt over bakken og retning. */
  y: number;
  direction: Vec3;
  length: number;
  thickness: number;
  stations: Station[];
  flat: boolean;
}

/** Høyden og bredden på grunnformene, omtrent 40 mm som en ny spline. */
const SIZE = 40;

function recipe(kind: PrimitiveKind): Recipe {
  switch (kind) {
    case 'sphere':
      // En kort spline med stor tykkelse er en kule, siden endene er avrundet.
      return { y: SIZE / 2 - 1, direction: [0, 1, 0], length: 2, thickness: SIZE / 2, stations: [], flat: false };
    case 'capsule':
      // En rett spline med sirkelprofil har avrundede ender.
      return { y: 11, direction: [0, 1, 0], length: SIZE + 10 - 22, thickness: 11, stations: [], flat: false };
    case 'cone': {
      // Konturene avtar jevnt fra foten til spissen; flate ender gir en plan fot.
      const base = SIZE / 2;
      const stations = [0, 0.25, 0.5, 0.75, 1].map((u) => station(u, ellipse(1, 1), Math.max(0.04, 1 - u)));
      return { y: 0, direction: [0, 1, 0], length: SIZE + 5, thickness: base, stations, flat: true };
    }
    case 'box':
      // Kvadratisk kontur med flate ender. Kvadratet i registeret er innskrevet i 0,85, så konturen skaleres opp til full bredde.
      return { y: 0, direction: [0, 1, 0], length: SIZE, thickness: SIZE / 2, stations: [0, 1].map((u) => station(u, PRESETS.Kvadrat(), 1 / 0.85)), flat: true };
  }
}

/**
 * Redigeringen som lager grunnformen med foten i `at` på bakken. `next` er
 * id-en den nye splinen får (fra dokumentet), så konturene kan legges på i
 * samme angresteg.
 */
export function primitiveEdit(kind: PrimitiveKind, at: Vec3, next: number): SkeletonEdit {
  const r = recipe(kind);
  const edits: SkeletonEdit[] = [
    { kind: 'addRoot', start: [at[0], r.y, at[2]], direction: r.direction, length: r.length, thickness: r.thickness },
    { kind: 'rename', spline: next, name: PRIMITIVE_NAMES[kind] },
  ];
  if (r.stations.length) edits.push({ kind: 'setStations', spline: next, stations: r.stations });
  if (r.flat) edits.push({ kind: 'setFlatEnd', spline: next, flat: true });
  return { kind: 'batch', edits };
}

/** Største radius formen har rundt splinen, for å plassere den ved siden av andre former. */
export function primitiveRadius(kind: PrimitiveKind) {
  const r = recipe(kind);
  return kind === 'box' ? r.thickness * Math.SQRT2 : r.thickness;
}
