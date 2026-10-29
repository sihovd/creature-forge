import type { SplineView } from './protocol';
import type { Selection, SkeletonView } from './skeleton-view';

export interface LayerPanelCallbacks {
  select: (selection: Selection) => void;
  rename: (spline: number, name: string) => void;
  setLayer: (root: number, hidden: boolean, locked: boolean) => void;
}

/** Navnet som vises for en spline: brukerens navn, ellers et laget navn. */
export function splineName(s: SplineView) {
  if (s.name) return s.name;
  return s.parent === null ? `Lag ${s.id}` : `Gren ${s.id}`;
}

/** Navnet på splinen inne i laget: rotsplinen er lagets hovedspline. */
function rowName(s: SplineView) {
  return s.parent === null ? 'Hovedspline' : splineName(s);
}

const SVG_EYE = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/></svg>';
const SVG_EYE_OFF = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 3l18 18M10.6 5.6A10 10 0 0 1 12 5.5c6.4 0 10 6.5 10 6.5a17 17 0 0 1-3.2 3.9M6.4 6.9C3.7 8.6 2 12 2 12s3.6 6.5 10 6.5c1.6 0 3-.4 4.2-1"/></svg>';
const SVG_LOCK = '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/></svg>';
const SVG_UNLOCK = '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 7.5-2"/></svg>';

/**
 * Laglisten: ett lag per rotspline med hele hierarkiet av grener og
 * konturene til hver spline. Valg i listen og i 3D-visningen er det samme
 * valget. Laget kan navngis, skjules og låses.
 */
export class LayerPanel {
  private collapsed = new Set<string>();
  private editing: string | null = null;

  constructor(
    private root: HTMLElement,
    private view: SkeletonView,
    private cb: LayerPanelCallbacks,
  ) {
    root.addEventListener('click', (e) => this.onClick(e));
  }

  /** Forrige trykk på en rad, for dobbelttrykk. Listen tegnes på nytt ved
   * hvert valg, så nettleserens eget dblclick når ikke alltid frem. */
  private lastClick: { key: string; time: number } | null = null;

  private isDoubleClick(key: string, time: number) {
    const last = this.lastClick;
    this.lastClick = { key, time };
    if (last && last.key === key && time - last.time < 450) {
      this.lastClick = null;
      return true;
    }
    return false;
  }

