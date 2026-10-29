import * as THREE from 'three';
import type { Vec3 } from './protocol';
import type { Viewport } from './viewport';

/** Lengden på pilene på skjermen, i piksler. */
const ARROW_PX = 70;

const AXES: { label: string; dir: Vec3; color: number }[] = [
  { label: '+X', dir: [1, 0, 0], color: 0xd64545 },
  { label: '−X', dir: [-1, 0, 0], color: 0xd64545 },
  { label: '+Y', dir: [0, 1, 0], color: 0x2f9e5b },
  { label: '−Y', dir: [0, -1, 0], color: 0x2f9e5b },
  { label: '+Z', dir: [0, 0, 1], color: 0x3a6fd8 },
  { label: '−Z', dir: [0, 0, -1], color: 0x3a6fd8 },
];

/** Valgt retning, og lengden når brukeren dro retningshåndtaket. */
export type AxisPick = (dir: Vec3, length?: number) => void;

/** Korteste uttrekk fra retningshåndtaket, i mm. */
const MIN_LENGTH_MM = 4;

/**
 * Aksekorset som vises ved «Ny spline»: seks piler langs ±X, ±Y og ±Z fra
 * startpunktet. Brukeren velger uttrekksretningen ved å trykke på en pil
 * (merkelappen ved pilspissen er en knapp, så den virker med mus, touch og
 * penn). Esc eller «Avbryt» lukker korset uten å lage noe.
 *
 * Ved «Ny gren» vises i tillegg et retningshåndtak: en pil i foreslått
 * retning som kan dras fritt i synsplanet. Slippes den, lages grenen i den
 * retningen og med den lengden; et trykk uten drag godtar forslaget.
 */
export class AxisGizmo {
  private group = new THREE.Group();
  private arrows: THREE.ArrowHelper[] = [];
  private labels: HTMLButtonElement[] = [];
  private bar: HTMLDivElement;
  private origin = new THREE.Vector3();
  private onPick: AxisPick | null = null;
  /** Retningshåndtaket for en ny gren: retning og lengde i mm. */
  private free: { dir: THREE.Vector3; length: number } | null = null;
  private freeArrow = new THREE.ArrowHelper(new THREE.Vector3(1, 0, 0), new THREE.Vector3(), 1, 0x7c4dff);
  private freeHandle: HTMLButtonElement;
  private freeDrag: { pointerId: number; moved: boolean; x: number; y: number } | null = null;

  constructor(private viewport: Viewport, container: HTMLElement) {
    this.group.visible = false;
    this.group.renderOrder = 5;
    for (const a of AXES) {
      const arrow = new THREE.ArrowHelper(new THREE.Vector3(...a.dir), new THREE.Vector3(), 1, a.color);
      // Negative retninger tegnes svakere, så de positive aksene er lette å kjenne igjen.
      for (const m of [arrow.line.material, arrow.cone.material] as THREE.Material[]) {
        m.depthTest = false;
        m.transparent = true;
        m.opacity = a.label.startsWith('+') ? 1 : 0.55;
      }
      arrow.renderOrder = 5;
      this.arrows.push(arrow);
      this.group.add(arrow);
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'axis-label';
      b.textContent = a.label;
      b.title = `Trekk ut langs ${a.label}`;
      b.style.setProperty('--axis', `#${a.color.toString(16).padStart(6, '0')}`);
      b.hidden = true;
      b.addEventListener('click', () => this.pick(a.dir));
      container.append(b);
      this.labels.push(b);
    }
    for (const m of [this.freeArrow.line.material, this.freeArrow.cone.material] as THREE.Material[]) m.depthTest = false;
    this.freeArrow.renderOrder = 6;
    this.freeArrow.visible = false;
    viewport.addToScene(this.freeArrow);
    this.freeHandle = document.createElement('button');
    this.freeHandle.type = 'button';
    this.freeHandle.className = 'axis-free';
    this.freeHandle.title = 'Dra for å velge retning og lengde, eller trykk for å godta';
    this.freeHandle.setAttribute('aria-label', 'Retningshåndtak for ny gren');
    this.freeHandle.hidden = true;
    this.freeHandle.addEventListener('pointerdown', (e) => this.onFreeDown(e));
    this.freeHandle.addEventListener('pointermove', (e) => this.onFreeMove(e));
    this.freeHandle.addEventListener('pointerup', (e) => this.onFreeUp(e));
    this.freeHandle.addEventListener('pointercancel', () => (this.freeDrag = null));
    container.append(this.freeHandle);
    this.bar = document.createElement('div');
    this.bar.className = 'gizmo-bar';
    this.bar.hidden = true;
    this.bar.innerHTML = '<span class="gizmo-text">Velg uttrekksretning</span>';
    const cancel = document.createElement('button');
    cancel.type = 'button';
    cancel.textContent = 'Avbryt';
    cancel.addEventListener('click', () => this.hide());
    this.bar.append(cancel);
    container.append(this.bar);
    viewport.addToScene(this.group);
    viewport.onFrame(() => this.place());
    window.addEventListener('keydown', (e) => e.key === 'Escape' && this.hide());
  }

