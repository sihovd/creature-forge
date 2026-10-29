import type { Profile, Vec2 } from './protocol';
import { insertPoint, movePoint, nearestOnOutline, polygon, polygonProfile, setHandle } from './profile-math';

const SVG = 'http://www.w3.org/2000/svg';
/** Synlig område i enheter av tykkelsen (sirkelen har radius 1). */
const VIEW = 1.6;
const PICK_PX = 9;

export interface ProfileEditorCallbacks {
  /** Viser omrisset på modellen uten å lagre (under drag). */
  preview: (profile: Profile) => void;
  /** Lagrer omrisset som ett angresteg. */
  commit: (profile: Profile, describe: string) => void;
  /** Valgt punkt er endret. */
  selectionChanged: () => void;
  status: (text: string) => void;
}

type Grab = { kind: 'point'; index: number } | { kind: 'handle'; index: number; side: 'in' | 'out' };

/**
 * Tverrsnittseditoren: omrisset til valgt kontur i sitt eget plan, med
 * x langs U og y langs V i splinens ramme. Dra punkter og håndtak,
 * dobbeltklikk på omrisset for et nytt punkt. I tegnemodus legger hvert
 * klikk et hjørnepunkt, og klikk på første punkt lukker omrisset.
 */
export class ProfileEditor {
  readonly svg: SVGSVGElement;
  profile: Profile | null = null;
  selected: number | null = null;
  readOnly = false;
  private drawing: Vec2[] | null = null;
  private grab: { pointerId: number; what: Grab; moved: boolean; work: Profile } | null = null;
  private cursor: Vec2 | null = null;
  /** Forrige trykk, for dobbeltklikk. Elementene tegnes på nytt ved hvert
   * trykk, så nettleserens eget dblclick kommer ikke alltid frem. */
  private lastDown: { time: number; x: number; y: number } | null = null;

  constructor(parent: HTMLElement, private cb: ProfileEditorCallbacks) {
    this.svg = document.createElementNS(SVG, 'svg');
    this.svg.setAttribute('viewBox', `${-VIEW} ${-VIEW} ${2 * VIEW} ${2 * VIEW}`);
    this.svg.setAttribute('class', 'profile-svg');
    this.svg.setAttribute('role', 'img');
    this.svg.setAttribute('aria-label', 'Tverrsnittsomriss');
    parent.append(this.svg);
    this.svg.addEventListener('pointerdown', (e) => this.onDown(e));
    this.svg.addEventListener('pointermove', (e) => this.onMove(e));
    this.svg.addEventListener('pointerup', (e) => this.onUp(e));
    this.svg.addEventListener('pointercancel', () => this.cancelGrab());
    this.render();
  }

  get isDrawing() {
    return this.drawing !== null;
  }

  set(profile: Profile | null, readOnly: boolean) {
    if (this.grab) return;
    this.profile = profile;
    this.readOnly = readOnly;
    if (!profile || (this.selected !== null && this.selected >= profile.points.length)) this.selected = null;
    this.render();
  }

  /** Starter tegning av et nytt omriss som erstatter det gamle. */
  startDrawing() {
    this.drawing = [];
    this.selected = null;
    this.cb.status('Klikk i konturfeltet for å sette punkter. Klikk på første punkt for å lukke, Esc avbryter.');
    this.render();
  }

  stopDrawing() {
    this.drawing = null;
    this.render();
  }

  /** Musposisjon i profilplanet (y opp). */
  private local(e: { clientX: number; clientY: number }): Vec2 {
    const r = this.svg.getBoundingClientRect();
    const x = ((e.clientX - r.left) / r.width) * 2 * VIEW - VIEW;
    const y = VIEW - ((e.clientY - r.top) / r.height) * 2 * VIEW;
    return [x, y];
  }

  private unitsPerPx() {
    return (2 * VIEW) / Math.max(1, this.svg.getBoundingClientRect().width);
  }

  private pick(q: Vec2): Grab | null {
    const p = this.profile;
    if (!p) return null;
    const max = PICK_PX * this.unitsPerPx();
    let best: Grab | null = null;
    let bestD = max;
    const near = (pt: Vec2, g: Grab) => {
      const d = Math.hypot(pt[0] - q[0], pt[1] - q[1]);
      if (d <= bestD) {
        bestD = d;
        best = g;
      }
    };
    p.points.forEach((pt, index) => near(pt.position, { kind: 'point', index }));
    if (this.selected !== null) {
      const a = p.points[this.selected];
      for (const side of ['in', 'out'] as const) {
        const h = side === 'in' ? a.handle_in : a.handle_out;
        if (Math.hypot(h[0], h[1]) > 1e-9) near([a.position[0] + h[0], a.position[1] + h[1]], { kind: 'handle', index: this.selected, side });
      }
    }
    return best;
  }

