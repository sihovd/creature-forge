import { describe, expect, it } from 'vitest';
import { normalize, parseObj, parseStl } from '../src/part-import';

describe('import av mesh-deler', () => {
  it('leser OBJ med firkanter, «v/vt/vn» og negative indekser', () => {
    const obj = `
v 0 0 0
v 2 0 0
v 2 4 0
v 0 4 0
vt 0 0
f 1/1/1 2/1/1 3/1/1 4/1/1
f -4 -2 -1
`;
    const m = parseObj(obj);
    expect(m.positions).toHaveLength(4);
    expect(m.triangles).toEqual([
      [0, 1, 2],
      [0, 2, 3],
      [0, 2, 3],
    ]);
  });

  it('leser STL i tekst- og binærformat', () => {
    const ascii = `solid t
facet normal 0 0 1
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 1 0
endloop
endfacet
endsolid t`;
    const a = parseStl(new TextEncoder().encode(ascii).buffer as ArrayBuffer);
    expect(a.triangles).toEqual([[0, 1, 2]]);
    const buf = new ArrayBuffer(84 + 50);
    const view = new DataView(buf);
    view.setUint32(80, 1, true);
    [0, 0, 0, 1, 0, 0, 0, 1, 0].forEach((v, i) => view.setFloat32(84 + 12 + i * 4, v, true));
    const b = parseStl(buf);
    expect(b.positions[1]).toEqual([1, 0, 0]);
    expect(b.triangles).toHaveLength(1);
  });

  it('normaliserer til høyde 1 med foten i origo', () => {
    const m = normalize({ positions: [[10, 5, 2], [14, 5, 2], [12, 25, 6]], triangles: [[0, 1, 2]] });
    const ys = m.positions.map((p) => p[1]);
    expect(Math.min(...ys)).toBe(0);
    expect(Math.max(...ys)).toBe(1);
    expect(m.positions[0][0]).toBeCloseTo(-0.1);
    expect(() => normalize({ positions: [[0, 0, 0]], triangles: [[0, 1, 2]] })).toThrow();
  });
});
