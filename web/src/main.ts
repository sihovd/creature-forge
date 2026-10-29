import { AxisGizmo } from './axis-gizmo';
import { ContextMenu, type MenuItem } from './context-menu';
import { ContourMode } from './contour-mode';
import { ContourRegistry } from './contour-registry';
import { GeometryClient } from './geometry-client';
import * as THREE from 'three';
import type { Command, GeometryResult, PullParams, SkeletonEdit, SplineView, Vec3 } from './protocol';
import { LayerPanel, splineName } from './layer-panel';
import { ProfilePanel } from './profile-panel';
import { PullTool } from './pull-tool';
import { polylineFraction } from './skeleton-math';
import { SkeletonTool } from './skeleton-tool';
import { SkeletonView, type Selection } from './skeleton-view';
import { Viewport } from './viewport';

/** Lengden på en ny spline, i mm. */
const NEW_SPLINE_MM = 40;
/** Langt trykk med finger eller penn åpner hurtigmenyen. */
const LONG_PRESS_MS = 550;
const TAP_PX = 6;

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!;
const container = $('#viewport');
const status = $('#status');
const hint = $('#empty-hint');
const openButton = $<HTMLButtonElement>('#open-project');
const saveButton = $<HTMLButtonElement>('#save-project');
const fileInput = $<HTMLInputElement>('#project-file');
const toolSelect = $<HTMLButtonElement>('#tool-select');
const toolBezier = $<HTMLButtonElement>('#tool-bezier');
const toggleLayers = $<HTMLButtonElement>('#toggle-layers');
const inspectorTitle = $('#inspector-title');
const inspectorPath = $('#inspector-path');
const objectSection = $('#object-section');
const objectName = $<HTMLInputElement>('#object-name');
const layerHidden = $<HTMLInputElement>('#layer-hidden');
const layerLocked = $<HTMLInputElement>('#layer-locked');
const lockedInfo = $('#locked-info');
const editSections = $<HTMLFieldSetElement>('#edit-sections');
const branchSection = $('#branch-section');
const editContour = $<HTMLButtonElement>('#edit-contour');
const orientFollow = $<HTMLButtonElement>('#orient-follow');
const orientKeep = $<HTMLButtonElement>('#orient-keep');
const undoButton = $<HTMLButtonElement>('#undo');
const redoButton = $<HTMLButtonElement>('#redo');
const spacingInput = $<HTMLInputElement>('#spacing');
const radiusInput = $<HTMLInputElement>('#radius');
const addSpline = $<HTMLButtonElement>('#add-spline');
const addBranch = $<HTMLButtonElement>('#add-branch');
const anchorKind = $<HTMLButtonElement>('#anchor-kind');
const removeSpline = $<HTMLButtonElement>('#remove-spline');
const thicknessInput = $<HTMLInputElement>('#thickness');

// Arbeidsrommet starter alltid tomt; brukeren legger selv til første form.
const viewport = new Viewport(container);
const geometry = new GeometryClient();
const skeleton = new SkeletonView(viewport);
const gizmo = new AxisGizmo(viewport, container);
let activeTool: 'select' | 'bezier' = 'select';
const narrow = window.matchMedia('(max-width: 640px)');
const fmt = (n: number) => n.toLocaleString('nb-NO');

function readMm(input: HTMLInputElement, fallback: number) {
  const v = Number(input.value.replace(',', '.'));
  return Number.isFinite(v) && v > 0 ? Math.min(Math.max(v, Number(input.min)), Number(input.max)) : fallback;
}

function apply(result: GeometryResult) {
  if (result.kind === 'project') return;
  if (result.kind === 'preview') {
    viewport.setShapeMesh(result.shape, result.mesh);
    return;
  }
  if (result.kind === 'skeletonPreview') {
    skeleton.set(result.skeleton);
    for (const s of result.shapes) viewport.setShapeMesh(s.shape, s.mesh);
    return;
  }
  skeleton.set(result.skeleton);
  if (registry) registry.entries = result.registry;
  if (result.created !== null) skeleton.select({ spline: result.created, index: null });
  syncLayers();
  updateSkeletonPanel();
  container.dataset.profiles = String(skeleton.splines.reduce((n, s) => n + s.profiles.length, 0));
  container.dataset.splines = String(skeleton.count);
  let triangles = 0;
  for (const s of result.shapes) {
    triangles += viewport.setShapeMesh(s.shape, s.mesh);
    viewport.setControlPoints(s.shape, s.controlPoints);
  }
  undoButton.disabled = !result.state.canUndo;
  redoButton.disabled = !result.state.canRedo;
  container.dataset.shapeCount = String(viewport.shapeCount);
  container.dataset.controlPoints = String(viewport.controlPointCount);
  container.dataset.revision = String(result.state.revision);
  hint.hidden = viewport.shapeCount > 0 || skeleton.count > 0;
  return triangles;
}

