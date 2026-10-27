import * as THREE from 'three';
import { GripDrag, type GripMode } from './grip-math';
import type { PullParams } from './protocol';
import type { Viewport } from './viewport';

const PICK_RADIUS_PX = { mouse: 14, touch: 28 };

/** Modusen brukeren har valgt i panelet. Alt og Alt + Shift overstyrer den under draget. */
export type PanelMode = 'pull' | 'plane' | 'lock';

const MODE_LABEL: Record<GripMode, string> = {
  pull: 'Trekk ut',
  plane: 'Flytt i plan',
  lock: 'Lås lengde og bøy',
  screen: 'Flytt tuppen',
};

export interface PullToolCallbacks {
  radiusMm: () => number;
  panelMode: () => PanelMode;
  /** Kalles under draget med siste grep. Klienten slår sammen raske oppdateringer. */
  preview: (shape: number, pull: PullParams, target: number | null) => void;
  /** `target` er uttrekkets plass i historikken ved redigering av en senterlinje, ellers null. */
  commit: (shape: number, pull: PullParams, target: number | null) => void;
  cancel: () => void;
  status: (text: string) => void;
}

interface Drag {
  pointerId: number;
  shape: number;
  /** Historikkplass for senterlinjen som redigeres, eller null for et nytt uttrekk. */
  target: number | null;
  radius: number;
  grip: GripDrag;
  mouse: THREE.Vector2;
  /** Grepets posisjon ved starten av draget. */
  startTip: THREE.Vector3;
  alt: boolean;
  shift: boolean;
}

/**
 * Formgrep (spesifikasjonen kap. 4–6). Hold over et kontrollpunkt for å se
 * pilen, og dra for å trekke ut eller presse inn. Alt flytter grepet i planet
 * vinkelrett på normalen, og Alt + Shift låser lengden A–B og bøyer. Etter
 * slipp kan tuppen av senterlinjen dras for å endre bøyningen. Escape
 * avbryter hele draget.
 */
export class PullTool {
  private drag: Drag | null = null;

  constructor(
    private viewport: Viewport,
    private cb: PullToolCallbacks,
  ) {
    const el = viewport.renderer.domElement;
    // Fanges før kamerastyringen, slik at et drag på et grep ikke også roterer kameraet.
    el.parentElement!.addEventListener('pointerdown', (e) => this.onDown(e), { capture: true });
    el.addEventListener('pointermove', (e) => this.onMove(e));
    el.addEventListener('pointerup', (e) => this.onUp(e));
    el.addEventListener('pointercancel', (e) => this.onPointerCancel(e));
    el.addEventListener('pointerleave', () => !this.drag && this.setHover(null));
    window.addEventListener('keydown', (e) => this.onKey(e));
    window.addEventListener('keyup', (e) => this.onKey(e));
  }

  get dragging() {
    return this.drag !== null;
  }

  private onDown(e: PointerEvent) {
    if (e.button !== 0 || this.drag || e.target !== this.viewport.renderer.domElement) return;
    const px = e.pointerType === 'touch' ? PICK_RADIUS_PX.touch : PICK_RADIUS_PX.mouse;
    const mouse = new THREE.Vector2(e.clientX, e.clientY);
    const line = this.viewport.pickCenterlineTip(e.clientX, e.clientY, px);
    const point = line ? null : this.viewport.pickControlPoint(e.clientX, e.clientY, px);
    if (!line && !point) return;

    // Hindrer at kamerastyringen tar dette draget.
    this.viewport.controls.enabled = false;
    this.viewport.renderer.domElement.setPointerCapture(e.pointerId);
    const anchor = line ? line.anchor : point!.position;
    const normal = line ? line.normal : point!.normal;
    const tip = line ? line.tip : point!.position;
    const target = line ? line.target : null;
    const grip = new GripDrag(anchor, normal, tip, this.mode(target, e.altKey, e.shiftKey), mouse, this.viewport.projector);
    this.drag = {
      pointerId: e.pointerId,
      shape: line ? line.shape : point!.shape,
      target,
      radius: line ? line.radius : this.cb.radiusMm(),
      grip,
      mouse,
      startTip: tip.clone(),
      alt: e.altKey,
      shift: e.shiftKey,
    };
    this.viewport.setControlPointsVisible(false);
    this.show(this.drag);
  }

