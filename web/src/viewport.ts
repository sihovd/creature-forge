import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { Projector } from './grip-math';
import type { MeshResult } from './protocol';

export interface ControlPoint {
  shape: number;
  position: THREE.Vector3;
  normal: THREE.Vector3;
}

const POINT_COLOR = 0x5d6b62;
const ACTIVE_COLOR = 0xd2652d;

/** 3D-arbeidsflaten. Én scene-enhet er én millimeter. */
export class Viewport {
  readonly renderer: THREE.WebGLRenderer;
  readonly camera = new THREE.PerspectiveCamera(40, 1, 0.5, 5000);
  readonly controls: OrbitControls;
  private scene = new THREE.Scene();
  private shapes = new Map<number, THREE.Mesh>();
  private material = new THREE.MeshStandardMaterial({ color: 0xd9c7a8, roughness: 0.65 });
  /** Låste lag tegnes litt nedtonet. */
  private lockedMaterial = new THREE.MeshStandardMaterial({ color: 0xc9c4bb, roughness: 0.8, transparent: true, opacity: 0.75 });
  /** Skjulte og låste lag (kroppen har samme id som rotsplinen). */
  private hiddenShapes = new Set<number>();
  private lockedShapes = new Set<number>();
  private points = new Map<number, ControlPoint[]>();
  private pointMarkers = new THREE.Group();
  private pointGeometry = new THREE.SphereGeometry(0.55, 10, 8);
  private pointMaterial = new THREE.MeshBasicMaterial({ color: POINT_COLOR });
  private arrow = new THREE.ArrowHelper(new THREE.Vector3(0, 1, 0), new THREE.Vector3(), 10, ACTIVE_COLOR, 3, 2);
  private influence: THREE.Mesh;
  private raycaster = new THREE.Raycaster();
  private frameCallbacks: (() => void)[] = [];
  constructor(private container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    container.prepend(this.renderer.domElement);

    this.scene.background = new THREE.Color(0xf4f5f2);
    this.camera.position.set(75, 70, 105);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.target.set(0, 25, 0);
    this.controls.enableDamping = true;

    // Rutenett på 200 × 200 mm med 10 mm ruter, liggende i XZ-planet.
    this.scene.add(new THREE.GridHelper(200, 20, 0xb9bdb3, 0xdcdfd8));
    this.scene.add(new THREE.HemisphereLight(0xffffff, 0xc8c2b4, 1.6));
    const sun = new THREE.DirectionalLight(0xffffff, 1.4);
    sun.position.set(80, 160, 120);
    this.scene.add(sun);

    this.influence = new THREE.Mesh(
      new THREE.RingGeometry(0.96, 1, 64),
      new THREE.MeshBasicMaterial({ color: ACTIVE_COLOR, side: THREE.DoubleSide, transparent: true, opacity: 0.7, depthTest: false }),
    );
    this.arrow.visible = false;
    this.influence.visible = false;
    this.scene.add(this.pointMarkers, this.arrow, this.influence);

    new ResizeObserver(() => this.resize()).observe(container);
    this.resize();
    this.renderer.setAnimationLoop(() => {
      this.controls.update();
      for (const f of this.frameCallbacks) f();
      this.renderer.render(this.scene, this.camera);
    });
  }

  /** Kalles før hvert bilde tegnes, for eksempel for å plassere merkelapper. */
  onFrame(f: () => void) {
    this.frameCallbacks.push(f);
  }

  /** Legger til et objekt i scenen (brukes av skjelettvisningen). */
  addToScene(obj: THREE.Object3D) {
    this.scene.add(obj);
  }

  get shapeCount() {
    return this.shapes.size;
  }

  get controlPointCount() {
    let n = 0;
    for (const pts of this.points.values()) n += pts.length;
    return n;
  }