/** Kjører en kommando som endrer dokumentet, og viser resultatet. */
async function run(command: Command, describe: (triangles: number, ms: number) => string) {
  try {
    const { result, computeMs } = await geometry.send(command);
    const triangles = apply(result) ?? 0;
    status.textContent = describe(triangles, computeMs);
  } catch (err) {
    status.textContent = `Feil i geometrikjernen: ${(err as Error).message}`;
  }
}

// Forhåndsvisning under drag: bare én forespørsel om gangen, og bare det siste grepet sendes.
let queuedPreview: { shape: number; pull: PullParams } | null = null;
let previewInFlight = false;
async function pumpPreview() {
  if (previewInFlight || !queuedPreview) return;
  const { shape, pull } = queuedPreview;
  queuedPreview = null;
  previewInFlight = true;
  try {
    const { result } = await geometry.send(
{ kind: 'previewPull', shape, pull },
    );
    if (tool.dragging) apply(result);
  } catch {
    // Et ugyldig mellomsteg overskriver ikke siste gyldige visning.
  } finally {
    previewInFlight = false;
    if (tool.dragging) pumpPreview();
  }
}


/** Skjulte og låste lag i visningen, og laglisten. */
function syncLayers() {
  const roots = skeleton.splines.filter((s) => s.parent === null);
  viewport.setShapeStates(new Set(roots.filter((r) => r.hidden).map((r) => r.id)), new Set(roots.filter((r) => r.locked).map((r) => r.id)));
  container.dataset.layers = String(roots.length);
  container.dataset.hiddenLayers = String(roots.filter((r) => r.hidden).length);
  container.dataset.lockedLayers = String(roots.filter((r) => r.locked).length);
}

function updateSkeletonPanel() {
  const sel = skeleton.selection;
  const spline = sel ? skeleton.spline(sel.spline) : null;
  const anchor = spline && sel!.index !== null ? spline.anchors[sel!.index] : null;
  addBranch.disabled = !spline;
  removeSpline.disabled = !spline;
  removeSpline.textContent = sel?.layer ? 'Slett laget' : 'Slett';
  anchorKind.disabled = !anchor;
  anchorKind.textContent = anchor?.kind === 'corner' ? 'Mykt punkt' : 'Hjørnepunkt';
  if (spline) thicknessInput.value = String(spline.thickness);
  branchSection.hidden = !spline || spline.parent === null || !!sel?.layer;
  orientFollow.setAttribute('aria-pressed', String(spline?.orientation !== 'keep_direction'));
  orientKeep.setAttribute('aria-pressed', String(spline?.orientation === 'keep_direction'));
  profiles?.update();
  contourMode?.update();
  registry?.update();
  editContour.disabled = !(sel?.station != null && spline && !spline.locked);
  updateInspector();
  layers?.render();
  container.dataset.selection = !sel ? '' : sel.layer ? `layer:${sel.spline}` : sel.station != null ? `contour:${sel.spline}:${sel.station}` : `spline:${sel.spline}`;
}

/** Overskriften i inspektøren: hva som er valgt, og hvor det ligger i laget. */
function updateInspector() {
  const sel = skeleton.selection;
  const s = sel ? skeleton.spline(sel.spline) : null;
  objectSection.hidden = !s;
  editSections.disabled = !!s?.locked;
  lockedInfo.hidden = !s?.locked;
  if (!s || !sel) {
    inspectorTitle.textContent = 'Ingenting valgt';
    inspectorPath.textContent = 'Velg et lag, en spline eller en kontur i listen eller i arbeidsflaten.';
    return;
  }
  const layer = skeleton.spline(s.layer)!;
  const chain: string[] = [];
  for (let p = s.parent !== null ? skeleton.spline(s.parent) : null; p; p = p.parent !== null ? skeleton.spline(p.parent) : null) {
    if (p.parent !== null) chain.unshift(splineName(p));
  }
  if (sel.layer) {
    inspectorTitle.textContent = splineName(layer);
    inspectorPath.textContent = 'Lag';
  } else if (sel.station != null) {
    inspectorTitle.textContent = `Kontur ${sel.station + 1}`;
    inspectorPath.textContent = [splineName(layer), ...chain, s.parent === null ? 'Hovedspline' : splineName(s)].join(' / ');
  } else {
    inspectorTitle.textContent = s.parent === null ? 'Hovedspline' : splineName(s);
    inspectorPath.textContent = [splineName(layer), ...chain].join(' / ');
  }
  // Navnet gjelder laget når hele laget eller hovedsplinen er valgt.
  const named = sel.layer || s.parent === null ? layer : s;
  if (document.activeElement !== objectName) objectName.value = named.name;
  objectName.placeholder = splineName(named);
  objectName.dataset.spline = String(named.id);
  layerHidden.checked = layer.hidden;
  layerLocked.checked = layer.locked;
}

function editSkeleton(edit: SkeletonEdit, what: string) {
  run({ kind: 'commitSkeleton', edit }, (_t, ms) => `${what} · ${ms.toFixed(0)} ms`);
}

// Trykk og langt trykk registreres før verktøyene, så de ser alle trykk.
installPressHandling();

