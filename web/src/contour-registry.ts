import { polygon, PRESETS, station, VOLUMES } from './profile-math';
import type { Profile, RegistryEntry, SkeletonEdit, SplineView } from './protocol';
import { ProfilePanel } from './profile-panel';
import type { SkeletonView } from './skeleton-view';

export interface ContourRegistryCallbacks {
  commit: (edit: SkeletonEdit, describe: string) => void;
  status: (text: string) => void;
}

const SVG = 'http://www.w3.org/2000/svg';
const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!;

/** En flis i registeret: ferdig preset eller lagret oppføring. */
interface Tile {
  key: string;
  name: string;
  /** Profilen som vises på flisen (for et volum: den største konturen). */
  thumb: Profile;
  volume: boolean;
  entry?: number;
  apply: (s: SplineView) => void;
}

/**
 * Konturregisteret: ferdige presets (sirkel, ellipse, egg, trekant, kvadrat,
 * H), volumpresets (kule, hode) og profiler eller volumer brukeren har lagret
 * i prosjektet. Et preset kopieres inn når det brukes og kobles ikke varig;
 * en varig kobling lages med «Gjør til master» og «Koble til master».
 */
export class ContourRegistry {
  entries: RegistryEntry[] = [];
  private grid = $('#registry');
  private help = $('#registry-help');
  private name = $<HTMLInputElement>('#registry-name');
  private saveProfile = $<HTMLButtonElement>('#save-profile');
  private saveVolume = $<HTMLButtonElement>('#save-volume');

  constructor(
    private view: SkeletonView,
    private profiles: ProfilePanel,
    private cb: ContourRegistryCallbacks,
  ) {
    this.grid.addEventListener('click', (e) => this.onClick(e));
    this.saveProfile.addEventListener('click', () => {
      const c = this.profiles.current();
      if (!c) return;
      const name = this.name.value.trim() || `Kontur ${this.entries.length + 1}`;
      this.name.value = '';
      cb.commit({ kind: 'saveProfile', spline: c.spline.id, station: c.index, name }, `Lagret «${name}» i registeret`);
    });
    this.saveVolume.addEventListener('click', () => {
      const s = this.spline();
      if (!s || s.profiles.length < 2) return;
      const name = this.name.value.trim() || `Volum ${this.entries.length + 1}`;
      this.name.value = '';
      cb.commit({ kind: 'saveVolume', spline: s.id, name }, `Lagret volumet «${name}» i registeret`);
    });
    this.name.addEventListener('keydown', (e) => e.stopPropagation());
  }

  private spline(): SplineView | null {
    const sel = this.view.selection;
    return sel && !sel.layer ? this.view.spline(sel.spline) : null;
  }

  private tiles(): Tile[] {
    const out: Tile[] = [];
    for (const [name, make] of Object.entries(PRESETS)) {
      out.push({ key: `p:${name}`, name, thumb: make(), volume: false, apply: (s) => this.applyProfile(s, make(), name) });
    }
    for (const [name, make] of Object.entries(VOLUMES)) {
      const sample = make(40, 10);
      const big = sample.reduce((a, b) => (b.scale > a.scale ? b : a));
      out.push({
        key: `v:${name}`,
        name,
        thumb: big.shape.kind === 'own' ? big.shape.profile : PRESETS.Sirkel(),
        volume: true,
        apply: (s) => this.applyStations(s, make(s.length, s.thickness), name),
      });
    }
    for (const e of this.entries) {
      const big = e.stations.reduce((a, b) => (b.scale > a.scale ? b : a));
      const thumb = big.shape.kind === 'own' ? big.shape.profile : PRESETS.Sirkel();
      out.push({
        key: `e:${e.id}`,
        name: e.name,
        thumb,
        volume: e.volume,
        entry: e.id,
        apply: (s) =>
          e.volume ? this.applyStations(s, e.stations, e.name) : this.applyProfile(s, thumb, e.name),
      });
    }
    return out;
  }

