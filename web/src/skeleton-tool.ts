import * as THREE from 'three';
import { GripDrag } from './grip-math';
import type { SkeletonEdit, SplineView, Vec3 } from './protocol';
import { BendDrag } from './skeleton-math';
import type { SkeletonPick, SkeletonView, SnapTarget } from './skeleton-view';
import type { Viewport } from './viewport';

const PICK_RADIUS_PX = { mouse: 12, touch: 26 };
/** Kurven gripes nærmere enn punktene, så prikkene rundt den kan brukes. */
const CURVE_RADIUS_PX = { mouse: 6, touch: 16 };
/** Avstand på skjermen der enden av en spline snapper til en annen spline. */
const SNAP_RADIUS_PX = 14;
const v3 = (p: Vec3) => new THREE.Vector3(p[0], p[1], p[2]);
const arr = (v: THREE.Vector3): Vec3 => [v.x, v.y, v.z];
const deg = (rad: number) => THREE.MathUtils.radToDeg(rad).toFixed(0);

export interface SkeletonToolCallbacks {
  /** Forhåndsviser en redigering og gir tilbake skjelettet slik det ville blitt. */
  preview: (edit: SkeletonEdit) => Promise<SplineView[] | null>;
  /** Lagrer redigeringen som ett angresteg. */
  commit: (edit: SkeletonEdit, describe: string) => void;
  /** Viser siste lagrede skjelett igjen. */
  restore: () => void;
  status: (text: string) => void;
  selectionChanged: () => void;
  /** Aktivt verktøy: «Velg» eller «Bézier» (trykk på kurven setter inn et punkt). */
  tool: () => 'select' | 'bezier';
}

/** Et trykk regnes som trykk og ikke drag når pekeren flyttes mindre enn dette. */
const TAP_PX = 5;

type TipMode = 'pull' | 'plane' | 'bend';

/** Hele formen flyttes ved å gripe kurven til en rotspline. */
type CurvePick = { kind: 'curve'; spline: number; index: number; position: THREE.Vector3 };

interface Drag {
  pointerId: number;
  pick: SkeletonPick | CurvePick;
  startMouse: THREE.Vector2;
  mouse: THREE.Vector2;
  moved: boolean;
  /** Draget startet på tuppen. Tuppen er fortsatt siste punkt når bøyen legger inn flere. */
  tip: boolean;
  alt: boolean;
  shift: boolean;
  ctrl: boolean;
  /** Redigeringer fra tidligere moduser i samme drag. */
  frozen: SkeletonEdit[];
  mode: TipMode | 'free';
  grip: GripDrag | null;
  bend: { drag: BendDrag; from: number; start: THREE.Vector3 } | null;
  /** Punktet på en annen spline som enden snapper til. */
  snap: SnapTarget | null;
  /** Fra pekeren til det grepne punktet på skjermen, ved starten av draget. */
  grabOffset: THREE.Vector2;
}

/**
 * Redigering av spline-skjelettet (issue #4). Ankerpunkter og håndtak dras i
 * synsplanet. Tuppen av en spline dras ut langs enderetningen; Alt panorerer
 * den i planet vinkelrett på enderetningen, og Alt + Shift bøyer resten av
 * splinen fra valgt punkt med låst buelengde. Under bøying stiller scroll
 * radien og Ctrl snapper vinkelen til 15°. Festet til en gren dras langs
 * forelderen. Dobbeltklikk på en kurve legger inn et nytt punkt. Grip
 * kurven til en rotspline for å flytte hele formen. Når et punkt eller
 * tuppen dras nær en annen spline, snapper det dit (Shift slår av snapping).
 * Hvert drag er ett angresteg, og Esc avbryter.
 */
export class SkeletonTool {
  private drag: Drag | null = null;
  /** Skjelettet slik det sist ble vist under draget. */
  private latest: SplineView[] = [];
  private inFlight = false;
  private queued: SkeletonEdit | null = null;
  /** Et trykk på kurven med Bézier-verktøyet som setter inn et punkt når pekeren slippes. */
  private insert: { pointerId: number; x: number; y: number; spline: number; at: number } | null = null;
  /** Pekeren som sist startet et drag i skjelettet, så andre trykkhandlinger kan hoppe over den. */
  handledPointer: number | null = null;

