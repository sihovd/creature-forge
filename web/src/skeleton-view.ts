import * as THREE from 'three';
import type { SplineView } from './protocol';
import { polylineFraction } from './skeleton-math';
import type { Viewport } from './viewport';

const LINE_COLOR = 0x3b5ba5;
const SELECTED_COLOR = 0xd2652d;
const HANDLE_COLOR = 0x7a7f73;
const RING_COLOR = 0x6f8fd6;
const SNAP_COLOR = 0x1f9d55;

/** Noe i skjelettet som kan gripes. */
export type SkeletonPick =
  | { kind: 'anchor'; spline: number; index: number; position: THREE.Vector3 }
  | { kind: 'handle'; spline: number; index: number; side: 'in' | 'out'; position: THREE.Vector3 };

export interface Selection {
  spline: number;
  /** Valgt ankerpunkt, eller null. */
  index: number | null;
  /** Valgt tverrsnittsprofil (kontur), eller null. */
  station?: number | null;
}

/** Et punkt en spline-ende kan snappe til. */
export interface SnapTarget {
  spline: number;
  position: THREE.Vector3;
  /** Andel av buelengden på målsplinen. */
  at: number;
}

const v3 = (p: [number, number, number]) => new THREE.Vector3(p[0], p[1], p[2]);

/**
 * Tegner spline-skjelettet: kurven, ankerpunktene og, for valgt spline,
 * håndtakene. Hjørnepunkter tegnes som kuber og myke punkter som kuler.
 * Skjelettet tegnes oppå formene, så det kan redigeres også inne i kroppen.
 */
export class SkeletonView {
  splines: SplineView[] = [];
  selection: Selection | null = null;
  private group = new THREE.Group();
  private sphere = new THREE.SphereGeometry(1.2, 16, 12);
  private cube = new THREE.BoxGeometry(2, 2, 2);
  private handleDot = new THREE.SphereGeometry(0.8, 12, 8);
  private materials = {
    line: new THREE.LineBasicMaterial({ color: LINE_COLOR, depthTest: false }),
    selectedLine: new THREE.LineBasicMaterial({ color: SELECTED_COLOR, depthTest: false }),
    anchor: new THREE.MeshBasicMaterial({ color: LINE_COLOR, depthTest: false }),
    selected: new THREE.MeshBasicMaterial({ color: SELECTED_COLOR, depthTest: false }),
    handle: new THREE.MeshBasicMaterial({ color: HANDLE_COLOR, depthTest: false }),
    handleLine: new THREE.LineBasicMaterial({ color: HANDLE_COLOR, depthTest: false }),
    ring: new THREE.LineBasicMaterial({ color: RING_COLOR, depthTest: false, transparent: true, opacity: 0.8 }),
    selectedRing: new THREE.LineBasicMaterial({ color: SELECTED_COLOR, depthTest: false }),
  };
  private snapMarker: THREE.Mesh;
  private bendRing: THREE.Mesh;
  private bendLine: THREE.Line;

