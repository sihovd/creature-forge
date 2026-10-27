import { GeometryClient } from './geometry-client';
import { Viewport } from './viewport';

const SPHERE_RADIUS_MM = 25;
const TARGET_EDGE_MM = 1.5;

const container = document.querySelector<HTMLElement>('#viewport')!;
const status = document.querySelector<HTMLElement>('#status')!;
const hint = document.querySelector<HTMLElement>('#empty-hint')!;
const addSphere = document.querySelector<HTMLButtonElement>('#add-sphere')!;

// Arbeidsrommet starter alltid tomt; brukeren legger selv til første form.
const viewport = new Viewport(container);
const geometry = new GeometryClient();
let revision = 0;

addSphere.disabled = true;
geometry.ready.then((version) => {
  addSphere.disabled = false;
  status.textContent = `Geometrikjerne klar (beregningsversjon ${version}).`;
});

addSphere.addEventListener('click', async () => {
  addSphere.disabled = true;
  try {
    const { mesh, computeMs } = await geometry.request({
      kind: 'createSphere',
      revision: ++revision,
      radiusMm: SPHERE_RADIUS_MM,
      targetEdgeMm: TARGET_EDGE_MM,
    });
    const triangles = viewport.addMesh(mesh, { x: 0, y: SPHERE_RADIUS_MM, z: 0 });
    container.dataset.shapeCount = String(viewport.shapeCount);
    hint.hidden = true;
    status.textContent = `Kule ${SPHERE_RADIUS_MM * 2} mm: ${triangles.toLocaleString('nb-NO')} trekanter, beregnet på ${computeMs.toFixed(1)} ms.`;
  } catch (err) {
    status.textContent = `Feil i geometrikjernen: ${(err as Error).message}`;
  } finally {
    addSphere.disabled = false;
  }
});
