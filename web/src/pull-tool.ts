import * as THREE from 'three';
import type { ControlPoint, Viewport } from './viewport';
import type { PullParams } from './protocol';

const PICK_RADIUS_PX = { mouse: 14, touch: 28 };

export interface PullToolCallbacks {
  radiusMm: () => number;
  /** Kalles under draget med siste grep. Klienten slår sammen raske oppdateringer. */
  preview: (shape: number, pull: PullParams) => void;
  commit: (shape: number, pull: PullParams) => void;
  cancel: (shape: number) => void;
  status: (text: string) => void;
}

interface Drag {
  pointerId: number;
  point: ControlPoint;
  start: THREE.Vector2;
  /** Skjermretning for uttrekksretningen og piksler per mm langs den. */
  screenDir: THREE.Vector2;
  pxPerMm: number;
  distance: number;
}

/**
 * Formgrep: hold pekeren over et kontrollpunkt for å se pilen, og dra for å
 * trekke ut eller presse inn (spesifikasjonen kap. 4). Retningen låses ved
 * starten av draget. Escape avbryter og gjenoppretter formen.
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
    el.addEventListener('pointercancel', (e) => this.onCancel(e));
    el.addEventListener('pointerleave', () => !this.drag && this.setHover(null));
    window.addEventListener('keydown', (e) => e.key === 'Escape' && this.cancel());
  }

  get dragging() {
    return this.drag !== null;
  }

  private onDown(e: PointerEvent) {
    if (e.button !== 0 || this.drag || e.target !== this.viewport.renderer.domElement) return;
    const radius = e.pointerType === 'touch' ? PICK_RADIUS_PX.touch : PICK_RADIUS_PX.mouse;
    const point = this.viewport.pickControlPoint(e.clientX, e.clientY, radius);
    if (!point) return;
    // Hindrer at kamerastyringen tar dette draget.
    this.viewport.controls.enabled = false;
    this.viewport.renderer.domElement.setPointerCapture(e.pointerId);

    const a = this.viewport.toScreen(point.position);
    const b = this.viewport.toScreen(point.position.clone().add(point.normal));
    const screenDir = b.sub(a);
    const perMm = this.viewport.pixelsPerMm(point.position);
    // Peker retningen nesten mot kameraet, brukes loddrett musebevegelse i stedet.
    const usable = screenDir.length() > 0.3 * perMm;
    this.drag = {
      pointerId: e.pointerId,
      point,
      start: new THREE.Vector2(e.clientX, e.clientY),
      screenDir: usable ? screenDir.clone().normalize() : new THREE.Vector2(0, -1),
      pxPerMm: usable ? screenDir.length() : perMm,
      distance: 0,
    };
    this.setHover(point);
    this.viewport.setControlPointsVisible(false);
  }

  private onMove(e: PointerEvent) {
    const drag = this.drag;
    if (!drag) {
      if (e.pointerType === 'mouse') {
        this.setHover(this.viewport.pickControlPoint(e.clientX, e.clientY, PICK_RADIUS_PX.mouse));
      }
      return;
    }
    if (e.pointerId !== drag.pointerId) return;
    const delta = new THREE.Vector2(e.clientX, e.clientY).sub(drag.start);
    drag.distance = delta.dot(drag.screenDir) / drag.pxPerMm;
    this.showDrag(drag);
    this.cb.preview(drag.point.shape, this.params(drag));
  }

  private onUp(e: PointerEvent) {
    const drag = this.drag;
    if (!drag || e.pointerId !== drag.pointerId) return;
    this.end();
    if (Math.abs(drag.distance) < 0.01) return this.cb.cancel(drag.point.shape);
    this.cb.commit(drag.point.shape, this.params(drag));
  }

  private onCancel(e: PointerEvent) {
    if (this.drag && e.pointerId === this.drag.pointerId) this.cancel();
  }

  /** Avbryter hele draget og gjenoppretter utgangspunktet. */
  cancel() {
    const drag = this.drag;
    if (!drag) return;
    this.end();
    this.cb.cancel(drag.point.shape);
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
    const { position: a, normal: n } = drag.point;
    return [a.x, a.y, a.z, n.x, n.y, n.z, drag.distance, this.cb.radiusMm()];
  }

  private showDrag(drag: Drag) {
    const tip = drag.point.position.clone().addScaledVector(drag.point.normal, drag.distance);
    this.viewport.showGrip(tip, drag.point.normal, this.cb.radiusMm(), drag.point.position);
    const verb = drag.distance >= 0 ? 'Trekker ut' : 'Presser inn';
    this.cb.status(`${verb} ${Math.abs(drag.distance).toFixed(1)} mm · Esc avbryter`);
  }

  private setHover(point: ControlPoint | null) {
    const el = this.viewport.renderer.domElement;
    if (point) {
      this.viewport.showGrip(point.position, point.normal, this.cb.radiusMm());
      el.style.cursor = this.drag ? 'grabbing' : 'grab';
    } else {
      this.viewport.hideGrip();
      el.style.cursor = '';
    }
  }
}