  /** Kopierer profilen inn i valgt kontur, eller lager en ny kontur med den. */
  private applyProfile(s: SplineView, profile: Profile, name: string) {
    const c = this.profiles.current();
    if (c) {
      // En koblet kontur blir individuell med kopien; masteren endres ikke.
      this.cb.commit({ kind: 'setProfile', spline: c.spline.id, station: c.index, profile }, `Kontur: ${name} (kopi, ingen kobling)`);
      return;
    }
    const at = this.profiles.anchorAt(s);
    const index = ProfilePanel.indexFor(s, at);
    this.view.select({ spline: s.id, index: null, station: index });
    this.cb.commit(
      { kind: 'batch', edits: [{ kind: 'addProfile', spline: s.id, at, from: null }, { kind: 'setProfile', spline: s.id, station: index, profile }] },
      `Ny kontur: ${name}`,
    );
  }

  /** Erstatter konturene på splinen med volumet (kopier, ingen kobling). */
  private applyStations(s: SplineView, stations: RegistryEntry['stations'], name: string) {
    this.view.select({ spline: s.id, index: null, station: null });
    const copies = stations.map((st) => station(st.at, st.shape.kind === 'own' ? st.shape.profile : PRESETS.Sirkel(), st.scale, st.rotation));
    this.cb.commit({ kind: 'setStations', spline: s.id, stations: copies }, `Volum: ${name} · ${copies.length} konturer`);
  }

  private onClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    const tile = target.closest<HTMLElement>('[data-key]');
    if (!tile) return;
    const t = this.tiles().find((x) => x.key === tile.dataset.key);
    if (!t) return;
    if (target.closest('[data-remove]') && t.entry !== undefined) {
      this.cb.commit({ kind: 'removeEntry', entry: t.entry }, `Slettet «${t.name}» fra registeret`);
      return;
    }
    const s = this.spline();
    if (!s || s.locked) {
      this.cb.status('Velg en spline eller en kontur først.');
      return;
    }
    t.apply(s);
  }

  update() {
    const s = this.spline();
    const c = this.profiles.current();
    const usable = !!s && !s.locked;
    this.saveProfile.disabled = !c || !!s?.locked;
    this.saveVolume.disabled = !s || s.profiles.length < 2;
    this.help.textContent = !usable
      ? 'Velg en spline eller kontur for å bruke registeret.'
      : c
        ? 'En profil erstatter valgt kontur som kopi. Et volum erstatter alle konturene på splinen.'
        : 'En profil legges til som ny kontur. Et volum erstatter alle konturene på splinen.';
    const nodes = this.tiles().map((t) => {
      const b = document.createElement('div');
      b.className = `tile${t.volume ? ' volume' : ''}${usable ? '' : ' disabled'}`;
      b.dataset.key = t.key;
      b.setAttribute('role', 'listitem');
      const button = document.createElement('button');
      button.type = 'button';
      button.title = t.volume ? `${t.name}: volum med flere konturer` : `${t.name}: profil`;
      button.disabled = !usable;
      button.append(thumbnail(t.thumb), Object.assign(document.createElement('span'), { textContent: t.name }));
      b.append(button);
      if (t.entry !== undefined) {
        const x = document.createElement('button');
        x.type = 'button';
        x.className = 'tile-remove';
        x.dataset.remove = '';
        x.title = `Slett «${t.name}» fra registeret`;
        x.setAttribute('aria-label', x.title);
        x.textContent = '×';
        b.append(x);
      }
      return b;
    });
    this.grid.replaceChildren(...nodes);
  }
}

function thumbnail(p: Profile) {
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('viewBox', '-1.3 -1.3 2.6 2.6');
  svg.setAttribute('aria-hidden', 'true');
  const poly = document.createElementNS(SVG, 'polygon');
  poly.setAttribute('points', polygon(p).map((q) => `${q[0]},${-q[1]}`).join(' '));
  svg.append(poly);
  return svg;
}
