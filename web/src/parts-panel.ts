// Mesh-deler i inspektøren: delregisteret (innebygde former og importerte
// mesher) og delene som er satt inn på valgt spline.
import { readPartFile } from './part-import';
import type { Library, Part, PartSource, SkeletonEdit, SplineView, Vec3 } from './protocol';
import type { SkeletonView } from './skeleton-view';

export interface PartsPanelCallbacks {
  commit: (edit: SkeletonEdit, describe: string) => void;
  status: (text: string) => void;
}

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!;
const key = (s: PartSource) => (s.kind === 'builtin' ? `b:${s.name}` : `s:${s.id}`);

export class PartsPanel {
  library: Library = { parts: [], mirror: false, settings: { crease_detail: 2, crease_angle: 25 } };
  private source = $<HTMLSelectElement>('#part-source');
  private addButton = $<HTMLButtonElement>('#add-part');
  private removeMesh = $<HTMLButtonElement>('#remove-part-mesh');
  private list = $('#part-list');
  private file = $<HTMLInputElement>('#part-file');
  /** En importert del velges i registeret når den kommer tilbake fra kjernen. */
  private selectNewest = false;

  constructor(
    private view: SkeletonView,
    private cb: PartsPanelCallbacks,
  ) {
    this.addButton.addEventListener('click', () => {
      const s = this.selected();
      if (!s) return;
      // I valgt punkt står delen ut fra overflaten; ellers fortsetter den tuppen.
      const sel = this.view.selection;
      const anchor = sel?.index != null && sel.index < s.anchors.length - 1 ? s.anchors[sel.index] : null;
      this.insert(s, anchor ? anchor.at : 1, anchor !== null);
    });
    $<HTMLButtonElement>('#import-part').addEventListener('click', () => this.file.click());
    this.file.addEventListener('change', async () => {
      const f = this.file.files?.[0];
      this.file.value = '';
      if (f) await this.importFile(f);
    });
    this.removeMesh.addEventListener('click', () => {
      const src = this.current();
      if (src?.kind === 'stored') this.cb.commit({ kind: 'removePartMesh', id: src.id }, 'Fjernet delen fra registeret');
    });
    this.source.addEventListener('change', () => this.updateButtons());
  }

  /** Delen som er valgt i registeret. */
  current(): PartSource | null {
    return this.library.parts.find((p) => key(p.source) === this.source.value)?.source ?? null;
  }

  private selected() {
    const sel = this.view.selection;
    return sel && !sel.layer ? this.view.spline(sel.spline) : null;
  }

  /** Setter inn valgt del på splinen i andelen `at`, eventuelt ut fra overflaten mot `toward`. */
  insert(s: SplineView, at: number, outward: boolean, toward?: Vec3) {
    const source = this.current();
    if (!source) return;
    const part: Part = { source, at, size: Math.max(4, s.thickness * 2), rotation: 0, outward };
    const name = this.library.parts.find((p) => key(p.source) === key(source))?.name ?? 'del';
    this.cb.commit({ kind: 'addPart', spline: s.id, part, ...(toward ? { toward } : {}) }, `Satte inn ${name}`);
  }

  async importFile(file: File) {
    try {
      const mesh = await readPartFile(file);
      const name = file.name.replace(/\.[^.]+$/, '') || 'Importert del';
      this.selectNewest = true;
      this.cb.commit({ kind: 'importPart', name, positions: mesh.positions, triangles: mesh.triangles }, `Importerte ${name} til delregisteret`);
    } catch (err) {
      this.cb.status(`Kunne ikke importere: ${(err as Error).message}`);
    }
  }

  private updateButtons() {
    this.addButton.disabled = !this.selected() || !this.current();
    this.removeMesh.disabled = this.current()?.kind !== 'stored';
  }

  update() {
    const previous = this.source.value;
    this.source.replaceChildren(
      ...this.library.parts.map((p) => {
        const o = document.createElement('option');
        o.value = key(p.source);
        o.textContent = p.source.kind === 'stored' ? `${p.name} (importert)` : p.name;
        return o;
      }),
    );
    const count = this.source.options.length;
    if (this.selectNewest && count && this.source.options[count - 1].value.startsWith('s:') && this.source.options[count - 1].value !== previous) {
      this.selectNewest = false;
      this.source.value = this.source.options[count - 1].value;
    } else if ([...this.source.options].some((o) => o.value === previous)) this.source.value = previous;
    else if (this.source.options.length) this.source.value = this.source.options[this.source.options.length - 1].value;
    this.updateButtons();
    const s = this.selected();
    this.list.replaceChildren();
    if (!s) return;
    s.parts.forEach((part, index) => this.list.append(this.row(s, part, index)));
  }

  private row(s: SplineView, part: SplineView['parts'][number], index: number) {
    const row = document.createElement('div');
    row.className = 'part-row';
    row.setAttribute('role', 'listitem');
    const { name, base: _b, axis: _a, ...stored } = part;
    const set = (change: Partial<Part>, what: string) =>
      this.cb.commit({ kind: 'setPart', spline: s.id, index, part: { ...stored, ...change } }, what);
    const title = document.createElement('strong');
    title.textContent = name;
    const field = (label: string, value: number, unit: string, min: number, max: number, step: number, onChange: (v: number) => void) => {
      const l = document.createElement('label');
      l.textContent = label;
      const span = document.createElement('span');
      span.className = 'field';
      const input = document.createElement('input');
      Object.assign(input, { type: 'number', min: String(min), max: String(max), step: String(step), value: String(Math.round(value * 10) / 10) });
      input.addEventListener('change', () => {
        const v = Number(input.value.replace(',', '.'));
        if (Number.isFinite(v)) onChange(Math.min(Math.max(v, min), max));
      });
      span.append(input, ` ${unit}`);
      l.append(span);
      return l;
    };
    const outward = document.createElement('label');
    outward.className = 'check';
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.checked = part.outward;
    box.addEventListener('change', () => set({ outward: box.checked }, box.checked ? 'Delen står ut fra overflaten' : 'Delen følger kurven'));
    outward.append(box, ' Ut fra overflaten');
    const remove = document.createElement('button');
    remove.type = 'button';
    remove.textContent = 'Slett';
    remove.addEventListener('click', () => this.cb.commit({ kind: 'removePart', spline: s.id, index }, `Slettet ${name}`));
    row.append(
      title,
      field('Plass', part.at * 100, '%', 0, 100, 1, (v) => set({ at: v / 100 }, 'Flyttet delen')),
      field('Høyde', part.size, 'mm', 0.5, 500, 0.5, (v) => set({ size: v }, `Høyde ${v} mm`)),
      field('Rotasjon', (part.rotation * 180) / Math.PI, '°', -360, 360, 5, (v) => set({ rotation: (v * Math.PI) / 180 }, `Rotasjon ${v}°`)),
      outward,
      remove,
    );
    return row;
  }
}