  constructor(
    private viewport: Viewport,
    private view: SkeletonView,
    private cb: SkeletonToolCallbacks,
  ) {
    const el = viewport.renderer.domElement;
    // Fanges før formgrepet og kamerastyringen.
    el.parentElement!.addEventListener('pointerdown', (e) => this.onDown(e), { capture: true });
    el.addEventListener('pointermove', (e) => this.onMove(e));
    el.addEventListener('pointerup', (e) => this.onUp(e));
    el.addEventListener('pointercancel', (e) => this.drag?.pointerId === e.pointerId && this.cancel());
    el.addEventListener('dblclick', (e) => this.onDoubleClick(e));
    el.parentElement!.addEventListener('wheel', (e) => this.onWheel(e), { capture: true, passive: false });
    window.addEventListener('pointermove', (e) => {
      if (this.insert?.pointerId === e.pointerId && Math.hypot(e.clientX - this.insert.x, e.clientY - this.insert.y) > TAP_PX) this.insert = null;
    });
    window.addEventListener('pointerup', (e) => this.onInsertUp(e));
    window.addEventListener('keydown', (e) => this.onKey(e));
    window.addEventListener('keyup', (e) => this.onKey(e));
  }

  get dragging() {
    return this.drag !== null;
  }

  private onDown(e: PointerEvent) {
    if (e.button !== 0 || this.drag || e.target !== this.viewport.renderer.domElement) return;
    const touch = e.pointerType === 'touch';
    const px = touch ? PICK_RADIUS_PX.touch : PICK_RADIUS_PX.mouse;
    let pick: SkeletonPick | CurvePick | null = this.view.pick(e.clientX, e.clientY, px);
    const curvePx = touch ? CURVE_RADIUS_PX.touch : CURVE_RADIUS_PX.mouse;
    if (!pick && this.cb.tool() === 'bezier') {
      // Bézier: et trykk på kurven setter inn et punkt. Draget går til
      // kameraet, så et drag ved siden av et punkt flytter verken punkt eller form.
      const near = this.view.pickCurve(e.clientX, e.clientY, curvePx);
      this.insert = near ? { pointerId: e.pointerId, x: e.clientX, y: e.clientY, spline: near.spline, at: near.at } : null;
      return;
    }
    if (!pick) {
      const near = this.view.pickCurve(e.clientX, e.clientY, curvePx);
      if (near && this.view.spline(near.spline)?.parent === null) {
        pick = { kind: 'curve', spline: near.spline, index: -1, position: near.position };
      }
    }
    if (!pick) return;
    // Skjelettet ligger oppå formene og har forrang foran formgrep og kamera.
    e.stopImmediatePropagation();
    this.handledPointer = e.pointerId;
    this.viewport.controls.enabled = false;
    // Kontrollpunktene gjelder meshen før draget og skjules til den er beregnet på nytt.
    this.viewport.setControlPointsVisible(false);
    this.viewport.renderer.domElement.setPointerCapture(e.pointerId);
    this.latest = this.view.splines;
    const mouse = new THREE.Vector2(e.clientX, e.clientY);
    const previous = this.view.selection;
    this.view.select({ spline: pick.spline, index: pick.kind === 'curve' ? null : pick.index });
    this.cb.selectionChanged();
    this.drag = {
      pointerId: e.pointerId,
      pick,
      startMouse: mouse.clone(),
      mouse,
      moved: false,
      tip: this.isTip(pick),
      alt: e.altKey,
      shift: e.shiftKey,
      ctrl: e.ctrlKey || e.metaKey,
      frozen: [],
      mode: 'free',
      grip: null,
      bend: null,
      snap: null,
      grabOffset: this.viewport.toScreen(pick.position).sub(this.canvasPoint(mouse)),
    };
    // Bøyen starter i punktet som var valgt før tuppen ble grepet.
    this.bendFrom = previous?.spline === pick.spline ? previous.index : null;
    this.enterMode(this.drag);
  }

  private bendFrom: number | null = null;

  private isTip(pick: SkeletonPick | CurvePick) {
    const s = this.spline(pick.spline);
    return pick.kind === 'anchor' && !!s && pick.index === s.anchors.length - 1;
  }

  private spline(id: number) {
    return this.latest.find((s) => s.id === id) ?? null;
  }

  private tipMode(drag: Drag): TipMode | 'free' {
    if (!drag.tip) return 'free';
    if (drag.alt) return drag.shift ? 'bend' : 'plane';
    return 'pull';
  }