  private onMove(e: PointerEvent) {
    const drag = this.drag;
    if (!drag) {
      if (e.pointerType === 'mouse') this.hoverAt(e.clientX, e.clientY);
      return;
    }
    if (e.pointerId !== drag.pointerId) return;
    drag.mouse.set(e.clientX, e.clientY);
    drag.alt = e.altKey;
    drag.shift = e.shiftKey;
    this.step(drag);
  }

  /** Alt og Shift kan trykkes og slippes midt i draget uten at pekeren beveges. */
  private onKey(e: KeyboardEvent) {
    if (e.type === 'keydown' && e.key === 'Escape') return this.cancel();
    const drag = this.drag;
    if (!drag || (e.key !== 'Alt' && e.key !== 'Shift')) return;
    // Hindrer at nettleseren åpner menyen på Alt under draget.
    e.preventDefault();
    drag.alt = e.altKey;
    drag.shift = e.shiftKey;
    this.step(drag);
  }

  private step(drag: Drag) {
    drag.grip.update(drag.mouse, this.mode(drag.target, drag.alt, drag.shift));
    this.show(drag);
    this.cb.preview(drag.shape, this.params(drag), drag.target);
  }

  private onUp(e: PointerEvent) {
    const drag = this.drag;
    if (!drag || e.pointerId !== drag.pointerId) return;
    this.end();
    if (drag.grip.tip.distanceTo(drag.startTip) < 0.01) return this.cb.cancel();
    this.cb.commit(drag.shape, this.params(drag), drag.target);
  }

  private onPointerCancel(e: PointerEvent) {
    if (this.drag && e.pointerId === this.drag.pointerId) this.cancel();
  }

  /** Avbryter hele draget og gjenoppretter utgangspunktet. */
  cancel() {
    if (!this.drag) return;
    this.end();
    this.cb.cancel();
    this.cb.status('Draget er avbrutt.');
  }

  private end() {
    const drag = this.drag!;
    this.drag = null;
    this.viewport.controls.enabled = true;
    this.viewport.setControlPointsVisible(true);
    this.viewport.hideGuide();
    if (this.viewport.renderer.domElement.hasPointerCapture(drag.pointerId)) {
      this.viewport.renderer.domElement.releasePointerCapture(drag.pointerId);
    }
    this.setHover(null);
  }

  private mode(target: number | null, alt: boolean, shift: boolean): GripMode {
    if (alt) return shift ? 'lock' : 'plane';
    const panel = this.cb.panelMode();
    // En eksisterende senterlinje flyttes fritt i synsplanet som standard.
    return panel === 'pull' && target !== null ? 'screen' : panel;
  }

  private params(drag: Drag): PullParams {
    const { anchor: a, normal: n, tip: b } = drag.grip;
    return [a.x, a.y, a.z, n.x, n.y, n.z, b.x, b.y, b.z, drag.radius];
  }

  private show(drag: Drag) {
    const g = drag.grip;
    const dir = g.length > 1e-6 ? g.tip.clone().sub(g.anchor).normalize() : g.normal;
    this.viewport.showGrip(g.tip, dir, drag.radius, g.anchor, g.normal);
    this.viewport.showGuide(g.anchor, g.tip);
    this.viewport.renderer.domElement.style.cursor = 'grabbing';
    const lock =
      g.mode === 'lock' && g.lockedLength === null ? ' · trekk ut før lengden kan låses' : '';
    const along = g.distanceAlongNormal;
    const what =
      g.mode === 'pull' ? `${along >= 0 ? 'ut' : 'inn'} ${Math.abs(along).toFixed(1)} mm` : `A–B ${g.length.toFixed(1)} mm`;
    this.cb.status(`${MODE_LABEL[g.mode]}: ${what}${lock} · Esc avbryter`);
  }

  private hoverAt(x: number, y: number) {
    const line = this.viewport.pickCenterlineTip(x, y, PICK_RADIUS_PX.mouse);
    if (line) {
      const dir = line.tip.clone().sub(line.anchor).normalize();
      this.viewport.showGrip(line.tip, dir, line.radius, line.anchor, line.normal);
      this.viewport.renderer.domElement.style.cursor = 'grab';
      return;
    }
    this.setHover(this.viewport.pickControlPoint(x, y, PICK_RADIUS_PX.mouse));
  }

  private setHover(point: { position: THREE.Vector3; normal: THREE.Vector3 } | null) {
    const el = this.viewport.renderer.domElement;
    if (point) {
      this.viewport.showGrip(point.position, point.normal, this.cb.radiusMm());
      el.style.cursor = 'grab';
    } else {
      this.viewport.hideGrip();
      el.style.cursor = '';
    }
  }
}
