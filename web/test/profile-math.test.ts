import { describe, expect, it } from 'vitest';
import { area, ellipse, insertPoint, PRESETS, polygon, removePoint, setHandle, setKind } from '../src/profile-math';

describe('konturomriss', () => {
  it('sirkelen har radius 1 og areal π', () => {
    for (const [x, y] of polygon(ellipse(1, 1))) expect(Math.hypot(x, y)).toBeCloseTo(1, 3);
    expect(area(ellipse(1, 1))).toBeCloseTo(Math.PI, 2);
  });

  it('nytt punkt endrer ikke formen', () => {
    const c = ellipse(1, 1);
    const { profile, index } = insertPoint(c, 0, 0.5);
    expect(index).toBe(1);
    expect(profile.points).toHaveLength(5);
    for (const [x, y] of polygon(profile)) expect(Math.hypot(x, y)).toBeCloseTo(1, 3);
    // Rett kant i H: nytt hjørnepunkt midt på kanten.
    const h = insertPoint(PRESETS.H(), 0, 0.5).profile;
    expect(h.points[1]).toMatchObject({ position: [1, 0], kind: 'corner' });
  });

  it('mykt punkt holder håndtakene på linje, hjørne fjerner dem', () => {
    const c = ellipse(1, 1);
    const moved = setHandle(c, 0, 'out', [1.2, 0.8]);
    const a = moved.points[0];
    const cross = a.handle_in[0] * a.handle_out[1] - a.handle_in[1] * a.handle_out[0];
    expect(cross).toBeCloseTo(0, 9);
    const corner = setKind(c, 0, 'corner');
    expect(corner.points[0].handle_out).toEqual([0, 0]);
    expect(setKind(corner, 0, 'smooth').points[0].handle_out[1]).toBeGreaterThan(0);
    expect(removePoint(c, 0).points).toHaveLength(3);
  });

  it('H-en er konkav med samme areal som i kjernen', () => {
    expect(Math.abs(area(PRESETS.H()))).toBeCloseTo(4 - 2 * 0.9 * 0.75, 9);
  });
});