  get open() {
    return this.group.visible;
  }

  /**
   * Viser korset i `origin`. `onPick` får valgt retning (enhetsvektor). Med
   * `free` vises også retningshåndtaket i den retningen og med den lengden.
   */
  show(origin: THREE.Vector3, onPick: AxisPick, free?: { dir: THREE.Vector3; length: number; text: string }) {
    this.origin.copy(origin);
    this.onPick = onPick;
    this.free = free ? { dir: free.dir.clone().normalize(), length: free.length } : null;
    this.bar.querySelector('.gizmo-text')!.textContent = free?.text ?? 'Velg uttrekksretning';
    this.group.visible = true;
    this.bar.hidden = false;
    for (const l of this.labels) l.hidden = false;
    this.freeHandle.hidden = !free;
    this.freeArrow.visible = !!free;
    this.place();
  }

  hide() {
    this.group.visible = false;
    this.bar.hidden = true;
    for (const l of this.labels) l.hidden = true;
    this.freeArrow.visible = false;
    this.freeHandle.hidden = true;
    this.free = null;
    this.freeDrag = null;
    this.onPick = null;
  }

  private pick(dir: Vec3, length?: number) {
    const f = this.onPick;
    this.hide();
    f?.(dir, length);
  }

  private onFreeDown(e: PointerEvent) {
    e.preventDefault();
    this.freeHandle.setPointerCapture(e.pointerId);
    this.freeDrag = { pointerId: e.pointerId, moved: false, x: e.clientX, y: e.clientY };
  }

  private onFreeMove(e: PointerEvent) {
    const d = this.freeDrag;
    if (!d || d.pointerId !== e.pointerId || !this.free) return;
    if (Math.hypot(e.clientX - d.x, e.clientY - d.y) > 3) d.moved = true;
    const p = this.viewport.pointInViewPlane(e.clientX, e.clientY, this.origin);
    if (!p) return;
    const v = p.sub(this.origin);
    if (v.length() < 1e-6) return;
    this.free.length = Math.max(v.length(), MIN_LENGTH_MM);
    this.free.dir.copy(v.normalize());
    this.place();
  }

  private onFreeUp(e: PointerEvent) {
    const d = this.freeDrag;
    if (!d || d.pointerId !== e.pointerId || !this.free) return;
    this.freeDrag = null;
    const { dir, length } = this.free;
    this.pick([dir.x, dir.y, dir.z], length);
  }

  /** Holder pilene like lange på skjermen og merkelappene ved pilspissene. */
  private place() {
    if (!this.group.visible) return;
    const len = ARROW_PX / this.viewport.pixelsPerMm(this.origin);
    this.group.position.copy(this.origin);
    this.arrows.forEach((arrow, i) => {
      arrow.setLength(len, len * 0.22, len * 0.12);
      const tip = this.origin.clone().addScaledVector(new THREE.Vector3(...AXES[i].dir), len * 1.18);
      const p = this.viewport.toScreen(tip);
      this.labels[i].style.transform = `translate(${p.x}px, ${p.y}px) translate(-50%, -50%)`;
    });
    if (this.free) {
      const { dir, length } = this.free;
      this.freeArrow.position.copy(this.origin);
      this.freeArrow.setDirection(dir);
      this.freeArrow.setLength(length, Math.min(length * 0.25, len * 0.22), Math.min(length * 0.14, len * 0.12));
      const p = this.viewport.toScreen(this.origin.clone().addScaledVector(dir, length));
      this.freeHandle.style.transform = `translate(${p.x}px, ${p.y}px) translate(-50%, -50%)`;
    }
  }
}
