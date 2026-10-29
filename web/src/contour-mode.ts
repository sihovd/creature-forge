import * as THREE from 'three';
import { ProfileEditor } from './profile-editor';
import { removePoint, setKind } from './profile-math';
import type { Profile, SkeletonEdit, SplineView, StationView } from './protocol';
import type { SkeletonView } from './skeleton-view';
import type { Viewport } from './viewport';

export interface ContourModeCallbacks {
  /** Viser en redigering uten å lagre den (under drag). */
  preview: (edit: SkeletonEdit) => void;
  commit: (edit: SkeletonEdit, describe: string) => void;
  status: (text: string) => void;
  /** Modusen er startet eller avsluttet. */
  changed: (active: boolean) => void;
}

/** Hvor stor del av bildehøyden konturen fyller når modusen starter. */
const FILL = 0.5;
const v3 = (p: [number, number, number]) => new THREE.Vector3(p[0], p[1], p[2]);

/**
 * Konturredigering direkte på modellen («Rediger kontur»). Kameraet ser
 * vinkelrett på konturplanet og zoomer inn, modellen skjæres i planet og
 * tegnes nedtonet som referanse, og omrisset redigeres i sitt eget 2D-plan
 * oppå modellen: punkter, håndtak, myke punkter og hjørnepunkter, innsetting
 * og tegning av et nytt omriss. Kameraet kan zoomes og panoreres, men ikke
 * roteres, så planet står vinkelrett hele tiden. «Ferdig» eller Esc går
 * tilbake til vanlig 3D-visning med kameraet der det var.
 */
export class ContourMode {
  readonly editor: ProfileEditor;
  private target: { spline: number; station: number } | null = null;
  private saved: ReturnType<Viewport['cameraState']> | null = null;
  private bar: HTMLDivElement;
  private title: HTMLElement;
  private info: HTMLElement;
  private insertButton: HTMLButtonElement;
  private kindButton: HTMLButtonElement;
  private removeButton: HTMLButtonElement;
  private drawButton: HTMLButtonElement;
  private unlinkButton: HTMLButtonElement;

