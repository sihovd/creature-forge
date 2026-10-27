import { GeometryClient } from './geometry-client';
import type { Command, GeometryResult, PullParams } from './protocol';
import { PullTool, type PanelMode } from './pull-tool';
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
const modeButtons = [...document.querySelectorAll<HTMLButtonElement>('[data-mode]')];
const showCenterlines = $<HTMLInputElement>('#show-centerlines');

// Arbeidsrommet starter alltid tomt; brukeren legger selv til første form.
const viewport = new Viewport(container);
const geometry = new GeometryClient();
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
  let triangles = 0;
  for (const s of result.shapes) {
    triangles += viewport.setShapeMesh(s.shape, s.mesh);
    viewport.setControlPoints(s.shape, s.controlPoints);
    viewport.setCenterlines(s.shape, s.centerlines);
  }
  undoButton.disabled = !result.state.canUndo;
  redoButton.disabled = !result.state.canRedo;
  container.dataset.shapeCount = String(viewport.shapeCount);
  container.dataset.controlPoints = String(viewport.controlPointCount);
  container.dataset.centerlines = String(viewport.centerlineCount);
  container.dataset.revision = String(result.state.revision);
  hint.hidden = viewport.shapeCount > 0;
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
let queuedPreview: { shape: number; pull: PullParams; target: number | null } | null = null;
let previewInFlight = false;
async function pumpPreview() {
  if (previewInFlight || !queuedPreview) return;
  const { shape, pull, target } = queuedPreview;
  queuedPreview = null;
  previewInFlight = true;
  try {
    const { result } = await geometry.send(
      target === null ? { kind: 'previewPull', shape, pull } : { kind: 'previewEdit', shape, target, pull },
    );
    if (tool.dragging) apply(result);
  } catch {
    // Et ugyldig mellomsteg overskriver ikke siste gyldige visning.
  } finally {
    previewInFlight = false;
    if (tool.dragging) pumpPreview();
  }
}

let panelMode: PanelMode = 'pull';
for (const b of modeButtons) {
  b.addEventListener('click', () => {
    panelMode = b.dataset.mode as PanelMode;
    for (const other of modeButtons) other.setAttribute('aria-pressed', String(other === b));
  });
}
showCenterlines.addEventListener('change', () => viewport.setCenterlinesVisible(showCenterlines.checked));

const tool = new PullTool(viewport, {
  radiusMm: () => readMm(radiusInput, 12),
  panelMode: () => panelMode,
  preview: (shape, pull, target) => {
    queuedPreview = { shape, pull, target };
    pumpPreview();
  },
  commit: (shape, pull, target) => {
    queuedPreview = null;
    const [ax, ay, az, nx, ny, nz, bx, by, bz] = pull;
    const ab = Math.hypot(bx - ax, by - ay, bz - az);
    const along = (bx - ax) * nx + (by - ay) * ny + (bz - az) * nz;
    const what =
      target !== null
        ? `Endret senterlinjen (A–B ${ab.toFixed(1)} mm)`
        : Math.abs(ab - Math.abs(along)) < 0.01
          ? `${along >= 0 ? 'Trakk ut' : 'Presset inn'} ${Math.abs(along).toFixed(1)} mm`
          : `Trakk ut og bøyde (A–B ${ab.toFixed(1)} mm)`;
    run(
      target === null ? { kind: 'commitPull', shape, pull } : { kind: 'commitEdit', shape, target, pull },
      (t, ms) => `${what} · ${fmt(t)} trekanter · ${ms.toFixed(0)} ms`,
    );
  },
  // Gjenoppretter siste bekreftede mesh ved å hente formen på nytt.
  cancel: () => {
    queuedPreview = null;
    run({ kind: 'controlPoints', spacingMm: geometry.spacingMm }, () => status.textContent ?? '');
  },
  status: (text) => (status.textContent = text),
});

addSphere.disabled = true;
geometry.ready.then((version) => {
  addSphere.disabled = false;
  status.textContent = `Geometrikjerne klar (beregningsversjon ${version}).`;
});

addSphere.addEventListener('click', () =>
  run({ kind: 'addSphere', center: [0, SPHERE_RADIUS_MM, 0], radiusMm: SPHERE_RADIUS_MM }, (t, ms) =>
    `Kule ${SPHERE_RADIUS_MM * 2} mm: ${fmt(t)} trekanter, beregnet på ${ms.toFixed(1)} ms.`,
  ),
);

const undo = () => !tool.dragging && run({ kind: 'undo' }, () => 'Angret.');
const redo = () => !tool.dragging && run({ kind: 'redo' }, () => 'Gjort om.');
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
      firstCenterlineTip(): { x: number; y: number } | null;
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
  firstCenterlineTip() {
    const rect = viewport.renderer.domElement.getBoundingClientRect();
    const line = viewport.pickCenterlineTip(0, 0, Infinity);
    if (!line) return null;
    const s = viewport.toScreen(line.tip);
    return { x: rect.left + s.x, y: rect.top + s.y };
  },
};