// Registreres før formgrepet, så skjelettet får pekeren først.
const skeletonTool = new SkeletonTool(viewport, skeleton, {
  preview: async (edit) => {
    const { result } = await geometry.send({ kind: 'previewSkeleton', edit });
    return result.kind === 'skeletonPreview' ? result.skeleton : null;
  },
  commit: (edit, what) => editSkeleton(edit, what),
  restore: () => run({ kind: 'controlPoints', spacingMm: geometry.spacingMm }, () => status.textContent ?? ''),
  status: (text) => (status.textContent = text),
  selectionChanged: updateSkeletonPanel,
  tool: () => activeTool,
});

const arr = (v: THREE.Vector3): Vec3 => [v.x, v.y, v.z];

// Forhåndsvisning av konturen under drag i konturfeltet: én forespørsel om gangen.
let queuedSkeletonPreview: SkeletonEdit | null = null;
let skeletonPreviewInFlight = false;
async function previewSkeletonEdit(edit: SkeletonEdit) {
  queuedSkeletonPreview = edit;
  if (skeletonPreviewInFlight) return;
  while (queuedSkeletonPreview) {
    const next = queuedSkeletonPreview;
    queuedSkeletonPreview = null;
    skeletonPreviewInFlight = true;
    try {
      const { result } = await geometry.send({ kind: 'previewSkeleton', edit: next });
      apply(result);
    } catch {
      // Et ugyldig mellomsteg (for eksempel et omriss som krysser seg selv) vises ikke.
    } finally {
      skeletonPreviewInFlight = false;
    }
  }
}

const contourMode: ContourMode = new ContourMode(viewport, skeleton, container, {
  preview: (edit) => previewSkeletonEdit(edit),
  commit: (edit, what) => {
    queuedSkeletonPreview = null;
    editSkeleton(edit, what);
  },
  status: (text) => (status.textContent = text),
  changed: (active) => {
    container.dataset.contourMode = active ? 'on' : '';
    document.body.classList.toggle('contour-editing', active);
  },
});

/** «Rediger kontur»: konturredigering direkte på modellen. */
function editContourAt(spline: number, station: number) {
  setSelection({ spline, index: null, station });
  contourMode.enter(spline, station);
}

const profiles: ProfilePanel = new ProfilePanel(skeleton, {
  preview: (edit) => previewSkeletonEdit(edit),
  commit: (edit, what) => {
    queuedSkeletonPreview = null;
    editSkeleton(edit, what);
  },
  status: (text) => (status.textContent = text),
});

const layers: LayerPanel = new LayerPanel($('#layer-list'), skeleton, {
  select: (selection) => {
    setSelection(selection);
    // På små skjermer ligger listen over arbeidsflaten og lukkes etter valget.
    if (narrow.matches && toggleLayers.getAttribute('aria-expanded') === 'true') toggleLayers.click();
  },
  rename: (spline, name) => renameSpline(spline, name),
  setLayer: (root, hidden, locked) => setLayer(root, hidden, locked),
});

function setSelection(selection: Selection | null) {
  skeleton.select(selection);
  updateSkeletonPanel();
}

function setLayer(root: number, hidden: boolean, locked: boolean) {
  const r = skeleton.spline(root);
  if (!r) return;
  const what = hidden !== r.hidden ? (hidden ? 'Skjulte laget' : 'Viste laget') : locked ? 'Låste laget' : 'Låste opp laget';
  editSkeleton({ kind: 'setLayer', spline: root, hidden, locked }, `${what} ${splineName(r)}`);
}

objectName.addEventListener('change', () => {
  const id = Number(objectName.dataset.spline);
  if (skeleton.spline(id)) renameSpline(id, objectName.value);
});
objectName.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') objectName.blur();
});
function renameSpline(id: number, name: string) {
  editSkeleton({ kind: 'rename', spline: id, name }, name.trim() ? `Nytt navn: ${name.trim()}` : 'Navnet er fjernet');
}
for (const box of [layerHidden, layerLocked]) {
  box.addEventListener('change', () => {
    const s = skeleton.selection && skeleton.spline(skeleton.selection.spline);
    if (s) setLayer(s.layer, layerHidden.checked, layerLocked.checked);
  });
}

/**
 * Ny spline: aksekorset vises i `origin`, og brukeren velger
 * uttrekksretningen før splinen lages. Splinen velges med tuppen, så den
 * kan trekkes videre ut.
 */
function newSplineAt(origin: THREE.Vector3) {
  status.textContent = 'Velg uttrekksretning i aksekorset (X, Y eller Z). Esc avbryter.';
  gizmo.show(origin, (direction) =>
    editSkeleton(
      { kind: 'addRoot', start: arr(origin), direction, length: NEW_SPLINE_MM, thickness: readMm(thicknessInput, 5) },
      'Ny spline · dra tuppen for å trekke videre ut',
    ),
  );
}

const registry: ContourRegistry = new ContourRegistry(skeleton, profiles, {
  commit: (edit, what) => editSkeleton(edit, what),
  status: (text) => (status.textContent = text),
});