  constructor(
    private viewport: Viewport,
    private view: SkeletonView,
    container: HTMLElement,
    private cb: ContourModeCallbacks,
  ) {
    const layer = document.createElement('div');
    layer.className = 'contour-overlay';
    layer.hidden = true;
    container.append(layer);
    this.editor = new ProfileEditor(
      layer,
      {
        preview: (profile) => this.current() && cb.preview(this.profileEdit(profile)),
        commit: (profile, what) => {
          if (this.current()) cb.commit(this.profileEdit(profile), what);
          this.updateButtons();
        },
        selectionChanged: () => this.updateButtons(),
        status: cb.status,
      },
      { className: 'profile-svg on-model', pointPx: 2.6 },
    );
    this.bar = document.createElement('div');
    this.bar.className = 'contour-bar';
    this.bar.hidden = true;
    this.bar.innerHTML = `
      <div class="contour-head"><strong class="contour-title"></strong><span class="contour-info"></span></div>
      <div class="contour-tools">
        <button type="button" data-act="insert" aria-pressed="false" title="Trykk på omrisset for å sette inn et punkt">Sett inn punkt</button>
        <button type="button" data-act="kind" title="Bytt mellom mykt punkt og hjørnepunkt">Hjørnepunkt</button>
        <button type="button" data-act="remove" title="Slett valgt punkt">Slett punkt</button>
        <button type="button" data-act="draw" title="Tegn et nytt lukket omriss">Tegn nytt</button>
        <button type="button" data-act="unlink" title="Bryt koblingen til masteren og behold formen">Gjør individuell</button>
        <button type="button" data-act="done" class="primary">Ferdig</button>
      </div>`;
    container.append(this.bar);
    this.title = this.bar.querySelector('.contour-title')!;
    this.info = this.bar.querySelector('.contour-info')!;
    const btn = (act: string) => this.bar.querySelector<HTMLButtonElement>(`[data-act="${act}"]`)!;
    this.insertButton = btn('insert');
    this.kindButton = btn('kind');
    this.removeButton = btn('remove');
    this.drawButton = btn('draw');
    this.unlinkButton = btn('unlink');
    this.insertButton.addEventListener('click', () => {
      this.editor.insertOnTap = !this.editor.insertOnTap;
      this.insertButton.setAttribute('aria-pressed', String(this.editor.insertOnTap));
      cb.status(this.editor.insertOnTap ? 'Trykk på omrisset for å sette inn et punkt.' : 'Dra punkter og håndtak. Dobbeltklikk på omrisset setter inn et punkt.');
    });
    this.kindButton.addEventListener('click', () => {
      const c = this.current();
      const i = this.editor.selected;
      if (!c || i === null) return;
      const next = c.station.profile.points[i].kind === 'corner' ? 'smooth' : 'corner';
      cb.commit(this.profileEdit(setKind(c.station.profile, i, next)), next === 'corner' ? 'Hjørnepunkt i konturen' : 'Mykt punkt i konturen');
    });
    this.removeButton.addEventListener('click', () => {
      const c = this.current();
      const i = this.editor.selected;
      if (!c || i === null || c.station.profile.points.length <= 3) return;
      this.editor.selected = null;
      cb.commit(this.profileEdit(removePoint(c.station.profile, i)), 'Slettet konturpunkt');
    });
    this.drawButton.addEventListener('click', () => {
      if (this.editor.isDrawing) this.editor.stopDrawing();
      else this.editor.startDrawing();
      this.updateButtons();
    });
    this.unlinkButton.addEventListener('click', () => {
      const c = this.current();
      if (c) cb.commit({ kind: 'unlink', spline: c.spline.id, station: c.index }, 'Konturen er individuell');
    });
    btn('done').addEventListener('click', () => this.exit());
    window.addEventListener('keydown', (e) => {
      if (e.key !== 'Escape' || !this.target) return;
      if (this.editor.isDrawing) {
        this.editor.stopDrawing();
        this.updateButtons();
      } else this.exit();
    });
    viewport.onFrame(() => this.place());
  }

  get active() {
    return this.target !== null;
  }

  /** Konturen som redigeres. */
  current(): { spline: SplineView; index: number; station: StationView } | null {
    const t = this.target;
    const s = t && this.view.spline(t.spline);
    const st = s?.profiles[t!.station];
    return s && st ? { spline: s, index: t!.station, station: st } : null;
  }

  /** Starter konturredigering for kontur `station` på splinen `spline`. */
  enter(spline: number, station: number) {
    const s = this.view.spline(spline);
    const st = s?.profiles[station];
    if (!s || !st || s.locked || s.hidden) return;
    if (!this.target) this.saved = this.viewport.cameraState();
    this.target = { spline, station };
    this.editor.selected = null;
    const origin = v3(st.origin);
    const normal = v3(st.normal).normalize();
    // Omrisset er i enheter av tykkelsen; største utstrekning i mm.
    const k = st.scale * s.thickness;
    const extent = Math.max(...st.profile.points.map((p) => Math.hypot(p.position[0], p.position[1]))) * k;
    const distance = this.viewport.distanceForHeight((2 * Math.max(extent, 1)) / FILL);
    this.viewport.setCamera({ position: origin.clone().addScaledVector(normal, distance), target: origin, up: v3(st.y_axis).normalize() });
    this.viewport.controls.enableRotate = false;
    this.viewport.interactionLocked = true;
    // Delen mellom konturen og kameraet skjæres bort.
    this.viewport.setSection(new THREE.Plane().setFromNormalAndCoplanarPoint(normal.clone().negate(), origin));
    this.view.setVisible(false);
    this.editor.svg.parentElement!.hidden = false;
    this.bar.hidden = false;
    this.cb.changed(true);
    this.update();
    this.cb.status('Rediger konturen på modellen: dra punkter og håndtak, zoom og panorer fritt. «Ferdig» eller Esc avslutter.');
  }

