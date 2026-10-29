import * as THREE from 'three';
import type { Projector } from './grip-math';

/** Bøyevinkelen pekeren gir per piksel: 180° på 300 piksler. */
export const BEND_RAD_PER_PX = Math.PI / 300;
/** Vinkeltrinn med Ctrl under bøying. */
export const BEND_SNAP_RAD = THREE.MathUtils.degToRad(15);
/** Faktor per scrolltrinn for bøyeradien. */
export const RADIUS_STEP = 1.15;

/**
 * Bøyens retning og vinkel for ønsket enderetning `end`, målt fra
 * startretningen `t`. Vinkelen er høyst 180°; for å bøye videre legges et
 * nytt punkt inn. Ved 0° og 180° er retningen ubestemt, og `fallback` brukes.
 */
export function bendFromEnd(
  t: THREE.Vector3,
  end: THREE.Vector3,
  fallback: THREE.Vector3,
  snap: number | null,
): { direction: THREE.Vector3; angle: number } {
  const e = end.clone().normalize();
  let angle = Math.acos(THREE.MathUtils.clamp(e.dot(t), -1, 1));
  const n = e.clone().addScaledVector(t, -e.dot(t));
  const direction = n.lengthSq() > 1e-12 ? n.normalize() : fallback.clone();
  if (snap) angle = Math.min(Math.round(angle / snap) * snap, Math.PI);
  return { direction, angle };
}

/**
 * Alt + Shift på tuppen av en spline: pekeren styrer enderetningen som et
 * relativt kulegrep, og scroll endrer radien. Enderetningen dreies mot den
 * veien pekeren beveger seg i bildet, så tuppen følger pekeren uansett
 * hvordan kurven står i forhold til kameraet. Andre retninger nås ved å
 * dreie kameraet mellom dragene.
 * Buelengden er låst i kjernen; her bestemmes bare retning, vinkel og radius.
 */
export class BendDrag {
  /** Ønsket enderetning. */
  end: THREE.Vector3;
  radius: number;
  private lastMouse: THREE.Vector2;
  private right: THREE.Vector3;
  private up: THREE.Vector3;
  private lastDirection: THREE.Vector3;

  constructor(
    /** Startretningen der bøyen begynner. */
    readonly tangent: THREE.Vector3,
    /** Enderetningen før draget, så bøyen starter uten hopp. */
    endTangent: THREE.Vector3,
    /** Lengden som bøyes, i mm. */
    readonly restLength: number,
    mouse: THREE.Vector2,
    view: Pick<Projector, 'cameraRight' | 'cameraUp'>,
    radius = restLength / 4,
  ) {
    this.end = endTangent.clone().normalize();
    this.lastMouse = mouse.clone();
    this.right = view.cameraRight.clone();
    this.up = view.cameraUp.clone();
    this.radius = radius;
    const [e1] = basis(tangent);
    this.lastDirection = bendFromEnd(tangent, this.end, e1, null).direction;
  }

  update(mouse: THREE.Vector2) {
    const dx = mouse.x - this.lastMouse.x;
    const dy = mouse.y - this.lastMouse.y;
    this.lastMouse.copy(mouse);
    // Bevegelsen i bildet som retning i scenen; skjermens y peker nedover.
    const move = this.right.clone().multiplyScalar(dx).addScaledVector(this.up, -dy);
    const axis = new THREE.Vector3().crossVectors(this.end, move);
    if (axis.lengthSq() < 1e-12) return;
    this.end.applyAxisAngle(axis.normalize(), Math.hypot(dx, dy) * BEND_RAD_PER_PX * axisShare(this.end, move)).normalize();
  }

  /** Scroll opp gjør buen større, scroll ned mindre. Radius null gir en knekk. */
  scroll(deltaY: number) {
    const steps = -Math.sign(deltaY);
    this.radius = this.radius < 0.05 && steps > 0 ? 0.5 : this.radius * RADIUS_STEP ** steps;
    if (this.radius < 0.05) this.radius = 0;
  }

  params(snap: boolean) {
    const b = bendFromEnd(this.tangent, this.end, this.lastDirection, snap ? BEND_SNAP_RAD : null);
    this.lastDirection = b.direction;
    // Radien kan ikke være større enn at buen får plass; lengden økes aldri.
    const max = b.angle > 1e-9 ? this.restLength / b.angle : Infinity;
    return { ...b, radius: Math.min(this.radius, max) };
  }
}

/** Den delen av bevegelsen som står vinkelrett på retningen og dermed dreier den. */
function axisShare(d: THREE.Vector3, move: THREE.Vector3) {
  const m = move.length();
  return m > 0 ? new THREE.Vector3().crossVectors(d, move).length() / m : 0;
}

function basis(n: THREE.Vector3): [THREE.Vector3, THREE.Vector3] {
  const helper = Math.abs(n.x) < 0.9 ? new THREE.Vector3(1, 0, 0) : new THREE.Vector3(0, 1, 0);
  const e1 = new THREE.Vector3().crossVectors(helper, n).normalize();
  return [e1, new THREE.Vector3().crossVectors(n, e1)];
}

/** Andelen av buelengden for punkt `i` i en tegnet polyline. */
export function polylineFraction(points: THREE.Vector3[], i: number) {
  let total = 0;
  let upTo = 0;
  for (let k = 1; k < points.length; k++) {
    total += points[k].distanceTo(points[k - 1]);
    if (k === i) upTo = total;
  }
  return total > 0 ? upTo / total : 0;
}