addSpline.addEventListener('click', () => {
  const rect = container.getBoundingClientRect();
  const ground = viewport.groundPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
  newSplineAt(ground ?? new THREE.Vector3());
});

function setTool(next: 'select' | 'bezier') {
  activeTool = next;
  toolSelect.setAttribute('aria-pressed', String(next === 'select'));
  toolBezier.setAttribute('aria-pressed', String(next === 'bezier'));
  container.dataset.tool = next;
  status.textContent = next === 'bezier' ? 'Bézier: trykk på en kurve for å sette inn et ankerpunkt. Punkter og håndtak dras som før.' : 'Velg: trykk for å velge, dra punkter og håndtak.';
}
toolSelect.addEventListener('click', () => setTool('select'));
toolBezier.addEventListener('click', () => setTool('bezier'));

toggleLayers.addEventListener('click', () => {
  const open = toggleLayers.getAttribute('aria-expanded') !== 'true';
  toggleLayers.setAttribute('aria-expanded', String(open));
  document.body.classList.toggle('layers-closed', !open);
});
// Laglisten er lukket på små skjermer til brukeren åpner den.
if (narrow.matches) toggleLayers.click();

/**
 * Ny gren fra andelen `at` av `parent`. Et retningshåndtak vises først,
 * foreslått vinkelrett på forelderen i festepunktet (mot `toward` eller mot
 * høyre i bildet). Grenen får sin egen retning; forelderens kurve og
 * håndtak endres ikke.
 */
function addBranchAt(parent: SplineView, at: number, toward?: THREE.Vector3) {
  const pts = parent.points.map((q) => new THREE.Vector3(...q));
  const { point, tangent } = pointOnPolyline(pts, at);
  const dir = (toward ?? viewport.projector.cameraRight).clone().projectOnPlane(tangent);
  if (dir.lengthSq() < 1e-6) dir.copy(viewport.projector.cameraUp).projectOnPlane(tangent);
  // Lang nok til å stikke ut av forelderens mesh.
  const length = parent.thickness + 20;
  status.textContent = 'Ny gren: dra håndtaket for retning og lengde, trykk på det for å godta, eller velg en akse.';
  gizmo.show(
    point,
    (direction, picked) =>
      editSkeleton(
        { kind: 'addChild', parent: parent.id, at, direction, length: picked ?? length, thickness: parent.thickness * 0.6 },
        'Ny gren',
      ),
    { dir: dir.normalize(), length, text: 'Ny gren: dra håndtaket eller velg akse' },
  );
}

/** Punktet og retningen i andelen `at` av buelengden langs en linje. */
function pointOnPolyline(pts: THREE.Vector3[], at: number) {
  const cum = [0];
  for (let i = 1; i < pts.length; i++) cum.push(cum[i - 1] + pts[i].distanceTo(pts[i - 1]));
  const target = THREE.MathUtils.clamp(at, 0, 1) * cum[cum.length - 1];
  let i = 1;
  while (i < pts.length - 1 && cum[i] < target) i++;
  const span = cum[i] - cum[i - 1];
  const t = span > 0 ? (target - cum[i - 1]) / span : 0;
  const tangent = pts[i].clone().sub(pts[i - 1]).normalize();
  return { point: pts[i - 1].clone().lerp(pts[i], t), tangent };
}

for (const [button, orientation] of [[orientFollow, 'follow_parent'], [orientKeep, 'keep_direction']] as const) {
  button.addEventListener('click', () => {
    const s = skeleton.selection && skeleton.spline(skeleton.selection.spline);
    if (!s || s.parent === null || s.orientation === orientation) return;
    editSkeleton({ kind: 'setOrientation', spline: s.id, orientation }, orientation === 'keep_direction' ? 'Grenen beholder retningen' : 'Grenen følger forelderen');
  });
}

addBranch.addEventListener('click', () => {
  const sel = skeleton.selection;
  const parent = sel && skeleton.spline(sel.spline);
  if (!parent) return;
  // Grenen festes i valgt punkt, ellers midt på, og peker mot høyre i bildet.
  const anchor = sel.index !== null && sel.index > 0 ? parent.anchors[sel.index] : null;
  addBranchAt(parent, anchor ? anchor.at : 0.5);
});

anchorKind.addEventListener('click', () => {
  const sel = skeleton.selection;
  const spline = sel && skeleton.spline(sel.spline);
  if (!spline || sel.index === null) return;
  const next = spline.anchors[sel.index].kind === 'corner' ? 'smooth' : 'corner';
  editSkeleton(
    { kind: 'setKind', spline: spline.id, index: sel.index, anchor_kind: next },
    next === 'corner' ? 'Hjørnepunkt' : 'Mykt punkt',
  );
});

removeSpline.addEventListener('click', () => {
  const sel = skeleton.selection;
  if (sel) editSkeleton({ kind: 'remove', spline: sel.spline }, sel.layer ? 'Slettet laget' : 'Slettet spline med grener');
});

