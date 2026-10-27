import * as THREE from 'three';

/**
 * Hvordan et grep beveges (spesifikasjonen kap. 5 og 24):
 * - `pull`: langs overflatenormalen n0.
 * - `plane` (Alt): i planet vinkelrett på n0, utspent av e1 og e2.
 * - `lock` (Alt + Shift): avstanden L = |B − A| låses, og B roteres rundt A.
 * - `screen`: i planet vinkelrett på synsretningen (brukes for å flytte tuppen
 *   av en eksisterende senterlinje).
 */
export type GripMode = 'pull' | 'plane' | 'lock' | 'screen';

/** Det grepet trenger å vite om kameraet. */
export interface Projector {
  /** Skjermposisjon i CSS-piksler. */
  toScreen(p: THREE.Vector3): THREE.Vector2;
  /** Piksler per millimeter ved et punkt i scenen. */
  pixelsPerMm(p: THREE.Vector3): number;
  cameraRight: THREE.Vector3;
  cameraUp: THREE.Vector3;
}

/** Minste trådlengde i mm før Alt + Shift kan låse den. */
export const MIN_LOCK_LENGTH_MM = 0.5;

/** To enhetsvektorer e1, e2 med e1 × e2 = n. */
export function planeBasis(n: THREE.Vector3): [THREE.Vector3, THREE.Vector3] {
  const helper = Math.abs(n.x) < 0.9 ? new THREE.Vector3(1, 0, 0) : new THREE.Vector3(0, 1, 0);
  const e1 = new THREE.Vector3().crossVectors(helper, n).normalize();
  const e2 = new THREE.Vector3().crossVectors(n, e1).normalize();
  return [e1, e2];
}

/**
 * Tilstanden til ett drag. Ved hvert modusskifte lagres nye referanseverdier
 * fra grepets nåværende posisjon, slik at grepet ikke hopper.
 */
export class GripDrag {
  readonly e1: THREE.Vector3;
  readonly e2: THREE.Vector3;
  mode: GripMode;
  /** Grepets nåværende posisjon B. */
  tip: THREE.Vector3;
  /** Låst trådlengde i Alt + Shift, ellers null. */
  lockedLength: number | null = null;

  private refMouse: THREE.Vector2;
  private refTip: THREE.Vector3;
  private lastMouse: THREE.Vector2;
  private refRight = new THREE.Vector3();
  private refUp = new THREE.Vector3();

  constructor(
    readonly anchor: THREE.Vector3,
    readonly normal: THREE.Vector3,
    tip: THREE.Vector3,
    mode: GripMode,
    mouse: THREE.Vector2,
    private view: Projector,
  ) {
    [this.e1, this.e2] = planeBasis(normal);
    this.tip = tip.clone();
    this.mode = mode;
    this.refMouse = mouse.clone();
    this.lastMouse = mouse.clone();
    this.refTip = tip.clone();
    this.rebase(mode);
  }

  /** Signert lengde langs n0, brukt i statuslinjen. */
  get distanceAlongNormal() {
    return this.tip.clone().sub(this.anchor).dot(this.normal);
  }

  get length() {
    return this.tip.distanceTo(this.anchor);
  }

  /** Oppdaterer grepet fra pekerens posisjon og ønsket modus. */
  update(mouse: THREE.Vector2, mode: GripMode) {
    if (mode !== this.mode) this.rebase(mode);
    this.lastMouse.copy(mouse);
    const delta = mouse.clone().sub(this.refMouse);
    switch (this.mode) {
      case 'pull':
        this.tip = this.refTip.clone().addScaledVector(this.normal, this.alongScreen(this.normal, delta));
        break;
      case 'plane': {
        const [x, y] = this.inPlane(this.e1, this.e2, delta);
        this.tip = this.refTip.clone().addScaledVector(this.e1, x).addScaledVector(this.e2, y);
        break;
      }
      case 'screen': {
        const [x, y] = this.inPlane(this.refRight, this.refUp, delta);
        this.tip = this.refTip.clone().addScaledVector(this.refRight, x).addScaledVector(this.refUp, y);
        break;
      }
      case 'lock':
        this.tip = this.rotated(delta);
        break;
    }
    return this.tip;
  }

  private rebase(mode: GripMode) {
    this.mode = mode;
    this.refMouse = this.lastMouse.clone();
    this.refTip = this.tip.clone();
    this.refRight.copy(this.view.cameraRight);
    this.refUp.copy(this.view.cameraUp);
    const l = this.length;
    // Ved null eller nesten null lengde må brukeren først trekke ut en målbar lengde.
    this.lockedLength = mode === 'lock' && l >= MIN_LOCK_LENGTH_MM ? l : null;
  }

  /** Millimeter langs `dir` for en musebevegelse, fra skjermprojeksjonen av retningen. */
  private alongScreen(dir: THREE.Vector3, delta: THREE.Vector2) {
    const a = this.view.toScreen(this.refTip);
    const s = this.view.toScreen(this.refTip.clone().add(dir)).sub(a);
    const perMm = this.view.pixelsPerMm(this.refTip);
    // Peker retningen nesten mot kameraet, brukes loddrett musebevegelse.
    if (s.length() < 0.3 * perMm) return -delta.y / perMm;
    return delta.dot(s) / s.lengthSq();
  }

  /** Løser J·[x, y] = Δ, der J er skjermprojeksjonen av u og v (piksler per mm). */
  private inPlane(u: THREE.Vector3, v: THREE.Vector3, delta: THREE.Vector2): [number, number] {
    const o = this.view.toScreen(this.refTip);
    const su = this.view.toScreen(this.refTip.clone().add(u)).sub(o);
    const sv = this.view.toScreen(this.refTip.clone().add(v)).sub(o);
    const det = su.x * sv.y - su.y * sv.x;
    const perMm = this.view.pixelsPerMm(this.refTip);
    if (Math.abs(det) < 0.05 * perMm * perMm) {
      // Planet sees nesten fra kanten: bruk kameraets akser projisert ned i planet.
      const n = new THREE.Vector3().crossVectors(u, v).normalize();
      const right = this.refRight.clone().projectOnPlane(n);
      const up = this.refUp.clone().projectOnPlane(n);
      const move = right.multiplyScalar(delta.x / perMm).addScaledVector(up, -delta.y / perMm);
      return [move.dot(u), move.dot(v)];
    }
    return [(delta.x * sv.y - delta.y * sv.x) / det, (su.x * delta.y - su.y * delta.x) / det];
  }

  /** Alt + Shift: roterer retningen d om kameraets akser, B = A + L·d. */
  private rotated(delta: THREE.Vector2) {
    const L = this.lockedLength;
    if (L === null) return this.refTip.clone();
    const d = this.refTip.clone().sub(this.anchor).normalize();
    // Første bevegelse flytter tuppen omtrent like langt som pekeren.
    const radPerPx = 1 / (L * this.view.pixelsPerMm(this.refTip));
    d.applyAxisAngle(this.refUp, delta.x * radPerPx);
    d.applyAxisAngle(this.refRight, delta.y * radPerPx);
    return this.anchor.clone().addScaledVector(d.normalize(), L);
  }
}
