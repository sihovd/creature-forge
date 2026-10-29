import { GeometryClient } from './geometry-client';
import * as THREE from 'three';
import type { Command, GeometryResult, PullParams, SkeletonEdit, Vec3 } from './protocol';
import { PullTool } from './pull-tool';
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

addSpline.addEventListener('click', () => {
  // Nye rotspliner står loddrett og legges ved siden av hverandre.
  const roots = skeleton.splines.filter((s) => s.parent === null).length;
  editSkeleton(
    { kind: 'addRoot', start: [roots * 30, 0, 0], direction: [0, 1, 0], length: 40, thickness: readMm(thicknessInput, 5) },
    'Ny spline',
  );
});

addBranch.addEventListener('click', () => {
  const sel = skeleton.selection;
  const parent = sel && skeleton.spline(sel.spline);
  if (!parent) return;
  // Grenen festes i valgt punkt, ellers midt på, og peker mot høyre i bildet.
  const anchor = sel.index !== null && sel.index > 0 ? parent.anchors[sel.index] : null;
  const at = anchor ? anchor.at : 0.5;
  const tangent = new THREE.Vector3(...(anchor ?? parent.anchors[0]).tangent);
  const dir = viewport.projector.cameraRight.clone().projectOnPlane(tangent);
  if (dir.lengthSq() < 1e-6) dir.copy(viewport.projector.cameraUp).projectOnPlane(tangent);
  editSkeleton(
    // Lang nok til å stikke ut av forelderens mesh.
    { kind: 'addChild', parent: parent.id, at, direction: arr(dir.normalize()), length: parent.thickness + 20, thickness: parent.thickness * 0.6 },
    'Ny gren',
  );
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

// Hjelper for automatiserte nettlesertester: skjermposisjonen til det synlige
// kontrollpunktet nærmest midten av arbeidsflaten.
declare global {
  interface Window {
    creatureForgeTest?: {
      gripNearCenter(): { x: number; y: number } | null;
      splineAnchor(spline: number, index: number): { x: number; y: number } | null;
      splineLength(spline: number): number | null;
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
};
