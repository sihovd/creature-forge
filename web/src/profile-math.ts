// Tverrsnittsomriss i 2D (issue #4, del 3). Speiler `profile.rs` i kjernen;
// kjernen kontrollerer omrisset (minst tre punkter, areal, ingen selvkryssing)
// når det lagres.
import type { AnchorKind, Profile, ProfilePoint, Station, Vec2 } from './protocol';

const CIRCLE_K = 0.5522847498307934;
const POINTS_PER_CURVE = 16;

const add = (a: Vec2, b: Vec2): Vec2 => [a[0] + b[0], a[1] + b[1]];
const sub = (a: Vec2, b: Vec2): Vec2 => [a[0] - b[0], a[1] - b[1]];
const mul = (a: Vec2, s: number): Vec2 => [a[0] * s, a[1] * s];
const len = (a: Vec2) => Math.hypot(a[0], a[1]);
const lerp = (a: Vec2, b: Vec2, t: number): Vec2 => add(a, mul(sub(b, a), t));

const corner = (position: Vec2): ProfilePoint => ({ position, handle_in: [0, 0], handle_out: [0, 0], kind: 'corner' });

export function ellipse(a: number, b: number): Profile {
  const pts: Vec2[] = [[a, 0], [0, b], [-a, 0], [0, -b]];
  const tan: Vec2[] = [[0, b], [-a, 0], [0, -b], [a, 0]];
  return {
    points: pts.map((position, i) => ({
      position,
      handle_in: mul(tan[i], -CIRCLE_K),
      handle_out: mul(tan[i], CIRCLE_K),
      kind: 'smooth' as AnchorKind,
    })),
  };
}

export function polygonProfile(points: Vec2[]): Profile {
  return { points: points.map(corner) };
}

const smooth = (position: Vec2, handleOut: Vec2, handleIn: Vec2 = mul(handleOut, -1)): ProfilePoint => ({
  position,
  handle_in: handleIn,
  handle_out: handleOut,
  kind: 'smooth',
});

/** Eggform med den smale enden opp. */
export function egg(): Profile {
  return {
    points: [
      smooth([0.78, -0.08], [0, 0.55], [0, -0.42]),
      smooth([0, 1.1], [-0.42, 0]),
      smooth([-0.78, -0.08], [0, -0.42], [0, 0.55]),
      smooth([0, -0.86], [0.43, 0]),
    ],
  };
}

/** Likesidet trekant med spissen opp, innskrevet i enhetssirkelen. */
export function triangle(): Profile {
  return polygonProfile([[0, 1], [-0.866, -0.5], [0.866, -0.5]]);
}

export const PRESETS: Record<string, () => Profile> = {
  Sirkel: () => ellipse(1, 1),
  Ellipse: () => ellipse(1, 0.6),
  Egg: egg,
  Trekant: triangle,
  Kvadrat: () => polygonProfile([[0.85, -0.85], [0.85, 0.85], [-0.85, 0.85], [-0.85, -0.85]]),
  H: () =>
    polygonProfile([
      [1, -1], [1, 1], [0.45, 1], [0.45, 0.25], [-0.45, 0.25], [-0.45, 1],
      [-1, 1], [-1, -1], [-0.45, -1], [-0.45, -0.25], [0.45, -0.25], [0.45, -1],
    ]),
};

export function segment(p: Profile, i: number): [Vec2, Vec2, Vec2, Vec2] {
  const a = p.points[i];
  const b = p.points[(i + 1) % p.points.length];
  return [a.position, add(a.position, a.handle_out), add(b.position, b.handle_in), b.position];
}

function bezier(s: [Vec2, Vec2, Vec2, Vec2], t: number): Vec2 {
  const u = 1 - t;
  const w = [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t];
  return s.reduce<Vec2>((acc, q, k) => add(acc, mul(q, w[k])), [0, 0]);
}

/** Omrisset som polygon, med ankerpunktene med. */
export function polygon(p: Profile): Vec2[] {
  const out: Vec2[] = [];
  p.points.forEach((_, i) => {
    const s = segment(p, i);
    out.push(s[0]);
    const straight = len(sub(s[1], s[0])) < 1e-12 && len(sub(s[2], s[3])) < 1e-12;
    if (!straight) for (let k = 1; k < POINTS_PER_CURVE; k++) out.push(bezier(s, k / POINTS_PER_CURVE));
  });
  return out;
}

const clone = (p: Profile): Profile => ({ points: p.points.map((q) => ({ ...q, position: [...q.position], handle_in: [...q.handle_in], handle_out: [...q.handle_out] })) });