  exit() {
    if (!this.target) return;
    this.target = null;
    if (this.editor.isDrawing) this.editor.stopDrawing();
    this.editor.insertOnTap = false;
    this.insertButton.setAttribute('aria-pressed', 'false');
    this.viewport.setSection(null);
    this.viewport.controls.enableRotate = true;
    this.viewport.interactionLocked = false;
    if (this.saved) this.viewport.setCamera(this.saved);
    this.saved = null;
    this.view.setVisible(true);
    this.editor.svg.parentElement!.hidden = true;
    this.bar.hidden = true;
    this.cb.changed(false);
    this.cb.status('Konturredigeringen er avsluttet.');
  }

  /** Oppdaterer fra dokumentet etter en endring. Avslutter når konturen er borte. */
  update() {
    if (!this.target) return;
    const c = this.current();
    if (!c || c.spline.locked || c.spline.hidden) return this.exit();
    const name = c.spline.parent === null ? 'Hovedspline' : c.spline.name || `Gren ${c.spline.id}`;
    this.title.textContent = `Rediger kontur ${c.index + 1}`;
    const master = c.station.master;
    const linked = master === null ? 0 : this.view.splines.reduce((n, s) => n + s.profiles.filter((p) => p.master === master).length, 0);
    this.info.textContent = master === null ? ` · ${name}` : ` · ${name} · endrer masteren M${master} og ${linked} ${linked === 1 ? 'kontur' : 'konturer'}`;
    this.bar.classList.toggle('linked', master !== null && linked > 1);
    this.editor.set(c.station.profile, false);
    this.updateButtons();
    this.place();
  }

  /** En koblet kontur redigeres gjennom masteren, så alle forekomster følger. */
  private profileEdit(profile: Profile): SkeletonEdit {
    const c = this.current()!;
    if (c.station.master !== null) return { kind: 'setMaster', master: c.station.master, profile };
    return { kind: 'setProfile', spline: c.spline.id, station: c.index, profile };
  }

  private updateButtons() {
    const c = this.current();
    const i = this.editor.selected;
    const point = c && i !== null ? c.station.profile.points[i] : null;
    this.kindButton.disabled = !point;
    this.kindButton.textContent = point?.kind === 'corner' ? 'Mykt punkt' : 'Hjørnepunkt';
    this.removeButton.disabled = !point || c!.station.profile.points.length <= 3;
    this.drawButton.textContent = this.editor.isDrawing ? 'Avbryt tegning' : 'Tegn nytt';
    this.unlinkButton.hidden = c?.station.master == null;
    this.editor.svg.classList.toggle('drawing-mode', this.editor.isDrawing);
  }

  /**
   * Legger editorens utsnitt nøyaktig oppå konturen i bildet. Kameraet ser
   * vinkelrett på planet, så profilplanet avbildes med en ren skalering.
   */
  private place() {
    const c = this.current();
    if (!c) return;
    const st = c.station;
    const k = st.scale * c.spline.thickness;
    const o = this.viewport.toScreen(v3(st.origin));
    const ox = this.viewport.toScreen(v3(st.origin).addScaledVector(v3(st.x_axis), k));
    const pxPerUnit = ox.distanceTo(o);
    if (pxPerUnit < 1e-6) return;
    // Mål mot editorens egen flate, så den ligger riktig selv om den ikke
    // har nøyaktig samme størrelse og plass som lerretet.
    const canvas = this.viewport.renderer.domElement.getBoundingClientRect();
    const svg = this.editor.svg.getBoundingClientRect();
    if (svg.width < 1 || svg.height < 1) return;
    const ox0 = o.x + canvas.left - svg.left;
    const oy0 = o.y + canvas.top - svg.top;
    const w = svg.width / pxPerUnit;
    const h = svg.height / pxPerUnit;
    this.editor.setView(-ox0 / pxPerUnit, oy0 / pxPerUnit - h, w, h);
  }
}