  /** Starter en modus fra skjelettets nåværende tilstand, så formen ikke hopper. */
  private enterMode(drag: Drag) {
    const mode = this.tipMode(drag);
    // Mellom uttrekk og panorering bytter grepet modus selv, uten hopp.
    if ((mode === 'pull' || mode === 'plane') && (drag.mode === 'pull' || drag.mode === 'plane') && drag.grip) {
      drag.mode = mode;
      drag.grip.update(drag.mouse, mode);
      return;
    }
    const current = drag.mode === 'free' && drag.grip === null && drag.bend === null ? null : this.edit(drag, false);
    if (current && drag.moved) drag.frozen.push(current);
    drag.mode = mode;
    drag.grip = null;
    drag.bend = null;
    this.view.hideBend();
    const s = this.spline(drag.pick.spline)!;
    const a = drag.tip ? s.anchors[s.anchors.length - 1] : s.anchors[drag.pick.index];
    const projector = this.viewport.projector;
    if (mode === 'bend') {
      const last = s.anchors.length - 1;
      const from = this.bendFrom !== null && this.bendFrom < last ? this.bendFrom : 0;
      const f = s.anchors[from];
      const drag2 = new BendDrag(v3(f.tangent), v3(s.anchors[last].tangent), s.length * (1 - f.at), drag.mouse, projector);
      drag.bend = { drag: drag2, from, start: v3(f.position) };
    } else if (mode === 'pull' || mode === 'plane') {
      const p = v3(a.position);
      drag.grip = new GripDrag(p, v3(a.tangent).normalize(), p, mode, drag.mouse, projector);
    } else {
      const p = drag.pick.position;
      drag.grip = new GripDrag(p, new THREE.Vector3(0, 1, 0), p, 'screen', drag.mouse, projector);
    }
  }

  private onMove(e: PointerEvent) {
    const drag = this.drag;
    if (!drag || e.pointerId !== drag.pointerId) return;
    drag.mouse.set(e.clientX, e.clientY);
    if (drag.mouse.distanceTo(drag.startMouse) > 2) drag.moved = true;
    this.setKeys(drag, e.altKey, e.shiftKey, e.ctrlKey || e.metaKey);
    this.step(drag);
  }

  private onKey(e: KeyboardEvent) {
    if (e.type === 'keydown' && e.key === 'Escape') return this.cancel();
    const drag = this.drag;
    if (!drag || !['Alt', 'Shift', 'Control', 'Meta'].includes(e.key)) return;
    e.preventDefault();
    this.setKeys(drag, e.altKey, e.shiftKey, e.ctrlKey || e.metaKey);
    this.step(drag);
  }

  private setKeys(drag: Drag, alt: boolean, shift: boolean, ctrl: boolean) {
    drag.alt = alt;
    drag.shift = shift;
    drag.ctrl = ctrl;
    if (this.tipMode(drag) !== drag.mode) this.enterMode(drag);
  }

  private onWheel(e: WheelEvent) {
    const drag = this.drag;
    if (!drag?.bend) return;
    // Scroll stiller bøyeradien under bøying i stedet for å zoome.
    e.preventDefault();
    e.stopPropagation();
    drag.bend.drag.scroll(e.deltaY);
    drag.moved = true;
    this.step(drag);
  }

  private step(drag: Drag) {
    if (drag.grip) drag.grip.update(drag.mouse, drag.grip.mode);
    if (drag.bend) drag.bend.drag.update(drag.mouse);
    if (!drag.moved) return;
    drag.snap = this.findSnap(drag);
    this.view.showSnap(drag.snap);
    this.cb.status(this.describe(drag) + ' · Esc avbryter');
    this.requestPreview(this.edit(drag, true));
  }

  private canvasPoint(client: THREE.Vector2) {
    const rect = this.viewport.renderer.domElement.getBoundingClientRect();
    return new THREE.Vector2(client.x - rect.left, client.y - rect.top);
  }

  /**
   * Et punkt eller tuppen som dras, snapper til andre spliner. Målet finnes
   * fra pekeren, siden det er der brukeren ser, og grepet følger etter.
   */
  private findSnap(drag: Drag): SnapTarget | null {
    const { pick } = drag;
    if (!drag.grip || drag.shift || pick.kind !== 'anchor') return null;
    if (pick.index === 0 && this.spline(pick.spline)?.parent != null) return null;
    // Splinen selv og grenene på den kan ikke være mål.
    const exclude = new Set<number>([pick.spline]);
    for (let grew = true; grew; ) {
      grew = false;
      for (const s of this.latest) {
        if (s.parent !== null && exclude.has(s.parent) && !exclude.has(s.id)) {
          exclude.add(s.id);
          grew = true;
        }
      }
    }
    const screen = this.canvasPoint(drag.mouse).add(drag.grabOffset);
    return this.view.snapTarget(screen, SNAP_RADIUS_PX, exclude);
  }