thicknessInput.addEventListener('change', () => {
  const thickness = readMm(thicknessInput, 5);
  thicknessInput.value = String(thickness);
  const sel = skeleton.selection;
  if (sel) editSkeleton({ kind: 'setThickness', spline: sel.spline, thickness }, `Tykkelse ${thickness} mm`);
});

const tool = new PullTool(viewport, {
  radiusMm: () => readMm(radiusInput, 12),
  preview: (shape, pull) => {
    queuedPreview = { shape, pull };
    pumpPreview();
  },
  commit: (shape, pull) => {
    queuedPreview = null;
    const [ax, ay, az, nx, ny, nz, bx, by, bz] = pull;
    const along = (bx - ax) * nx + (by - ay) * ny + (bz - az) * nz;
    const what = `${along >= 0 ? 'Trakk ut' : 'Presset inn'} ${Math.abs(along).toFixed(1)} mm`;
    run({ kind: 'commitPull', shape, pull }, (t, ms) => `${what} · ${fmt(t)} trekanter · ${ms.toFixed(0)} ms`);
  },
  // Gjenoppretter siste bekreftede mesh ved å hente formen på nytt.
  cancel: () => {
    queuedPreview = null;
    run({ kind: 'controlPoints', spacingMm: geometry.spacingMm }, () => status.textContent ?? '');
  },
  status: (text) => (status.textContent = text),
});

addSpline.disabled = true;
openButton.disabled = true;
geometry.ready.then((version) => {
  addSpline.disabled = false;
  openButton.disabled = false;
  saveButton.disabled = false;
  status.textContent = `Geometrikjerne klar (beregningsversjon ${version}).`;
});

