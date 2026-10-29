import { ProfileEditor } from './profile-editor';
import { PRESETS, removePoint, setKind } from './profile-math';
import type { Profile, SkeletonEdit, SplineView, StationView } from './protocol';
import type { SkeletonView } from './skeleton-view';

export interface ProfilePanelCallbacks {
  /** Viser en redigering uten å lagre den. */
  preview: (edit: SkeletonEdit) => void;
  commit: (edit: SkeletonEdit, describe: string) => void;
  status: (text: string) => void;
}

/** En kontur slik utklippstavlen husker den. */
interface Clip {
  profile: Profile;
  master: number | null;
  scale: number;
  rotation: number;
}

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!;
const pct = (x: number) => `${Math.round(x * 100)} %`;
const RAD = Math.PI / 180;

/**
 * Konturpanelet: velg, legg til, kopier og lim inn tverrsnittsprofiler
 * langs valgt spline, rediger omrisset, og koble profiler til en master
 * (issue #4, del 3 og 4).
 */
export class ProfilePanel {
  readonly editor: ProfileEditor;
  clip: Clip | null = null;
  private select = $<HTMLSelectElement>('#station-select');
  private add = $<HTMLButtonElement>('#add-profile');
  private preset = $<HTMLSelectElement>('#profile-preset');
  private draw = $<HTMLButtonElement>('#draw-profile');
  private pointKind = $<HTMLButtonElement>('#point-kind');
  private removePointButton = $<HTMLButtonElement>('#remove-point');
  private at = $<HTMLInputElement>('#station-at');
  private scale = $<HTMLInputElement>('#station-scale');
  private rotation = $<HTMLInputElement>('#station-rotation');
  private copy = $<HTMLButtonElement>('#copy-profile');
  private paste = $<HTMLButtonElement>('#paste-profile');
  private remove = $<HTMLButtonElement>('#remove-profile');
  private info = $<HTMLParagraphElement>('#master-info');
  private makeMaster = $<HTMLButtonElement>('#make-master');
  private unlink = $<HTMLButtonElement>('#unlink');
  private link = $<HTMLSelectElement>('#link-master');
  private editMaster = $<HTMLInputElement>('#edit-master');
  private flatEnd = $<HTMLInputElement>('#flat-end');

