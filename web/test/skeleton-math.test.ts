import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { BEND_RAD_PER_PX, BendDrag, bendFromEnd, polylineFraction } from '../src/skeleton-math';

const Y = new THREE.Vector3(0, 1, 0);
const view = { cameraRight: new THREE.Vector3(1, 0, 0), cameraUp: new THREE.Vector3(0, 1, 0) };

describe('bøying fra tuppen', () => {
  it('gir vinkel og retning fra enderetningen', () => {
    const b = bendFromEnd(Y, new THREE.Vector3(1, 1, 0), new THREE.Vector3(0, 0, 1), null);
    expect(b.angle).toBeCloseTo(Math.PI / 4);
    expect(b.direction.x).toBeCloseTo(1);
    // 180° vender enden; retningen beholdes fra forrige verdi.
    const u = bendFromEnd(Y, new THREE.Vector3(0, -1, 0), new THREE.Vector3(0, 0, 1), null);
    expect(u.angle).toBeCloseTo(Math.PI);
    expect(u.direction.z).toBeCloseTo(1);
  });

  it('snapper til hele 15°', () => {
    const end = new THREE.Vector3(Math.sin(0.8), Math.cos(0.8), 0);
    const b = bendFromEnd(Y, end, new THREE.Vector3(1, 0, 0), THREE.MathUtils.degToRad(15));
    expect(THREE.MathUtils.radToDeg(b.angle)).toBeCloseTo(45);
  });

  it('starter uten hopp og følger pekeren', () => {
    const drag = new BendDrag(Y, Y, 40, new THREE.Vector2(100, 100), view);
    expect(drag.params(false).angle).toBeCloseTo(0);
    // Pekeren 150 px mot høyre: enden dreies 90° mot høyre i bildet.
    drag.update(new THREE.Vector2(100 + Math.PI / 2 / BEND_RAD_PER_PX, 100));
    const b = drag.params(false);
    expect(THREE.MathUtils.radToDeg(b.angle)).toBeCloseTo(90);
    expect(b.direction.x).toBeCloseTo(1);
    // Videre bevegelse i samme retning fortsetter til 180°.
    drag.update(new THREE.Vector2(100, 100 + Math.PI / 2 / BEND_RAD_PER_PX));
    expect(THREE.MathUtils.radToDeg(drag.params(false).angle)).toBeGreaterThan(120);
  });

  it('scroll endrer radien, og radien begrenses av lengden', () => {
    const drag = new BendDrag(Y, new THREE.Vector3(0, -1, 0), 40, new THREE.Vector2(), view, 10);
    drag.scroll(-100);
    expect(drag.radius).toBeCloseTo(11.5);
    drag.radius = 100;
    expect(drag.params(false).radius).toBeCloseTo(40 / Math.PI);
    for (let i = 0; i < 80; i++) drag.scroll(100);
    expect(drag.radius).toBe(0);
  });
});

it('andel av buelengden langs en polyline', () => {
  const pts = [0, 1, 3].map((x) => new THREE.Vector3(x, 0, 0));
  expect(polylineFraction(pts, 1)).toBeCloseTo(1 / 3);
  expect(polylineFraction(pts, 2)).toBeCloseTo(1);
});