  private onDown(e: PointerEvent) {
    if (e.button !== 0) return;
    // Hindrer at nettleseren markerer tekst og starter et eget drag (som avbryter vårt).
    e.preventDefault();
    window.getSelection()?.removeAllRanges();
    const q = this.local(e);
    const last = this.lastDown;
    this.lastDown = { time: e.timeStamp, x: e.clientX, y: e.clientY };
    if (last && e.timeStamp - last.time < 400 && Math.hypot(e.clientX - last.x, e.clientY - last.y) < 6 && !this.drawing) {
      this.lastDown = null;
      if (this.onDoubleClick(q)) return;
    }
    if (this.drawing) {
      const first = this.drawing[0];
      if (first && this.drawing.length >= 3 && Math.hypot(first[0] - q[0], first[1] - q[1]) < PICK_PX * this.unitsPerPx()) {
        const pts = this.drawing;
        this.drawing = null;
        this.cb.commit(polygonProfile(pts), 'Nytt omriss');
      } else {
        this.drawing.push(q);
        this.cb.status(`${this.drawing.length} punkter. Klikk på første punkt for å lukke omrisset.`);
      }
      this.render();
      return;
    }
    if (!this.profile) return;
    const grab = this.pick(q);
    if (grab) this.selected = grab.index;
    this.cb.selectionChanged();
    this.render();
    if (!grab || this.readOnly) {
      if (this.readOnly && grab) this.cb.status('Konturen er koblet til en master. Kryss av for «Rediger master» eller gjør den individuell.');
      return;
    }
    this.svg.setPointerCapture(e.pointerId);
    this.grab = { pointerId: e.pointerId, what: grab, moved: false, work: this.profile };
  }

  private onMove(e: PointerEvent) {
    const q = this.local(e);
    if (this.drawing) {
      this.cursor = q;
      this.render();
      return;
    }
    const g = this.grab;
    if (!g || e.pointerId !== g.pointerId || !this.profile) return;
    g.moved = true;
    g.work =
      g.what.kind === 'point'
        ? movePoint(this.profile, g.what.index, q)
        : setHandle(this.profile, g.what.index, g.what.side, q);
    this.render(g.work);
    this.cb.preview(g.work);
  }

  private onUp(e: PointerEvent) {
    const g = this.grab;
    if (!g || e.pointerId !== g.pointerId) return;
    this.grab = null;
    if (this.svg.hasPointerCapture(e.pointerId)) this.svg.releasePointerCapture(e.pointerId);
    if (!g.moved) return;
    this.profile = g.work;
    this.cb.commit(g.work, g.what.kind === 'point' ? 'Flyttet konturpunkt' : 'Flyttet konturhåndtak');
  }

  private cancelGrab() {
    if (!this.grab) return;
    this.grab = null;
    this.render();
  }

  /** Nytt punkt på omrisset der det dobbeltklikkes. Returnerer om et punkt ble lagt inn. */
  private onDoubleClick(q: Vec2) {
    if (!this.profile || this.readOnly || this.pick(q)) return false;
    const near = nearestOnOutline(this.profile, q);
    if (near.distance > 2 * PICK_PX * this.unitsPerPx()) return false;
    const { profile, index } = insertPoint(this.profile, near.index, near.t);
    this.selected = index;
    this.profile = profile;
    this.render();
    this.cb.commit(profile, 'Nytt konturpunkt');
    return true;
  }

  render(work?: Profile) {
    const p = work ?? this.profile;
    const el = (name: string, attrs: Record<string, string | number>) => {
      const node = document.createElementNS(SVG, name);
      for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
      return node;
    };
    // y opp i profilplanet, y ned i SVG.
    const pt = (v: Vec2) => `${v[0]},${-v[1]}`;
    const u = 0.012 * VIEW;
    const nodes: SVGElement[] = [
      el('line', { x1: -VIEW, y1: 0, x2: VIEW, y2: 0, class: 'axis' }),
      el('line', { x1: 0, y1: -VIEW, x2: 0, y2: VIEW, class: 'axis' }),
      el('circle', { cx: 0, cy: 0, r: 1, class: 'unit' }),
    ];
    if (p) {
      nodes.push(el('polygon', { points: polygon(p).map(pt).join(' '), class: this.readOnly ? 'outline linked' : 'outline' }));
      p.points.forEach((q, i) => {
        const active = i === this.selected;
        if (active) {
          for (const h of [q.handle_in, q.handle_out]) {
            if (Math.hypot(h[0], h[1]) < 1e-9) continue;
            const end: Vec2 = [q.position[0] + h[0], q.position[1] + h[1]];
            nodes.push(el('line', { x1: q.position[0], y1: -q.position[1], x2: end[0], y2: -end[1], class: 'handle-line' }));
            nodes.push(el('circle', { cx: end[0], cy: -end[1], r: 3 * u, class: 'handle' }));
          }
        }
        const cls = `point${active ? ' active' : ''}${i === 0 ? ' first' : ''}`;
        nodes.push(
          q.kind === 'corner'
            ? el('rect', { x: q.position[0] - 3.5 * u, y: -q.position[1] - 3.5 * u, width: 7 * u, height: 7 * u, class: cls })
            : el('circle', { cx: q.position[0], cy: -q.position[1], r: 4 * u, class: cls }),
        );
      });
    }
    if (this.drawing) {
      const pts = this.cursor ? [...this.drawing, this.cursor] : this.drawing;
      if (pts.length > 1) nodes.push(el('polyline', { points: pts.map(pt).join(' '), class: 'drawing' }));
      this.drawing.forEach((q, i) => nodes.push(el('rect', { x: q[0] - 3.5 * u, y: -q[1] - 3.5 * u, width: 7 * u, height: 7 * u, class: i === 0 ? 'point first' : 'point' })));
    }
    if (!p && !this.drawing) {
      const t = el('text', { x: 0, y: 0.05, class: 'empty' });
      t.textContent = 'Ingen kontur valgt';
      nodes.push(t);
    }
    this.svg.replaceChildren(...nodes);
    this.svg.dataset.points = String(p?.points.length ?? 0);
  }
}