  constructor(private view: SkeletonView, private cb: ProfilePanelCallbacks) {
    this.editor = new ProfileEditor($('#profile-editor'), {
      preview: (profile) => this.current() && cb.preview(this.profileEdit(profile)),
      commit: (profile, what) => {
        if (!this.current()) return;
        cb.commit(this.profileEdit(profile), what);
      },
      selectionChanged: () => this.updatePointButtons(),
      status: cb.status,
    });
    for (const name of Object.keys(PRESETS)) this.preset.append(new Option(name, name));

    this.select.addEventListener('change', () => {
      const sel = view.selection;
      if (!sel) return;
      view.select({ ...sel, station: this.select.value === '' ? null : Number(this.select.value) });
      this.update();
    });
    this.add.addEventListener('click', () => {
      const s = this.spline();
      if (s) this.addAt(s, this.anchorAt(s));
    });
    this.preset.addEventListener('change', () => {
      const name = this.preset.value;
      this.preset.value = '';
      if (PRESETS[name] && this.current()) cb.commit(this.profileEdit(PRESETS[name]()), `Kontur: ${name}`);
    });
    this.draw.addEventListener('click', () => {
      if (this.editor.isDrawing) this.editor.stopDrawing();
      else this.editor.startDrawing();
      this.draw.textContent = this.editor.isDrawing ? 'Avbryt' : 'Tegn nytt';
    });
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && this.editor.isDrawing) {
        this.editor.stopDrawing();
        this.draw.textContent = 'Tegn nytt';
      }
    });
    this.pointKind.addEventListener('click', () => {
      const c = this.current();
      const i = this.editor.selected;
      if (!c || i === null) return;
      const next = c.station.profile.points[i].kind === 'corner' ? 'smooth' : 'corner';
      cb.commit(this.profileEdit(setKind(c.station.profile, i, next)), next === 'corner' ? 'Hjørnepunkt i konturen' : 'Mykt punkt i konturen');
    });
    this.removePointButton.addEventListener('click', () => {
      const c = this.current();
      const i = this.editor.selected;
      if (!c || i === null || c.station.profile.points.length <= 3) return;
      this.editor.selected = null;
      cb.commit(this.profileEdit(removePoint(c.station.profile, i)), 'Slettet konturpunkt');
    });
    const stationInputs = [this.at, this.scale, this.rotation];
    for (const input of stationInputs) {
      input.addEventListener('change', () => {
        const c = this.current();
        if (!c) return;
        const edit = this.stationEdit(c.spline, c.index);
        if (edit) cb.commit(edit, 'Endret konturen');
      });
    }
    this.copy.addEventListener('click', () => this.copyCurrent());
    this.paste.addEventListener('click', () => {
      const s = this.spline();
      if (s) this.pasteAt(s, this.anchorAt(s));
    });
    this.remove.addEventListener('click', () => {
      const c = this.current();
      if (!c) return;
      view.select({ ...view.selection!, station: null });
      cb.commit({ kind: 'removeProfile', spline: c.spline.id, station: c.index }, 'Slettet kontur');
    });
    this.makeMaster.addEventListener('click', () => {
      const c = this.current();
      if (c) cb.commit({ kind: 'makeMaster', spline: c.spline.id, station: c.index }, 'Ny master');
    });
    this.unlink.addEventListener('click', () => {
      const c = this.current();
      if (c) cb.commit({ kind: 'unlink', spline: c.spline.id, station: c.index }, 'Konturen er individuell');
    });
    this.link.addEventListener('change', () => {
      const c = this.current();
      const master = Number(this.link.value);
      this.link.value = '';
      if (c && master) cb.commit({ kind: 'linkMaster', spline: c.spline.id, station: c.index, master }, `Koblet til master M${master}`);
    });
    this.editMaster.addEventListener('change', () => this.update());
    this.flatEnd.addEventListener('change', () => {
      const s = this.spline();
      if (s) cb.commit({ kind: 'setFlatEnd', spline: s.id, flat: this.flatEnd.checked }, this.flatEnd.checked ? 'Flat ende' : 'Avrundet ende');
    });
    this.update();
  }

  private spline(): SplineView | null {
    const sel = this.view.selection;
    return sel ? this.view.spline(sel.spline) : null;
  }

  /** Valgt kontur. */
  current(): { spline: SplineView; index: number; station: StationView } | null {
    const s = this.spline();
    const i = this.view.selection?.station;
    if (!s || i == null || !s.profiles[i]) return null;
    return { spline: s, index: i, station: s.profiles[i] };
  }

  /** Plassen til valgt ankerpunkt, ellers midt på splinen. */
  private anchorAt(s: SplineView) {
    const i = this.view.selection?.index;
    return i != null && s.anchors[i] ? s.anchors[i].at : 0.5;
  }

  /** Plassen en ny kontur i `at` får etter sorteringen i kjernen. */
  private static indexFor(s: SplineView, at: number) {
    return s.profiles.filter((p) => p.at <= at).length;
  }

  private linkedEditable(c: { station: StationView }) {
    return c.station.master !== null && this.editMaster.checked;
  }

  /** Redigeringen som lagrer et nytt omriss for valgt kontur eller dens master. */
  private profileEdit(profile: Profile): SkeletonEdit {
    const c = this.current()!;
    if (this.linkedEditable(c)) return { kind: 'setMaster', master: c.station.master!, profile };
    return { kind: 'setProfile', spline: c.spline.id, station: c.index, profile };
  }

  private stationEdit(s: SplineView, index: number): SkeletonEdit | null {
    const at = Number(this.at.value) / 100;
    const scale = Number(this.scale.value) / 100;
    const rotation = Number(this.rotation.value) * RAD;
    if (![at, scale, rotation].every(Number.isFinite) || scale <= 0) return null;
    return { kind: 'setStation', spline: s.id, station: index, at: Math.min(Math.max(at, 0), 1), scale, rotation };
  }

  addAt(s: SplineView, at: number) {
    this.view.select({ spline: s.id, index: this.view.selection?.spline === s.id ? this.view.selection.index : null, station: ProfilePanel.indexFor(s, at) });
    this.cb.commit({ kind: 'addProfile', spline: s.id, at, from: null }, `Ny kontur ved ${pct(at)}`);
  }

  copyCurrent() {
    const c = this.current();
    if (!c) return;
    const { profile, master, scale, rotation } = c.station;
    this.clip = { profile, master, scale, rotation };
    this.cb.status('Konturen er kopiert. Lim inn på valgt punkt, eller høyreklikk på en kurve.');
    this.update();
  }

  /** Limer inn kopien, med masterkobling, størrelse og rotasjon, i `at`. */
  pasteAt(s: SplineView, at: number) {
    const clip = this.clip;
    if (!clip) return;
    const station = ProfilePanel.indexFor(s, at);
    const shape: SkeletonEdit =
      clip.master !== null
        ? { kind: 'linkMaster', spline: s.id, station, master: clip.master }
        : { kind: 'setProfile', spline: s.id, station, profile: clip.profile };
    this.view.select({ spline: s.id, index: this.view.selection?.spline === s.id ? this.view.selection.index : null, station });
    this.cb.commit(
      {
        kind: 'batch',
        edits: [
          { kind: 'addProfile', spline: s.id, at, from: null },
          shape,
          { kind: 'setStation', spline: s.id, station, at, scale: clip.scale, rotation: clip.rotation },
        ],
      },
      `Limte inn kontur ved ${pct(at)}`,
    );
  }

  private updatePointButtons() {
    const c = this.current();
    const i = this.editor.selected;
    const editable = !!c && (c.station.master === null || this.editMaster.checked);
    const point = c && i !== null ? c.station.profile.points[i] : null;
    this.pointKind.disabled = !point || !editable;
    this.pointKind.textContent = point?.kind === 'corner' ? 'Mykt punkt' : 'Hjørnepunkt';
    this.removePointButton.disabled = !point || !editable || c!.station.profile.points.length <= 3;
  }

  /** Oppdaterer panelet fra valgt spline og kontur. */
  update() {
    const s = this.spline();
    const c = this.current();
    this.select.replaceChildren(new Option(s?.profiles.length ? 'Velg kontur …' : 'Ingen konturer (sirkel)', ''));
    s?.profiles.forEach((p, i) => {
      const label = `Kontur ${i + 1} · ${pct(p.at)}${p.master !== null ? ` · M${p.master}` : ''}`;
      this.select.append(new Option(label, String(i)));
    });
    this.select.value = c ? String(c.index) : '';
    this.select.disabled = !s || s.profiles.length === 0;
    this.add.disabled = !s;
    this.flatEnd.disabled = !s;
    this.flatEnd.checked = !!s?.flat_end;
    for (const el of [this.preset, this.draw, this.at, this.scale, this.rotation, this.copy, this.remove, this.makeMaster]) {
      el.disabled = !c;
    }
    this.paste.disabled = !s || !this.clip;
    const linked = c?.station.master ?? null;
    this.unlink.disabled = linked === null;
    this.editMaster.disabled = linked === null;
    if (linked === null) this.editMaster.checked = false;
    this.makeMaster.disabled = !c || linked !== null;
    // Masterne som finnes, hentet fra konturene som bruker dem.
    const masters = new Map<number, number>();
    for (const sp of this.view.splines) for (const p of sp.profiles) if (p.master !== null) masters.set(p.master, (masters.get(p.master) ?? 0) + 1);
    this.link.replaceChildren(new Option(masters.size ? 'Koble til master …' : 'Ingen mastere', ''));
    for (const [m, n] of [...masters].sort((a, b) => a[0] - b[0])) {
      if (m !== linked) this.link.append(new Option(`M${m} (${n} ${n === 1 ? 'kontur' : 'konturer'})`, String(m)));
    }
    this.link.disabled = !c || this.link.options.length < 2;
    if (c) {
      this.at.value = String(Math.round(c.station.at * 100));
      this.scale.value = String(Math.round(c.station.scale * 100));
      this.rotation.value = String(Math.round(c.station.rotation / RAD));
      this.info.textContent =
        linked === null
          ? 'Konturen er individuell.'
          : `Koblet til master M${linked} (${masters.get(linked)} ${masters.get(linked) === 1 ? 'kontur' : 'konturer'}). ${this.editMaster.checked ? 'Endringer gjelder alle koblede.' : 'Kryss av for å redigere masteren.'}`;
    } else {
      this.info.textContent = s ? 'Velg eller legg til en kontur for å forme tverrsnittet.' : 'Velg en spline for å se konturene.';
    }
    this.editor.set(c ? c.station.profile : null, !!c && linked !== null && !this.editMaster.checked);
    if (!c && this.editor.isDrawing) this.editor.stopDrawing();
    this.draw.textContent = this.editor.isDrawing ? 'Avbryt' : 'Tegn nytt';
    this.updatePointButtons();
  }
}