  setShapeMesh(shape: number, result: MeshResult | null) {
    const old = this.shapes.get(shape);
    if (old) {
      this.scene.remove(old);
      old.geometry.dispose();
      this.shapes.delete(shape);
    }
    if (!result) return 0;
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(result.positions, 3));
    geometry.setAttribute('normal', new THREE.BufferAttribute(result.normals, 3));
    geometry.setIndex(new THREE.BufferAttribute(result.indices, 1));
    geometry.computeBoundingSphere();
    const mesh = new THREE.Mesh(geometry, this.lockedShapes.has(shape) ? this.lockedMaterial : this.material);
    mesh.visible = !this.hiddenShapes.has(shape);
    mesh.userData.shape = shape;
    this.shapes.set(shape, mesh);
    this.scene.add(mesh);
    return result.indices.length / 3;
  }

  setControlPoints(shape: number, data: Float32Array) {
    const pts: ControlPoint[] = [];
    for (let i = 0; i + 5 < data.length; i += 6) {
      pts.push({
        shape,
        position: new THREE.Vector3(data[i], data[i + 1], data[i + 2]),
        normal: new THREE.Vector3(data[i + 3], data[i + 4], data[i + 5]).normalize(),
      });
    }
    if (pts.length) this.points.set(shape, pts);
    else this.points.delete(shape);
    this.rebuildPointMarkers();
  }

  /**
   * Skjulte lag vises ikke og kan ikke treffes. Låste lag vises, men treffes
   * ikke av pekeren og har ingen kontrollpunkter, så klikk går gjennom dem.
   */
  setShapeStates(hidden: Set<number>, locked: Set<number>) {
    this.hiddenShapes = hidden;
    this.lockedShapes = locked;
    for (const [id, mesh] of this.shapes) {
      mesh.visible = !hidden.has(id);
      mesh.material = locked.has(id) ? this.lockedMaterial : this.material;
    }
    this.rebuildPointMarkers();
  }

  private active(shape: number) {
    return !this.hiddenShapes.has(shape) && !this.lockedShapes.has(shape);
  }

  /** Formene pekeren kan treffe. */
  private pickable() {
    return [...this.shapes].filter(([id]) => this.active(id)).map(([, m]) => m);
  }

  setControlPointsVisible(visible: boolean) {
    this.pointMarkers.visible = visible;
  }

  /** Kameraet slik grepsberegningene trenger det. */
  get projector(): Projector {
    const right = new THREE.Vector3().setFromMatrixColumn(this.camera.matrixWorld, 0).normalize();
    const up = new THREE.Vector3().setFromMatrixColumn(this.camera.matrixWorld, 1).normalize();
    return {
      toScreen: (p) => this.toScreen(p),
      pixelsPerMm: (p) => this.pixelsPerMm(p),
      cameraRight: right,
      cameraUp: up,
    };
  }

  private rebuildPointMarkers() {
    this.pointMarkers.clear();
    const all = [...this.points].filter(([id]) => this.active(id)).flatMap(([, pts]) => pts);
    if (!all.length) return;
    const inst = new THREE.InstancedMesh(this.pointGeometry, this.pointMaterial, all.length);
    const m = new THREE.Matrix4();
    all.forEach((p, i) => inst.setMatrixAt(i, m.makeTranslation(p.position)));
    this.pointMarkers.add(inst);
  }

  /** Viser pil ved grepet og påvirkningsområdet rundt roten. `length` er pilens lengde i mm. */
  showGrip(
    position: THREE.Vector3,
    direction: THREE.Vector3,
    radiusMm: number,
    ringAt = position,
    ringNormal = direction,
    length = 10,
  ) {
    this.arrow.position.copy(position);
    this.arrow.setDirection(direction);
    this.arrow.setLength(Math.max(length, 4), 3, 2);
    this.arrow.visible = true;
    this.influence.position.copy(ringAt);
    this.influence.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), ringNormal);
    this.influence.scale.setScalar(radiusMm);
    this.influence.visible = true;
  }

  hideGrip() {
    this.arrow.visible = false;
    this.influence.visible = false;
  }

  /** Skjermkoordinater i CSS-piksler for et punkt i scenen. */
  toScreen(p: THREE.Vector3) {
    const v = p.clone().project(this.camera);
    const { clientWidth: w, clientHeight: h } = this.renderer.domElement;
    return new THREE.Vector2(((v.x + 1) / 2) * w, ((1 - v.y) / 2) * h);
  }

  /** Nærmeste synlige kontrollpunkt innenfor `maxPx` piksler fra pekeren. */
  pickControlPoint(clientX: number, clientY: number, maxPx: number): ControlPoint | null {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const mouse = new THREE.Vector2(clientX - rect.left, clientY - rect.top);
    const toCamera = new THREE.Vector3();
    let best: ControlPoint | null = null;
    let bestDist = maxPx;
    for (const [shape, pts] of this.points) {
      if (!this.active(shape)) continue;
      for (const p of pts) {
        toCamera.subVectors(this.camera.position, p.position);
        if (toCamera.dot(p.normal) <= 0) continue;
        const d = this.toScreen(p.position).distanceTo(mouse);
        if (d < bestDist && !this.occluded(p.position)) {
          best = p;
          bestDist = d;
        }
      }
    }
    return best;
  }

  private occluded(p: THREE.Vector3) {
    const dir = p.clone().sub(this.camera.position);
    const dist = dir.length();
    this.raycaster.set(this.camera.position, dir.normalize());
    const visible = [...this.shapes.values()].filter((m) => m.visible);
    const hit = this.raycaster.intersectObjects(visible, false)[0];
    return !!hit && hit.distance < dist - 0.5;
  }

  /** Formen og punktet under pekeren, eller null. */
  raycast(clientX: number, clientY: number): { shape: number; point: THREE.Vector3 } | null {
    this.setRay(clientX, clientY);
    const hit = this.raycaster.intersectObjects(this.pickable(), false)[0];
    return hit ? { shape: hit.object.userData.shape as number, point: hit.point } : null;
  }

  /** Punktet på gulvet (y = 0) under pekeren, eller null. */
  groundPoint(clientX: number, clientY: number): THREE.Vector3 | null {
    this.setRay(clientX, clientY);
    return this.raycaster.ray.intersectPlane(new THREE.Plane(new THREE.Vector3(0, 1, 0), 0), new THREE.Vector3());
  }

  /** Flytter kameraet tilbake til utgangsvisningen. */
  resetView() {
    this.camera.position.set(75, 70, 105);
    this.controls.target.set(0, 25, 0);
  }

  private setRay(clientX: number, clientY: number) {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((clientX - rect.left) / rect.width) * 2 - 1, -((clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
  }

  /** Antall skjermpiksler per millimeter ved et punkt i scenen. */
  pixelsPerMm(p: THREE.Vector3) {
    const dist = p.distanceTo(this.camera.position);
    const height = 2 * dist * Math.tan(THREE.MathUtils.degToRad(this.camera.fov / 2));
    return this.renderer.domElement.clientHeight / height;
  }

  private resize() {
    const { clientWidth: w, clientHeight: h } = this.container;
    if (w === 0 || h === 0) return;
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }
}
