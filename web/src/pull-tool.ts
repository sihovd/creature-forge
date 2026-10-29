import * as THREE from 'three';
import { GripDrag } from './grip-math';
import type { PullParams } from './protocol';
import type { Viewport } from './viewport';

const PICK_RADIUS_PX = { mouse: 14, touch: 28 };

export interface PullToolCallbacks {
  radiusMm: () => number;
  /** Kalles under draget med siste grep. Klienten slår sammen raske oppdateringer. */
  preview: (shape: number, pull: PullParams) => void;
  commit: (shape: number, pull: PullParams) => void;
  cancel: () => void;
  status: (text: string) => void;
}

interface Drag {
  pointerId: number;
  shape: number;
  radius: number;
  grip: GripDrag;
  mouse: THREE.Vector2;
}

/**
 * Formgrep med kontrollpunktene (spesifikasjonen kap. 4). Hold over et punkt
 * for å se pilen, og dra for å trekke ut eller presse inn langs normalen.
 * Grepet deformerer meshen direkte og lager ingen spline; bøying og
 * forlengelse gjøres med skjelettet. Escape avbryter hele draget.
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
    el.addEventListener('pointercancel', (e) => this.drag?.pointerId === e.pointerId && this.cancel());
    el.addEventListener('pointerleave', () => !this.drag && this.setHover(null));
    window.addEventListener('keydown', (e) => e.key === 'Escape' && this.cancel());
  }

  get dragging() {
    return this.drag !== null;
  }

  private onDown(e: PointerEvent) {
    if (e.button !== 0 || this.drag || e.target !== this.viewport.renderer.domElement || this.viewport.interactionLocked) return;
    const px = e.pointerType === 'touch' ? PICK_RADIUS_PX.touch : PICK_RADIUS_PX.mouse;
    const point = this.viewport.pickControlPoint(e.clientX, e.clientY, px);
    if (!point) return;

    // Hindrer at kamerastyringen tar dette draget.
    this.viewport.controls.enabled = false;
    this.viewport.renderer.domElement.setPointerCapture(e.pointerId);
    const mouse = new THREE.Vector2(e.clientX, e.clientY);
    const grip = new GripDrag(point.position, point.normal, point.position, 'pull', mouse, this.viewport.projector);
    this.drag = { pointerId: e.pointerId, shape: point.shape, radius: this.cb.radiusMm(), grip, mouse };
    this.viewport.setControlPointsVisible(false);
    this.show(this.drag);
  }

  private onMove(e: PointerEvent) {
    const drag = this.drag;
    if (!drag) {
      if (e.pointerType === 'mouse' && !this.viewport.interactionLocked) this.setHover(this.viewport.pickControlPoint(e.clientX, e.clientY, PICK_RADIUS_PX.mouse));
      return;
    }
    if (e.pointerId !== drag.pointerId) return;
    drag.mouse.set(e.clientX, e.clientY);
    drag.grip.update(drag.mouse, 'pull');
    this.show(drag);
    this.cb.preview(drag.shape, this.params(drag));
  }

  private onUp(e: PointerEvent) {
    const drag = this.drag;
    if (!drag || e.pointerId !== drag.pointerId) return;
    this.end();
    if (Math.abs(drag.grip.distanceAlongNormal) < 0.01) return this.cb.cancel();
    this.cb.commit(drag.shape, this.params(drag));
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
    if (this.viewport.renderer.domElement.hasPointerCapture(drag.pointerId)) {
      this.viewport.renderer.domElement.releasePointerCapture(drag.pointerId);
    }
    this.setHover(null);
  }

  private params(drag: Drag): PullParams {
    const { anchor: a, normal: n, tip: b } = drag.grip;
    return [a.x, a.y, a.z, n.x, n.y, n.z, b.x, b.y, b.z, drag.radius];
  }

  private show(drag: Drag) {
    const g = drag.grip;
    const dir = g.distanceAlongNormal >= 0 ? g.normal : g.normal.clone().negate();
    this.viewport.showGrip(g.tip, dir, drag.radius, g.anchor, g.normal);
    this.viewport.renderer.domElement.style.cursor = 'grabbing';
    const along = g.distanceAlongNormal;
    this.cb.status(`${along >= 0 ? 'Trekk ut' : 'Press inn'} ${Math.abs(along).toFixed(1)} mm · Esc avbryter`);
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