  private onClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    const row = target.closest<HTMLElement>('[data-spline]');
    if (!row || target.closest('input')) return;
    const spline = Number(row.dataset.spline);
    const key = row.dataset.key!;
    const s = this.view.spline(spline);
    if (!s) return;
    const action = target.closest<HTMLElement>('[data-action]')?.dataset.action;
    if (action === 'toggle') {
      if (this.collapsed.has(key)) this.collapsed.delete(key);
      else this.collapsed.add(key);
      return this.render();
    }
    if (action === 'hide') return this.cb.setLayer(spline, !s.hidden, s.locked);
    if (action === 'lock') return this.cb.setLayer(spline, s.hidden, !s.locked);
    if (row.dataset.kind !== 'contour' && this.isDoubleClick(key, e.timeStamp)) {
      // Dobbeltklikk gir nytt navn. Hovedsplinen har lagets navn, så navnet endres på laget.
      return this.startRename(s.parent === null ? `l${s.id}` : key);
    }
    if (row.dataset.kind === 'layer') this.cb.select({ spline, index: null, station: null, layer: true });
    else if (row.dataset.kind === 'contour') this.cb.select({ spline, index: null, station: Number(row.dataset.station) });
    else this.cb.select({ spline, index: null, station: null });
  }

  /** Starter omdøping av raden med nøkkelen `key` i listen. */
  startRename(key: string) {
    this.editing = key;
    this.render();
    const input = this.root.querySelector<HTMLInputElement>('input.rename');
    input?.focus();
    input?.select();
  }

  private finishRename(spline: number, value: string | null) {
    this.editing = null;
    const s = this.view.spline(spline);
    if (s && value !== null && value.trim() !== s.name) this.cb.rename(spline, value);
    else this.render();
  }

  /** Bygger listen på nytt fra skjelettet og valget. */
  render() {
    const splines = this.view.splines;
    const sel = this.view.selection;
    // Åpne foreldrene til det som er valgt, så valget er synlig i listen.
    if (sel) {
      let s = this.view.spline(sel.spline);
      if (sel.station != null && s) this.collapsed.delete(`s${s.id}`);
      while (s) {
        this.collapsed.delete(`l${s.layer}`);
        if (s.parent === null) break;
        this.collapsed.delete(`s${s.parent}`);
        s = this.view.spline(s.parent);
      }
    }
    const list = document.createElement('ul');
    list.className = 'layer-tree';
    list.setAttribute('role', 'tree');
    const roots = splines.filter((s) => s.parent === null);
    for (const r of roots) {
      const key = `l${r.id}`;
      const open = !this.collapsed.has(key);
      const li = this.row({
        key,
        kind: 'layer',
        spline: r.id,
        depth: 0,
        name: splineName(r),
        selected: !!sel?.layer && sel.spline === r.id,
        expandable: true,
        open,
        icon: 'layer',
        dim: r.hidden,
      });
      const tools = document.createElement('span');
      tools.className = 'row-tools';
      tools.append(
        this.toolButton('hide', r.hidden ? 'Vis laget' : 'Skjul laget', r.hidden ? SVG_EYE_OFF : SVG_EYE, r.hidden),
        this.toolButton('lock', r.locked ? 'Lås opp laget' : 'Lås laget', r.locked ? SVG_LOCK : SVG_UNLOCK, r.locked),
      );
      li.querySelector('.row')!.append(tools);
      list.append(li);
      if (open) this.appendSpline(list, r, 1);
    }
    if (!roots.length) {
      const empty = document.createElement('li');
      empty.className = 'layer-empty';
      empty.textContent = 'Ingen lag ennå. Velg «Ny spline» for å begynne.';
      list.append(empty);
    }
    this.root.replaceChildren(list);
    const input = this.root.querySelector<HTMLInputElement>('input.rename');
    if (input) {
      const spline = Number(input.closest<HTMLElement>('[data-spline]')!.dataset.spline);
      let done = false;
      const finish = (v: string | null) => {
        if (done) return;
        done = true;
        this.finishRename(spline, v);
      };
      input.addEventListener('keydown', (e) => {
        e.stopPropagation();
        if (e.key === 'Enter') finish(input.value);
        if (e.key === 'Escape') finish(null);
      });
      input.addEventListener('blur', () => finish(input.value));
    }
  }

  private appendSpline(list: HTMLElement, s: SplineView, depth: number) {
    const sel = this.view.selection;
    const children = this.view.splines.filter((c) => c.parent === s.id);
    const key = `s${s.id}`;
    const expandable = children.length + s.profiles.length > 0;
    const open = !this.collapsed.has(key);
    list.append(
      this.row({
        key,
        kind: 'spline',
        spline: s.id,
        depth,
        name: rowName(s),
        selected: !!sel && !sel.layer && sel.spline === s.id && sel.station == null,
        expandable,
        open,
        icon: s.parent === null ? 'spline' : 'branch',
        dim: s.hidden,
      }),
    );
    if (!open) return;
    s.profiles.forEach((p, i) => {
      list.append(
        this.row({
          key: `c${s.id}:${i}`,
          kind: 'contour',
          spline: s.id,
          station: i,
          depth: depth + 1,
          name: `Kontur ${i + 1}${p.master !== null ? ` · M${p.master}` : ''}`,
          selected: !!sel && !sel.layer && sel.spline === s.id && sel.station === i,
          expandable: false,
          open: false,
          icon: 'contour',
          dim: s.hidden,
        }),
      );
    });
    for (const c of children) this.appendSpline(list, c, depth + 1);
  }

  private toolButton(action: string, label: string, svg: string, on: boolean) {
    const b = document.createElement('button');
    b.type = 'button';
    b.dataset.action = action;
    b.className = `row-tool${on ? ' on' : ''}`;
    b.title = label;
    b.setAttribute('aria-label', label);
    b.setAttribute('aria-pressed', String(on));
    b.innerHTML = svg;
    return b;
  }

  private row(o: {
    key: string;
    kind: 'layer' | 'spline' | 'contour';
    spline: number;
    station?: number;
    depth: number;
    name: string;
    selected: boolean;
    expandable: boolean;
    open: boolean;
    icon: string;
    dim: boolean;
  }) {
    const li = document.createElement('li');
    li.setAttribute('role', 'treeitem');
    li.dataset.key = o.key;
    li.dataset.kind = o.kind;
    li.dataset.spline = String(o.spline);
    if (o.station !== undefined) li.dataset.station = String(o.station);
    if (o.expandable) li.setAttribute('aria-expanded', String(o.open));
    li.setAttribute('aria-selected', String(o.selected));
    const row = document.createElement('div');
    row.className = `row${o.selected ? ' selected' : ''}${o.dim ? ' dim' : ''}`;
    row.style.setProperty('--depth', String(o.depth));
    const toggle = document.createElement('button');
    toggle.type = 'button';
    toggle.className = 'row-toggle';
    toggle.dataset.action = 'toggle';
    toggle.setAttribute('aria-label', o.open ? 'Fold sammen' : 'Fold ut');
    toggle.textContent = o.expandable ? (o.open ? '▾' : '▸') : '';
    toggle.disabled = !o.expandable;
    const icon = document.createElement('span');
    icon.className = `row-icon icon-${o.icon}`;
    icon.setAttribute('aria-hidden', 'true');
    row.append(toggle, icon);
    if (this.editing === o.key) {
      const input = document.createElement('input');
      input.className = 'rename';
      input.value = o.kind === 'layer' || o.kind === 'spline' ? (this.view.spline(o.spline)?.name ?? '') : o.name;
      input.placeholder = o.name;
      input.setAttribute('aria-label', 'Nytt navn');
      row.append(input);
    } else {
      const name = document.createElement('span');
      name.className = 'row-name';
      name.textContent = o.name;
      row.append(name);
    }
    li.append(row);
    return li;
  }
}