  /** Redigeringen for hele draget så langt. */
  private edit(drag: Drag, all: boolean): SkeletonEdit {
    const { pick } = drag;
    let current: SkeletonEdit;
    if (drag.bend) {
      const b = drag.bend.drag.params(drag.ctrl);
      current = { kind: 'bend', spline: pick.spline, from: drag.bend.from, direction: arr(b.direction), angle: b.angle, radius: b.radius };
    } else if (pick.kind === 'curve') {
      const delta = drag.grip!.tip.clone().sub(pick.position);
      current = { kind: 'translate', spline: pick.spline, delta: arr(delta) };
    } else if (pick.kind === 'handle') {
      current = { kind: 'setHandle', spline: pick.spline, index: pick.index, side: pick.side, position: arr(drag.grip!.tip) };
    } else if (pick.index === 0 && this.spline(pick.spline)?.parent != null) {
      // Festet dras langs forelderen.
      const parent = this.spline(pick.spline)!.parent!;
      const near = this.view.pickCurve(drag.mouse.x, drag.mouse.y, Infinity, parent);
      current = { kind: 'reattach', spline: pick.spline, at: near ? near.at : this.spline(pick.spline)!.at };
    } else if (drag.tip) {
      current = { kind: 'moveTip', spline: pick.spline, position: arr(drag.snap?.position ?? drag.grip!.tip) };
    } else {
      current = { kind: 'moveAnchor', spline: pick.spline, index: pick.index, position: arr(drag.snap?.position ?? drag.grip!.tip) };
    }
    if (!all || drag.frozen.length === 0) return current;
    return { kind: 'batch', edits: [...drag.frozen, current] };
  }

  private describe(drag: Drag) {
    const snap = drag.snap ? ` · snapper til spline ${drag.snap.spline} (Shift slår av)` : '';
    return this.describeMove(drag) + snap;
  }

  private describeMove(drag: Drag) {
    if (drag.pick.kind === 'curve') return 'Flytt hele formen';
    if (drag.bend) {
      const b = drag.bend.drag.params(drag.ctrl);
      const snap = drag.ctrl ? ' (snapper 15°)' : '';
      const r = b.radius < 0.05 ? 'skarp knekk' : `radius ${b.radius.toFixed(1)} mm`;
      return `Bøy ${deg(b.angle)}°${snap} · ${r} · lengden er låst · scroll endrer radius`;
    }
    if (drag.mode === 'pull') return 'Trekk ut langs enden';
    if (drag.mode === 'plane') return 'Panorer tuppen (Alt)';
    if (drag.pick.kind === 'handle') return 'Flytt håndtak';
    return drag.pick.index === 0 && this.spline(drag.pick.spline)?.parent != null ? 'Flytt festet langs forelderen' : 'Flytt punkt';
  }

  private requestPreview(edit: SkeletonEdit) {
    this.queued = edit;
    if (this.inFlight) return;
    const run = async () => {
      while (this.queued && this.drag) {
        const next = this.queued;
        this.queued = null;
        this.inFlight = true;
        const result = await this.cb.preview(next).catch(() => null);
        this.inFlight = false;
        // Et ugyldig mellomsteg overskriver ikke siste gyldige visning.
        if (result && this.drag) {
          this.latest = result;
          this.view.set(result);
          this.showBend();
        }
      }
    };
    run();
  }

  private showBend() {
    const b = this.drag?.bend;
    if (!b) return;
    const s = this.spline(this.drag!.pick.spline);
    if (!s) return;
    const params = b.drag.params(this.drag!.ctrl);
    this.view.showBend(b.start, b.drag.tangent, params.direction, params.radius, v3(s.anchors[s.anchors.length - 1].position));
  }

  private onUp(e: PointerEvent) {
    const drag = this.drag;
    if (!drag || e.pointerId !== drag.pointerId) return;
    this.end();
    if (!drag.moved) {
      this.cb.status('Valgt punkt. Dra for å flytte, eller grip tuppen og hold Alt + Shift for å bøye herfra.');
      return;
    }
    this.cb.commit(this.edit(drag, true), this.describe(drag).split(' · scroll')[0]);
  }

  cancel() {
    if (!this.drag) return;
    this.end();
    this.cb.restore();
    this.cb.status('Draget er avbrutt.');
  }

  private end() {
    const drag = this.drag!;
    this.drag = null;
    this.queued = null;
    this.view.hideBend();
    this.view.showSnap(null);
    this.viewport.controls.enabled = true;
    this.viewport.setControlPointsVisible(true);
    const el = this.viewport.renderer.domElement;
    if (el.hasPointerCapture(drag.pointerId)) el.releasePointerCapture(drag.pointerId);
  }

  private onInsertUp(e: PointerEvent) {
    const ins = this.insert;
    if (!ins || ins.pointerId !== e.pointerId) return;
    this.insert = null;
    this.handledPointer = e.pointerId;
    this.view.select({ spline: ins.spline, index: null });
    this.cb.selectionChanged();
    this.cb.commit({ kind: 'insertAnchor', spline: ins.spline, at: ins.at }, 'Satte inn et ankerpunkt');
  }

  private onDoubleClick(e: MouseEvent) {
    const near = this.view.pickCurve(e.clientX, e.clientY, PICK_RADIUS_PX.mouse);
    if (!near || this.view.pick(e.clientX, e.clientY, PICK_RADIUS_PX.mouse)) return;
    this.cb.commit({ kind: 'insertAnchor', spline: near.spline, at: near.at }, 'La inn et nytt punkt');
  }
}