  constructor(private viewport: Viewport) {
    this.group.renderOrder = 3;
    this.bendRing = new THREE.Mesh(
      new THREE.CircleGeometry(1, 48),
      new THREE.MeshBasicMaterial({ color: SELECTED_COLOR, transparent: true, opacity: 0.15, side: THREE.DoubleSide, depthTest: false }),
    );
    this.bendLine = new THREE.Line(
      new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), new THREE.Vector3()]),
      new THREE.LineDashedMaterial({ color: SELECTED_COLOR, dashSize: 1.5, gapSize: 1, depthTest: false }),
    );
    this.bendRing.visible = this.bendLine.visible = false;
    this.snapMarker = new THREE.Mesh(
      new THREE.TorusGeometry(2.4, 0.35, 8, 32),
      new THREE.MeshBasicMaterial({ color: SNAP_COLOR, depthTest: false }),
    );
    this.snapMarker.renderOrder = 4;
    this.snapMarker.visible = false;
    viewport.addToScene(this.snapMarker);
    viewport.addToScene(this.group);
    viewport.addToScene(this.bendRing);
    viewport.addToScene(this.bendLine);
  }

  get count() {
    return this.splines.length;
  }

  spline(id: number) {
    return this.splines.find((s) => s.id === id) ?? null;
  }

  set(splines: SplineView[]) {
    this.splines = splines;
    if (this.selection && !this.spline(this.selection.spline)) this.selection = null;
    if (this.selection?.index != null && this.selection.index >= this.spline(this.selection.spline)!.anchors.length) {
      this.selection.index = null;
    }
    if (this.selection?.station != null && this.selection.station >= this.spline(this.selection.spline)!.profiles.length) {
      this.selection.station = null;
    }
    this.rebuild();
  }

  select(selection: Selection | null) {
    this.selection = selection;
    this.rebuild();
  }

  private rebuild() {
    for (const child of [...this.group.children]) {
      this.group.remove(child);
      if (child instanceof THREE.Line) child.geometry.dispose();
    }
    const add = (obj: THREE.Object3D) => {
      obj.renderOrder = 3;
      this.group.add(obj);
    };
    for (const s of this.splines) {
      const selected = this.selection?.spline === s.id;
      add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(s.points.map(v3)), selected ? this.materials.selectedLine : this.materials.line));
      // Konturene (tverrsnittene) tegnes som ringer rundt kurven.
      s.profiles.forEach((st, i) => {
        const ring = new THREE.LineLoop(
          new THREE.BufferGeometry().setFromPoints(st.ring.map(v3)),
          selected && this.selection!.station === i ? this.materials.selectedRing : this.materials.ring,
        );
        add(ring);
      });
      s.anchors.forEach((a, i) => {
        const active = selected && this.selection!.index === i;
        const marker = new THREE.Mesh(a.kind === 'corner' ? this.cube : this.sphere, active ? this.materials.selected : this.materials.anchor);
        marker.position.copy(v3(a.position));
        // Tuppen er større, siden den brukes til uttrekk og bøying.
        if (i === s.anchors.length - 1) marker.scale.setScalar(1.4);
        if (i === 0 && s.parent !== null) marker.scale.setScalar(0.7);
        add(marker);
        if (!selected) return;
        for (const h of [a.handle_in, a.handle_out]) {
          const end = v3(h);
          if (end.distanceTo(marker.position) < 1e-6) continue;
          add(new THREE.Line(new THREE.BufferGeometry().setFromPoints([marker.position, end]), this.materials.handleLine));
          const dot = new THREE.Mesh(this.handleDot, this.materials.handle);
          dot.position.copy(end);
          add(dot);
        }
      });
    }
  }

  /** Nærmeste ankerpunkt eller håndtak (bare for valgt spline) innenfor `maxPx`. */
  pick(clientX: number, clientY: number, maxPx: number): SkeletonPick | null {
    const mouse = this.mouse(clientX, clientY);
    let best: SkeletonPick | null = null;
    let bestDist = maxPx;
    // Ved lik avstand vinner det siste punktet, så tuppen på en kort spline
    // (som i en kule) kan gripes selv om den ligger oppå roten.
    const consider = (pick: SkeletonPick) => {
      const d = this.viewport.toScreen(pick.position).distanceTo(mouse);
      if (d <= bestDist) {
        best = pick;
        bestDist = d;
      }
    };
    for (const s of this.splines) {
      s.anchors.forEach((a, index) => {
        consider({ kind: 'anchor', spline: s.id, index, position: v3(a.position) });
        if (this.selection?.spline !== s.id) return;
        for (const side of ['in', 'out'] as const) {
          const h = v3(side === 'in' ? a.handle_in : a.handle_out);
          if (h.distanceTo(v3(a.position)) > 1e-6) consider({ kind: 'handle', spline: s.id, index, side, position: h });
        }
      });
    }
    return best;
  }

  /** Nærmeste punkt på en kurve: spline og andel av buelengden. */
  pickCurve(clientX: number, clientY: number, maxPx: number, only?: number) {
    const mouse = this.mouse(clientX, clientY);
    let best: { spline: number; at: number; position: THREE.Vector3 } | null = null;
    let bestDist = maxPx;
    for (const s of this.splines) {
      if (only !== undefined && s.id !== only) continue;
      const pts = s.points.map(v3);
      pts.forEach((p, i) => {
        const d = this.viewport.toScreen(p).distanceTo(mouse);
        if (d < bestDist) {
          bestDist = d;
          best = { spline: s.id, at: polylineFraction(pts, i), position: p };
        }
      });
    }
    return best as { spline: number; at: number; position: THREE.Vector3 } | null;
  }

  /**
   * Nærmeste punkt på en annen spline innenfor `maxPx` fra skjermpunktet
   * `screen` (CSS-piksler relativt til lerretet). Ankerpunkter går foran
   * punkter på kurven. Splinene i `exclude` hoppes over.
   */
  snapTarget(screen: THREE.Vector2, maxPx: number, exclude: Set<number>): SnapTarget | null {
    let best: SnapTarget | null = null;
    let bestDist = maxPx;
    for (const s of this.splines) {
      if (exclude.has(s.id)) continue;
      for (const a of s.anchors) {
        const d = this.viewport.toScreen(v3(a.position)).distanceTo(screen);
        // Ankerpunktene fanger litt lenger unna enn kurven.
        if (d < bestDist * 1.5 && (!best || d < bestDist)) {
          best = { spline: s.id, position: v3(a.position), at: a.at };
          bestDist = d;
        }
      }
    }
    if (best) return best;
    for (const s of this.splines) {
      if (exclude.has(s.id)) continue;
      const pts = s.points.map(v3);
      for (let i = 0; i + 1 < pts.length; i++) {
        const a = this.viewport.toScreen(pts[i]);
        const b = this.viewport.toScreen(pts[i + 1]);
        const ab = b.clone().sub(a);
        const l2 = ab.lengthSq();
        const t = l2 < 1e-9 ? 0 : THREE.MathUtils.clamp(screen.clone().sub(a).dot(ab) / l2, 0, 1);
        const d = a.clone().addScaledVector(ab, t).distanceTo(screen);
        if (d < bestDist) {
          bestDist = d;
          const at = polylineFraction(pts, i) + (polylineFraction(pts, i + 1) - polylineFraction(pts, i)) * t;
          best = { spline: s.id, position: pts[i].clone().lerp(pts[i + 1], t), at };
        }
      }
    }
    return best;
  }

  showSnap(target: SnapTarget | null) {
    this.snapMarker.visible = !!target;
    if (!target) return;
    this.snapMarker.position.copy(target.position);
    this.snapMarker.quaternion.copy(this.viewport.camera.quaternion);
    this.snapMarker.scale.setScalar(4 / this.viewport.pixelsPerMm(target.position));
  }

  /** Viser bøyens plan, retning og radius under Alt + Shift. */
  showBend(start: THREE.Vector3, tangent: THREE.Vector3, direction: THREE.Vector3, radius: number, tip: THREE.Vector3) {
    const normal = new THREE.Vector3().crossVectors(tangent, direction);
    if (normal.lengthSq() > 1e-12) {
      this.bendRing.position.copy(start);
      this.bendRing.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), normal.normalize());
      this.bendRing.scale.setScalar(Math.max(radius, 2));
      this.bendRing.visible = true;
    } else {
      this.bendRing.visible = false;
    }
    this.bendLine.geometry.setFromPoints([start, tip]);
    this.bendLine.computeLineDistances();
    this.bendLine.visible = true;
  }

  hideBend() {
    this.bendRing.visible = this.bendLine.visible = false;
  }

  private mouse(clientX: number, clientY: number) {
    const rect = this.viewport.renderer.domElement.getBoundingClientRect();
    return new THREE.Vector2(clientX - rect.left, clientY - rect.top);
  }
}
