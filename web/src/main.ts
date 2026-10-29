import { ContextMenu, type MenuItem } from './context-menu';
import { GeometryClient } from './geometry-client';
import * as THREE from 'three';
import type { Command, GeometryResult, PullParams, SkeletonEdit, SplineView, Vec3 } from './protocol';
import { ProfilePanel } from './profile-panel';
import { PullTool } from './pull-tool';
import { polylineFraction } from './skeleton-math';
import { SkeletonTool } from './skeleton-tool';
import { SkeletonView } from './skeleton-view';
import { Viewport } from './viewport';

const SPHERE_RADIUS_MM = 25;

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!;
const container = $('#viewport');
const status = $('#status');
const hint = $('#empty-hint');
const addSphere = $<HTMLButtonElement>('#add-sphere');
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
const fmt = (n: number) => n.toLocaleString('nb-NO');

function readMm(input: HTMLInputElement, fallback: number) {
  const v = Number(input.value.replace(',', '.'));
  return Number.isFinite(v) && v > 0 ? Math.min(Math.max(v, Number(input.min)), Number(input.max)) : fallback;
}

function apply(result: GeometryResult) {
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
  if (result.created !== null) skeleton.select({ spline: result.created, index: null });
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


function updateSkeletonPanel() {
  const sel = skeleton.selection;
  const spline = sel ? skeleton.spline(sel.spline) : null;
  const anchor = spline && sel!.index !== null ? spline.anchors[sel!.index] : null;
  addBranch.disabled = !spline;
  removeSpline.disabled = !spline;
  anchorKind.disabled = !anchor;
  anchorKind.textContent = anchor?.kind === 'corner' ? 'Mykt punkt' : 'Hjørnepunkt';
  if (spline) thicknessInput.value = String(spline.thickness);
  profiles?.update();
}

function editSkeleton(edit: SkeletonEdit, what: string) {
  run({ kind: 'commitSkeleton', edit }, (_t, ms) => `${what} · ${ms.toFixed(0)} ms`);
}

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

const profiles: ProfilePanel = new ProfilePanel(skeleton, {
  preview: (edit) => previewSkeletonEdit(edit),
  commit: (edit, what) => {
    queuedSkeletonPreview = null;
    editSkeleton(edit, what);
  },
  status: (text) => (status.textContent = text),
});

addSpline.addEventListener('click', () => {
  // Nye rotspliner står loddrett og legges ved siden av hverandre.
  const roots = skeleton.splines.filter((s) => s.parent === null).length;
  editSkeleton(
    { kind: 'addRoot', start: [roots * 30, 0, 0], direction: [0, 1, 0], length: 40, thickness: readMm(thicknessInput, 5) },
    'Ny spline',
  );
});

/** Ny gren fra andelen `at` av `parent`, mot `toward` eller mot høyre i bildet. */
function addBranchAt(parent: SplineView, at: number, toward?: THREE.Vector3) {
  const anchor = parent.anchors.reduce((a, b) => (Math.abs(b.at - at) < Math.abs(a.at - at) ? b : a));
  const tangent = new THREE.Vector3(...anchor.tangent);
  const dir = (toward ?? viewport.projector.cameraRight).clone().projectOnPlane(tangent);
  if (dir.lengthSq() < 1e-6) dir.copy(viewport.projector.cameraUp).projectOnPlane(tangent);
  editSkeleton(
    // Lang nok til å stikke ut av forelderens mesh.
    { kind: 'addChild', parent: parent.id, at, direction: arr(dir.normalize()), length: parent.thickness + 20, thickness: parent.thickness * 0.6 },
    'Ny gren',
  );
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
  if (sel) editSkeleton({ kind: 'remove', spline: sel.spline }, 'Slettet spline med grener');
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

addSphere.disabled = true;
addSpline.disabled = true;
geometry.ready.then((version) => {
  addSphere.disabled = false;
  addSpline.disabled = false;
  status.textContent = `Geometrikjerne klar (beregningsversjon ${version}).`;
});

addSphere.addEventListener('click', () =>
  run({ kind: 'addSphere', center: [0, SPHERE_RADIUS_MM, 0], radiusMm: SPHERE_RADIUS_MM }, (t, ms) =>
    `Kule ${SPHERE_RADIUS_MM * 2} mm: ${fmt(t)} trekanter, beregnet på ${ms.toFixed(1)} ms.`,
  ),
);

const busy = () => tool.dragging || skeletonTool.dragging;
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
  if (busy() || rightDown?.moved) return;
  rightDown = null;
  openMenu(e.clientX, e.clientY);
});

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
          { label: `Rediger kontur ${near + 1}`, action: () => select(s.id, null, near) },
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
  if (hit) {
    const near = nearestSpline(hit.point, bodyOf(hit.shape));
    if (near) {
      const s = near.spline;
      select(s.id);
      const out = hit.point.clone().sub(near.point);
      return menu.show(x, y, 'Formen', [
        { label: 'Ny gren ut herfra', action: () => addBranchAt(s, near.at, out) },
        { label: 'Ny kontur her', action: () => profiles.addAt(s, near.at) },
        { label: 'Lim inn kontur her', action: () => profiles.pasteAt(s, near.at), disabled: !profiles.clip },
        { label: s.flat_end ? 'Avrundet ende' : 'Flat ende', action: () => editSkeleton({ kind: 'setFlatEnd', spline: s.id, flat: !s.flat_end }, s.flat_end ? 'Avrundet ende' : 'Flat ende') },
        null,
        ...historyItems(),
        null,
        { label: 'Slett formen', action: () => editSkeleton({ kind: 'remove', spline: hit.shape }, 'Slettet formen') },
      ]);
    }
  }
  const ground = viewport.groundPoint(x, y);
  menu.show(x, y, 'Arbeidsrommet', [
    {
      label: 'Legg til kule her',
      disabled: addSphere.disabled || !ground,
      action: () =>
        run({ kind: 'addSphere', center: [ground!.x, SPHERE_RADIUS_MM, ground!.z], radiusMm: SPHERE_RADIUS_MM }, (t, ms) =>
          `Kule ${SPHERE_RADIUS_MM * 2} mm: ${fmt(t)} trekanter, beregnet på ${ms.toFixed(1)} ms.`,
        ),
    },
    {
      label: 'Ny spline her',
      disabled: addSpline.disabled || !ground,
      action: () => editSkeleton({ kind: 'addRoot', start: [ground!.x, 0, ground!.z], direction: [0, 1, 0], length: 40, thickness: readMm(thicknessInput, 5) }, 'Ny spline'),
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
    };
  }
}
window.creatureForgeTest = {
  gripNearCenter() {
    const rect = viewport.renderer.domElement.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    const p = viewport.pickControlPoint(cx, cy, Math.max(rect.width, rect.height));
    if (!p) return null;
    const s = viewport.toScreen(p.position);
    return { x: rect.left + s.x, y: rect.top + s.y };
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
  splineAnchorsWorld(spline) {
    return skeleton.spline(spline)?.anchors.map((a) => a.position) ?? null;
  },
};
