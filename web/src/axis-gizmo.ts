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

/**
 * Aksekorset som vises ved «Ny spline»: seks piler langs ±X, ±Y og ±Z fra
 * startpunktet. Brukeren velger uttrekksretningen ved å trykke på en pil
 * (merkelappen ved pilspissen er en knapp, så den virker med mus, touch og
 * penn). Esc eller «Avbryt» lukker korset uten å lage noe.
 */
export class AxisGizmo {
  private group = new THREE.Group();
  private arrows: THREE.ArrowHelper[] = [];
  private labels: HTMLButtonElement[] = [];
  private bar: HTMLDivElement;
  private origin = new THREE.Vector3();
  private onPick: ((dir: Vec3) => void) | null = null;

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
    this.bar = document.createElement('div');
    this.bar.className = 'gizmo-bar';
    this.bar.hidden = true;
    this.bar.innerHTML = '<span>Velg uttrekksretning</span>';
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

  /** Viser korset i `origin`. `onPick` får valgt retning (enhetsvektor). */
  show(origin: THREE.Vector3, onPick: (dir: Vec3) => void) {
    this.origin.copy(origin);
    this.onPick = onPick;
    this.group.visible = true;
    this.bar.hidden = false;
    for (const l of this.labels) l.hidden = false;
    this.place();
  }

  hide() {
    this.group.visible = false;
    this.bar.hidden = true;
    for (const l of this.labels) l.hidden = true;
    this.onPick = null;
  }

  private pick(dir: Vec3) {
    const f = this.onPick;
    this.hide();
    f?.(dir);
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
  }
}