/** Nærmeste segment og parameter til et punkt. */
export function nearestOnOutline(p: Profile, q: Vec2): { index: number; t: number; distance: number } {
  let best = { index: 0, t: 0, distance: Infinity };
  p.points.forEach((_, i) => {
    const s = segment(p, i);
    for (let k = 0; k <= 64; k++) {
      const d = len(sub(bezier(s, k / 64), q));
      if (d < best.distance) best = { index: i, t: k / 64, distance: d };
    }
  });
  return best;
}

/** Legger inn et punkt i segment `i` ved `t` uten å endre formen. */
export function insertPoint(p: Profile, i: number, t: number): { profile: Profile; index: number } {
  const out = clone(p);
  const [p0, p1, p2, p3] = segment(p, i);
  const n = p.points.length;
  const j = (i + 1) % n;
  const straight = len(sub(p1, p0)) < 1e-12 && len(sub(p2, p3)) < 1e-12;
  const q0 = lerp(p0, p1, t), q1 = lerp(p1, p2, t), q2 = lerp(p2, p3, t);
  const r0 = lerp(q0, q1, t), r1 = lerp(q1, q2, t);
  const mid = lerp(r0, r1, t);
  let point: ProfilePoint = corner(mid);
  if (!straight) {
    out.points[i].handle_out = sub(q0, p0);
    out.points[j].handle_in = sub(q2, p3);
    point = { position: mid, handle_in: sub(r0, mid), handle_out: sub(r1, mid), kind: 'smooth' };
  }
  out.points.splice(i + 1, 0, point);
  return { profile: out, index: i + 1 };
}

export function movePoint(p: Profile, i: number, position: Vec2): Profile {
  const out = clone(p);
  out.points[i].position = position;
  return out;
}

/** Setter et håndtak (endepunkt i profilplanet). Et mykt punkt retter inn det andre. */
export function setHandle(p: Profile, i: number, side: 'in' | 'out', end: Vec2): Profile {
  const out = clone(p);
  const a = out.points[i];
  const offset = sub(end, a.position);
  const other = side === 'in' ? 'handle_out' : 'handle_in';
  a[side === 'in' ? 'handle_in' : 'handle_out'] = offset;
  if (a.kind === 'smooth' && len(offset) > 1e-12) a[other] = mul(offset, -len(a[other]) / len(offset));
  return out;
}

/** Hjørne fjerner håndtakene; mykt punkt får håndtak langs naboenes retning. */
export function setKind(p: Profile, i: number, kind: AnchorKind): Profile {
  const out = clone(p);
  const n = out.points.length;
  const a = out.points[i];
  a.kind = kind;
  if (kind === 'corner') {
    a.handle_in = [0, 0];
    a.handle_out = [0, 0];
  } else {
    const prev = out.points[(i + n - 1) % n].position;
    const next = out.points[(i + 1) % n].position;
    const dir = sub(next, prev);
    const l = len(dir);
    if (l > 1e-12) {
      a.handle_out = mul(dir, len(sub(next, a.position)) / 3 / l);
      a.handle_in = mul(dir, -len(sub(prev, a.position)) / 3 / l);
    }
  }
  return out;
}

export function removePoint(p: Profile, i: number): Profile {
  const out = clone(p);
  out.points.splice(i, 1);
  return out;
}

/** Signert areal; positivt mot klokka. */
export function area(p: Profile) {
  const poly = polygon(p);
  let a = 0;
  poly.forEach((q, i) => {
    const r = poly[(i + 1) % poly.length];
    a += q[0] * r[1] - r[0] * q[1];
  });
  return a / 2;
}

/** En kontur langs splinen med eget omriss, slik kjernen tar imot den. */
export const station = (at: number, profile: Profile, scale = 1, rotation = 0): Station => ({
  at,
  shape: { kind: 'own', profile },
  scale,
  rotation,
});

/**
 * Volumpresets: flere konturer langs splinen som sammen gir en form. Én
 * sirkelprofil gir ikke en kule alene; kula trenger konturer som vokser og
 * avtar langs splinen. Målene regnes fra splinens lengde og tykkelse (mm).
 */
export const VOLUMES: Record<string, (length: number, thickness: number) => Station[]> = {
  Kule: (length, thickness) => {
    const r = length / 2;
    return [0.03, 0.18, 0.5, 0.82, 0.97].map((u) => station(u, ellipse(1, 1), Math.max(0.05, (r * Math.sqrt(Math.max(0, 1 - (2 * u - 1) ** 2))) / thickness)));
  },
  Hode: () => [
    // Smal hals, større hodeparti og avsmalnende avslutning.
    station(0.05, ellipse(1, 1), 0.6),
    station(0.28, ellipse(1, 1), 0.65),
    station(0.55, egg(), 1.35),
    station(0.8, ellipse(1, 0.85), 1.1),
    station(0.97, ellipse(1, 0.8), 0.35),
  ],
};