/** Lagrer prosjektet som en .cfproj-fil (JSON). */
async function saveProject() {
  if (saveButton.disabled) return;
  try {
    const { result } = await geometry.send({ kind: 'saveProject' });
    if (result.kind !== 'project') return;
    const blob = new Blob([result.text], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'skapning.cfproj';
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
    container.dataset.saved = result.text;
    status.textContent = 'Prosjektet er lagret som skapning.cfproj.';
  } catch (err) {
    status.textContent = `Kunne ikke lagre: ${(err as Error).message}`;
  }
}

/** Åpner en prosjektfil. Historikken starter på nytt fra filen. */
async function openProject(text: string, name: string) {
  skeleton.select(null);
  await run({ kind: 'openProject', text }, () => `Åpnet ${name}.`);
}

saveButton.addEventListener('click', saveProject);
openButton.addEventListener('click', () => fileInput.click());
fileInput.addEventListener('change', async () => {
  const file = fileInput.files?.[0];
  fileInput.value = '';
  if (file) await openProject(await file.text(), file.name);
});

const busy = () => tool.dragging || skeletonTool.dragging || gizmo.open || contourMode.active;

editContour.addEventListener('click', () => {
  const sel = skeleton.selection;
  if (sel?.station != null) editContourAt(sel.spline, sel.station);
});
const undo = () => !busy() && run({ kind: 'undo' }, () => 'Angret.');
const redo = () => !busy() && run({ kind: 'redo' }, () => 'Gjort om.');
undoButton.addEventListener('click', undo);
redoButton.addEventListener('click', redo);

window.addEventListener('keydown', (e) => {
  // Tallfelt beholder sine egne snarveier.
  if (e.target instanceof HTMLInputElement) return;
  const mod = e.ctrlKey || e.metaKey;
  const key = e.key.toLowerCase();
  if (mod && key === 'z') {
    e.preventDefault();
    if (e.shiftKey) redo();
    else undo();
  } else if (mod && key === 'y') {
    e.preventDefault();
    redo();
  } else if (mod && key === 's') {
    e.preventDefault();
    saveProject();
  } else if (mod && key === 'o') {
    e.preventDefault();
    if (!openButton.disabled) fileInput.click();
  }
});

// Punktavstand er en håndteringsinnstilling og endrer ikke geometrien (CF02).
spacingInput.addEventListener('change', () => {
  geometry.spacingMm = readMm(spacingInput, 6);
  spacingInput.value = String(geometry.spacingMm);
  run({ kind: 'controlPoints', spacingMm: geometry.spacingMm }, () =>
    `Punktavstand ${geometry.spacingMm} mm: ${fmt(viewport.controlPointCount)} kontrollpunkter.`,
  );
});
radiusInput.addEventListener('change', () => (radiusInput.value = String(readMm(radiusInput, 12))));

// Hurtigmeny ved høyreklikk. Høyre museknapp panorerer også kameraet, så
// menyen åpnes bare når pekeren ikke ble flyttet.
const menu = new ContextMenu(document.body);
let rightDown: { x: number; y: number; moved: boolean } | null = null;
container.addEventListener('pointerdown', (e) => {
  if (e.button === 2) rightDown = { x: e.clientX, y: e.clientY, moved: false };
}, { capture: true });
window.addEventListener('pointermove', (e) => {
  if (rightDown && e.buttons & 2 && Math.hypot(e.clientX - rightDown.x, e.clientY - rightDown.y) > 5) {
    rightDown.moved = true;
    menu.hide();
  }
});
container.addEventListener('contextmenu', (e) => {
  e.preventDefault();
  // Langt trykk med finger eller penn håndteres selv, så menyen ikke åpnes to ganger.
  if (busy() || rightDown?.moved || !rightDown) return;
  rightDown = null;
  openMenu(e.clientX, e.clientY);
});

// Trykk og langt trykk i arbeidsflaten. Et trykk velger det som ligger under
// pekeren (kontur, spline eller lag); et langt trykk med finger eller penn
// åpner samme hurtigmeny som høyreklikk med mus.
function installPressHandling() {
  let press: { id: number; x: number; y: number; moved: boolean; long: boolean; timer: number } | null = null;
  container.addEventListener('pointerdown', (e) => {
    if (e.button !== 0 || e.target !== viewport.renderer.domElement) return;
    skeletonTool.handledPointer = null;
    if (press) clearTimeout(press.timer);
    press = { id: e.pointerId, x: e.clientX, y: e.clientY, moved: false, long: false, timer: 0 };
    if (e.pointerType !== 'mouse') {
      const p = press;
      p.timer = window.setTimeout(() => {
        if (p.moved || press !== p) return;
        p.long = true;
        // Draget som startet med trykket, avbrytes uten endring.
        skeletonTool.cancel();
        tool.cancel();
        openMenu(p.x, p.y);
      }, LONG_PRESS_MS);
    }
  }, { capture: true });
  window.addEventListener('pointermove', (e) => {
    if (press?.id === e.pointerId && Math.hypot(e.clientX - press.x, e.clientY - press.y) > TAP_PX) {
      press.moved = true;
      clearTimeout(press.timer);
    }
  });
  window.addEventListener('pointerup', (e) => {
    const p = press;
    if (!p || p.id !== e.pointerId) return;
    press = null;
    clearTimeout(p.timer);
    if (p.moved || p.long || skeletonTool.handledPointer === e.pointerId || gizmo.open) return;
    tapAt(e.clientX, e.clientY, e.pointerType === 'touch');
  });
  window.addEventListener('pointercancel', (e) => {
    if (press?.id === e.pointerId) {
      clearTimeout(press.timer);
      press = null;
    }
  });
}

/** Velger konturen, splinen eller laget under pekeren, eller fjerner valget. */
function tapAt(x: number, y: number, touch: boolean) {
  if (contourMode.active) return;
  const ring = skeleton.pickRing(x, y, touch ? 16 : 8);
  if (ring) return setSelection({ spline: ring.spline, index: null, station: ring.station });
  const curve = skeleton.pickCurve(x, y, touch ? 16 : 8);
  if (curve) return setSelection({ spline: curve.spline, index: null, station: null });
  const hit = viewport.raycast(x, y);
  const s = hit && skeleton.spline(hit.shape);
  if (s) return setSelection({ spline: s.layer, index: null, station: null, layer: true });
  setSelection(null);
}

/** Et låst lag under pekeren (låste lag treffes ikke ellers). */
function lockedLayerAt(x: number, y: number) {
  const roots = skeleton.splines.filter((s) => s.parent === null && s.locked && !s.hidden);
  if (!roots.length) return null;
  viewport.setShapeStates(new Set(skeleton.splines.filter((s) => s.parent === null && s.hidden).map((s) => s.id)), new Set());
  const hit = viewport.raycast(x, y);
  syncLayers();
  return hit ? (roots.find((r) => r.id === hit.shape) ?? null) : null;
}

/** Splinen nærmest et punkt i scenen blant `ids`, med andel av buelengden. */
function nearestSpline(p: THREE.Vector3, ids?: Set<number>) {
  let best: { spline: SplineView; at: number; point: THREE.Vector3; d: number } | null = null;
  for (const s of skeleton.splines) {
    if (ids && !ids.has(s.id)) continue;
    const pts = s.points.map((q) => new THREE.Vector3(...q));
    pts.forEach((q, i) => {
      const d = q.distanceTo(p) - s.thickness;
      if (!best || d < best.d) best = { spline: s, at: polylineFraction(pts, i), point: q, d };
    });
  }
  return best as { spline: SplineView; at: number; point: THREE.Vector3; d: number } | null;
}

/** Splinene i kroppen med roten `root`. */
function bodyOf(root: number) {
  const ids = new Set([root]);
  for (let grew = true; grew; ) {
    grew = false;
    for (const s of skeleton.splines) {
      if (s.parent !== null && ids.has(s.parent) && !ids.has(s.id)) {
        ids.add(s.id);
        grew = true;
      }
    }
  }
  return ids;
}

/** Velger en spline. Valgt kontur beholdes når splinen er den samme. */
function select(spline: number, index: number | null = null, station?: number | null) {
  const keep = skeleton.selection?.spline === spline ? skeleton.selection.station : null;
  skeleton.select({ spline, index, station: station === undefined ? keep : station });
  updateSkeletonPanel();
}

/** Valgene som gjelder en spline i andelen `at`. */
function splineItems(s: SplineView, at: number): MenuItem[] {
  const near = s.profiles.findIndex((p) => Math.abs(p.at - at) < 0.08);
  return [
    { label: 'Ny gren her', action: () => addBranchAt(s, at) },
    { label: 'Ny kontur her', action: () => profiles.addAt(s, at) },
    { label: 'Lim inn kontur her', action: () => profiles.pasteAt(s, at), disabled: !profiles.clip },
    ...(near >= 0
      ? [
          { label: `Rediger kontur ${near + 1}`, action: () => editContourAt(s.id, near) },
          {
            label: 'Kopier konturen',
            action: () => {
              select(s.id, null, near);
              profiles.copyCurrent();
            },
          },
        ]
      : []),
    { label: s.flat_end ? 'Avrundet ende' : 'Flat ende', action: () => editSkeleton({ kind: 'setFlatEnd', spline: s.id, flat: !s.flat_end }, s.flat_end ? 'Avrundet ende' : 'Flat ende') },
  ];
}

const historyItems = (): MenuItem[] => [
  { label: 'Angre', action: undo, disabled: undoButton.disabled, hint: 'Ctrl+Z' },
  { label: 'Gjør om', action: redo, disabled: redoButton.disabled, hint: 'Ctrl+Shift+Z' },
];

function openMenu(x: number, y: number) {
  if (gizmo.open || contourMode.active) return;
  const pick = skeleton.pick(x, y, 12);
  if (pick?.kind === 'anchor') {
    const s = skeleton.spline(pick.spline)!;
    const a = s.anchors[pick.index];
    const tip = pick.index === s.anchors.length - 1;
    select(s.id, pick.index);
    return menu.show(x, y, `${tip ? 'Tuppen' : `Punkt ${pick.index + 1}`} på spline ${s.id}`, [
      { label: a.kind === 'corner' ? 'Gjør til mykt punkt' : 'Gjør til hjørnepunkt', action: () => anchorKind.click() },
      ...(tip
        ? []
        : [{ label: 'Bøy herfra', action: () => (status.textContent = 'Grip tuppen og hold Alt + Shift for å bøye fra dette punktet.') }]),
      ...splineItems(s, a.at),
      null,
      { label: 'Slett spline med grener', action: () => editSkeleton({ kind: 'remove', spline: s.id }, 'Slettet spline med grener') },
    ]);
  }
  const ring = skeleton.pickRing(x, y, 8);
  if (ring) {
    const s = skeleton.spline(ring.spline)!;
    select(s.id, null, ring.station);
    return menu.show(x, y, `Kontur ${ring.station + 1} på ${s.parent === null ? 'hovedsplinen' : splineName(s)}`, [
      { label: 'Rediger kontur', action: () => editContourAt(s.id, ring.station) },
      { label: 'Kopier konturen', action: () => profiles.copyCurrent() },
      { label: 'Slett konturen', action: () => editSkeleton({ kind: 'removeProfile', spline: s.id, station: ring.station }, 'Slettet kontur') },
    ]);
  }
  const curve = skeleton.pickCurve(x, y, 10);
  if (curve) {
    const s = skeleton.spline(curve.spline)!;
    select(s.id);
    return menu.show(x, y, `Spline ${s.id} ved ${Math.round(curve.at * 100)} %`, [
      { label: 'Legg inn punkt her', action: () => editSkeleton({ kind: 'insertAnchor', spline: s.id, at: curve.at }, 'La inn et nytt punkt') },
      ...splineItems(s, curve.at),
      null,
      { label: 'Slett spline med grener', action: () => editSkeleton({ kind: 'remove', spline: s.id }, 'Slettet spline med grener') },
    ]);
  }
  const hit = viewport.raycast(x, y);
  const lockedHit = !hit && lockedLayerAt(x, y);
  if (lockedHit) {
    return menu.show(x, y, `${splineName(lockedHit)} (låst)`, [
      { label: 'Lås opp laget', action: () => setLayer(lockedHit.id, lockedHit.hidden, false) },
      { label: 'Skjul laget', action: () => setLayer(lockedHit.id, true, lockedHit.locked) },
    ]);
  }
  if (hit) {
    const near = nearestSpline(hit.point, bodyOf(hit.shape));
    if (near) {
      const s = near.spline;
      select(s.id);
      const out = hit.point.clone().sub(near.point);
      const layer = skeleton.spline(s.layer)!;
      return menu.show(x, y, splineName(layer), [
        { label: 'Velg hele laget', action: () => setSelection({ spline: layer.id, index: null, station: null, layer: true }) },
        { label: 'Ny gren ut herfra', action: () => addBranchAt(s, near.at, out) },
        { label: 'Ny kontur her', action: () => profiles.addAt(s, near.at) },
        { label: 'Lim inn kontur her', action: () => profiles.pasteAt(s, near.at), disabled: !profiles.clip },
        { label: s.flat_end ? 'Avrundet ende' : 'Flat ende', action: () => editSkeleton({ kind: 'setFlatEnd', spline: s.id, flat: !s.flat_end }, s.flat_end ? 'Avrundet ende' : 'Flat ende') },
        null,
        ...historyItems(),
        null,
        { label: 'Skjul laget', action: () => setLayer(layer.id, true, layer.locked) },
        { label: 'Lås laget', action: () => setLayer(layer.id, layer.hidden, true) },
        { label: 'Slett laget', action: () => editSkeleton({ kind: 'remove', spline: hit.shape }, 'Slettet laget') },
      ]);
    }
  }
  const ground = viewport.groundPoint(x, y);
  menu.show(x, y, 'Arbeidsrommet', [
    {
      label: 'Ny spline her',
      disabled: addSpline.disabled || !ground,
      action: () => newSplineAt(ground!),
    },
    null,
    ...historyItems(),
    { label: 'Tilbakestill visningen', action: () => viewport.resetView() },
  ]);
}

// Hjelper for automatiserte nettlesertester: skjermposisjonen til det synlige
// kontrollpunktet nærmest midten av arbeidsflaten.
declare global {
  interface Window {
    creatureForgeTest?: {
      gripNearCenter(): { x: number; y: number } | null;
      splineAnchor(spline: number, index: number): { x: number; y: number } | null;
      splineLength(spline: number): number | null;
      /** Skjermposisjonen til et punkt langs kurven (andel av punktene). */
      splineCurvePoint(spline: number, fraction: number): { x: number; y: number } | null;
      /** Ankerpunktene i verdenskoordinater. */
      splineAnchorsWorld(spline: number): Vec3[] | null;
      /** Plassen til konturene langs splinen (andel av buelengden). */
      profileAts(spline: number): number[] | null;
      /** Skjermposisjonen til punkt `i` i omrisset til en kontur. */
      ringScreen(spline: number, station: number, i: number): { x: number; y: number } | null;
      cameraPosition(): Vec3;
      /** Forelder, feste og orientering. */
      splineInfo(spline: number): { parent: number | null; at: number; orientation: string; layer: number } | null;
    };
  }
}
window.creatureForgeTest = {
  gripNearCenter() {
    // Kontrollpunktet nærmest midten som ikke ligger oppå skjelettet (det har forrang).
    const rect = viewport.renderer.domElement.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    let best: { x: number; y: number; d: number } | null = null;
    for (let r = 0; r < Math.max(rect.width, rect.height) / 2; r += 8) {
      for (let a = 0; a < 16; a++) {
        const x = cx + r * Math.cos((a / 16) * 2 * Math.PI);
        const y = cy + r * Math.sin((a / 16) * 2 * Math.PI);
        const p = viewport.pickControlPoint(x, y, 6);
        if (!p) continue;
        const s = viewport.toScreen(p.position);
        const q = { x: rect.left + s.x, y: rect.top + s.y };
        if (skeleton.pick(q.x, q.y, 20) || skeleton.pickCurve(q.x, q.y, 12)) continue;
        const d = Math.hypot(q.x - cx, q.y - cy);
        if (!best || d < best.d) best = { ...q, d };
      }
      if (best) return { x: best.x, y: best.y };
    }
    return null;
  },
  splineAnchor(spline, index) {
    const s = skeleton.spline(spline);
    const a = s?.anchors.at(index);
    if (!a) return null;
    const rect = viewport.renderer.domElement.getBoundingClientRect();
    const p = viewport.toScreen(new THREE.Vector3(...a.position));
    return { x: rect.left + p.x, y: rect.top + p.y };
  },
  splineLength(spline) {
    return skeleton.spline(spline)?.length ?? null;
  },
  splineCurvePoint(spline, fraction) {
    const s = skeleton.spline(spline);
    if (!s) return null;
    const q = s.points[Math.round(fraction * (s.points.length - 1))];
    const rect = viewport.renderer.domElement.getBoundingClientRect();
    const p = viewport.toScreen(new THREE.Vector3(...q));
    return { x: rect.left + p.x, y: rect.top + p.y };
  },
  profileAts(spline) {
    return skeleton.spline(spline)?.profiles.map((p) => p.at) ?? null;
  },
  ringScreen(spline, station, i) {
    const q = skeleton.spline(spline)?.profiles[station]?.ring[i];
    if (!q) return null;
    const rect = viewport.renderer.domElement.getBoundingClientRect();
    const p = viewport.toScreen(new THREE.Vector3(...q));
    return { x: rect.left + p.x, y: rect.top + p.y };
  },
  cameraPosition() {
    return arr(viewport.camera.position);
  },
  splineInfo(spline) {
    const s = skeleton.spline(spline);
    return s ? { parent: s.parent, at: s.at, orientation: s.orientation, layer: s.layer } : null;
  },
  splineAnchorsWorld(spline) {
    return skeleton.spline(spline)?.anchors.map((a) => a.position) ?? null;
  },
};
