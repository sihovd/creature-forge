import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { GripDrag, MIN_LOCK_LENGTH_MM, planeBasis, type Projector } from '../src/grip-math';

// Ortografisk kamera som ser langs −z: skjerm-x = x, skjerm-y = −y, 4 piksler per mm.
const PX = 4;
const view: Projector = {
  toScreen: (p) => new THREE.Vector2(p.x * PX, -p.y * PX),
  pixelsPerMm: () => PX,
  cameraRight: new THREE.Vector3(1, 0, 0),
  cameraUp: new THREE.Vector3(0, 1, 0),
};
const mouse = (x: number, y: number) => new THREE.Vector2(x, y);

function drag(normal = new THREE.Vector3(0, 1, 0)) {
  const a = new THREE.Vector3(0, 0, 0);
  return new GripDrag(a, normal, a, 'pull', mouse(0, 0), view);
}

describe('GripDrag', () => {
  it('planbasisen er ortonormal med e1 × e2 = n', () => {
    const n = new THREE.Vector3(0.3, 0.5, 0.8).normalize();
    const [e1, e2] = planeBasis(n);
    expect(e1.dot(n)).toBeCloseTo(0, 12);
    expect(e2.dot(n)).toBeCloseTo(0, 12);
    expect(new THREE.Vector3().crossVectors(e1, e2).distanceTo(n)).toBeLessThan(1e-12);
  });

  it('vanlig uttrekk følger normalen og gir samme mm uansett pikseltetthet', () => {
    const g = drag();
    g.update(mouse(0, -40), 'pull');
    expect(g.tip.distanceTo(new THREE.Vector3(0, 10, 0))).toBeLessThan(1e-9);
  });

  it('CF04: Alt flytter grepet i planet vinkelrett på normalen uten hopp', () => {
    const g = drag();
    g.update(mouse(0, -40), 'pull');
    const before = g.tip.clone();
    // Modusskiftet alene flytter ikke grepet.
    g.update(mouse(0, -40), 'plane');
    expect(g.tip.distanceTo(before)).toBeLessThan(1e-9);
    g.update(mouse(20, -40), 'plane');
    expect(g.tip.y).toBeCloseTo(10, 9);
    expect(g.tip.clone().sub(before).dot(g.normal)).toBeCloseTo(0, 9);
    expect(g.tip.distanceTo(before)).toBeGreaterThan(1);
  });

  it('CF05: Alt + Shift holder A fast og avstanden A–B lik låst verdi', () => {
    const g = drag();
    g.update(mouse(0, -40), 'pull');
    g.update(mouse(0, -40), 'lock');
    const L = g.lockedLength!;
    expect(L).toBeCloseTo(10, 9);
    for (const [x, y] of [[30, -40], [80, 10], [-120, 60], [400, -300]]) {
      g.update(mouse(x, y), 'lock');
      expect(Math.abs(g.tip.distanceTo(g.anchor) - L) / L).toBeLessThan(1e-6);
    }
    expect(g.anchor.length()).toBe(0);
  });

  it('CF06: bytte mellom modusene gir ingen posisjonshopp', () => {
    const g = drag();
    let m = mouse(0, 0);
    const modes = ['pull', 'plane', 'lock', 'pull', 'lock', 'plane'] as const;
    for (const mode of modes) {
      const before = g.tip.clone();
      g.update(m, mode);
      expect(g.tip.distanceTo(before)).toBeLessThan(1e-9);
      m = m.clone().add(mouse(15, -25));
      g.update(m, mode);
    }
  });

  it('Alt + Shift låser ikke en tråd uten målbar lengde', () => {
    const g = drag();
    g.update(mouse(0, -1), 'pull');
    expect(g.length).toBeLessThan(MIN_LOCK_LENGTH_MM);
    g.update(mouse(0, -1), 'lock');
    expect(g.lockedLength).toBeNull();
    const before = g.tip.clone();
    g.update(mouse(50, 50), 'lock');
    expect(g.tip.distanceTo(before)).toBeLessThan(1e-9);
  });

  it('skjermmodus flytter tuppen med pekeren i synsplanet', () => {
    const a = new THREE.Vector3();
    const g = new GripDrag(a, new THREE.Vector3(0, 0, 1), new THREE.Vector3(0, 0, 10), 'screen', mouse(0, 0), view);
    g.update(mouse(8, -12), 'screen');
    expect(g.tip.distanceTo(new THREE.Vector3(2, 3, 10))).toBeLessThan(1e-9);
  });
});
