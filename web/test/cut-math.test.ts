import { describe, expect, it } from 'vitest';
import { cuttableLayers, defaultCutTargets } from '../src/cut-math';
import type { SplineView, Vec3 } from '../src/protocol';

function layer(id: number, points: Vec3[], thickness: number, cut: SplineView['cut'] = null): SplineView {
  return {
    id,
    parent: null,
    at: 0,
    thickness,
    length: 1,
    points,
    anchors: [],
    profiles: [],
    flat_end: false,
    name: '',
    layer: id,
    hidden: false,
    locked: false,
    orientation: 'follow_parent',
    end_parent: null,
    end_at: 0,
    parts: [],
    cut,
  };
}

describe('negativ form', () => {
  const body = layer(1, [[0, 0, 0], [0, 40, 0]], 10);
  const far = layer(2, [[100, 0, 0], [100, 40, 0]], 10);
  const other = layer(4, [[-30, 20, 0], [-30, 21, 0]], 5, { targets: [], active: true, sharp: false });
  const cutter = layer(3, [[12, 20, 0], [12, 21, 0]], 5);

  it('foreslår lagene kutteren overlapper', () => {
    expect(defaultCutTargets([body, far, cutter, other], 3)).toEqual([1]);
  });

  it('tar med speilbildet når speilmodus er på', () => {
    const mirrored = layer(5, [[-80, 20, 0], [-80, 21, 0]], 5);
    const right = layer(6, [[80, 20, 0], [80, 21, 0]], 5);
    expect(defaultCutTargets([mirrored, right], 6)).toEqual([]);
    expect(defaultCutTargets([mirrored, right], 6, true)).toEqual([5]);
  });

  it('kan bare skjære i vanlige lag, ikke i seg selv eller andre negative former', () => {
    expect(cuttableLayers([body, far, cutter, other], 3).map((s) => s.id)).toEqual([1, 2]);
  });
});
