import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { Projector } from './grip-math';
import { CENTERLINE_SAMPLES, CENTERLINE_STRIDE, type MeshResult } from './protocol';

export interface ControlPoint {
  shape: number;
  position: THREE.Vector3;
  normal: THREE.Vector3;
}

/** En varig senterlinje fra et uttrekk. `target` er uttrekkets plass i historikken. */
export interface Centerline {
  shape: number;
  target: number;
  radius: number;
  anchor: THREE.Vector3;
  normal: THREE.Vector3;
  tip: THREE.Vector3;
}

const POINT_COLOR = 0x5d6b62;
const ACTIVE_COLOR = 0xd2652d;
const CENTERLINE_COLOR = 0x2f6f5e;

/** 3D-arbeidsflaten. Én scene-enhet er én millimeter. */
export class Viewport {
  readonly renderer: THREE.WebGLRenderer;
  readonly camera = new THREE.PerspectiveCamera(40, 1, 0.5, 5000);
  readonly controls: OrbitControls;
  private scene = new THREE.Scene();
  private shapes = new Map<number, THREE.Mesh>();
  private material = new THREE.MeshStandardMaterial({ color: 0xd9c7a8, roughness: 0.65 });
  private points = new Map<number, ControlPoint[]>();
  private pointMarkers = new THREE.Group();
  private pointGeometry = new THREE.SphereGeometry(0.55, 10, 8);
  private pointMaterial = new THREE.MeshBasicMaterial({ color: POINT_COLOR });
  private arrow = new THREE.ArrowHelper(new THREE.Vector3(0, 1, 0), new THREE.Vector3(), 10, ACTIVE_COLOR, 3, 2);
  private influence: THREE.Mesh;
  private raycaster = new THREE.Raycaster();
  private centerlines = new Map<number, Centerline[]>();
  private centerlineGroup = new THREE.Group();
  private handleGeometry = new THREE.SphereGeometry(1.1, 16, 12);
  private lineMaterial = new THREE.LineBasicMaterial({ color: CENTERLINE_COLOR, depthTest: false, transparent: true, opacity: 0.9 });
  private handleMaterial = new THREE.MeshBasicMaterial({ color: CENTERLINE_COLOR, depthTest: false });
  private rootMaterial = new THREE.MeshBasicMaterial({ color: CENTERLINE_COLOR, depthTest: false, transparent: true, opacity: 0.5 });
  private guide = new THREE.Line(
    new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), new THREE.Vector3()]),
    new THREE.LineDashedMaterial({ color: ACTIVE_COLOR, dashSize: 1.5, gapSize: 1, depthTest: false }),
  );

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
    this.guide.visible = false;
    this.guide.renderOrder = 2;
    this.centerlineGroup.renderOrder = 1;
    this.scene.add(this.pointMarkers, this.arrow, this.influence, this.centerlineGroup, this.guide);

    new ResizeObserver(() => this.resize()).observe(container);
    this.resize();
    this.renderer.setAnimationLoop(() => {
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
    });
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
    const mesh = new THREE.Mesh(geometry, this.material);
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

  setControlPointsVisible(visible: boolean) {
    this.pointMarkers.visible = visible;
  }

  get centerlineCount() {
    let n = 0;
    for (const c of this.centerlines.values()) n += c.length;
    return n;
  }

  setCenterlines(shape: number, data: Float64Array) {
    const lines: Centerline[] = [];
    const v = (i: number) => new THREE.Vector3(data[i], data[i + 1], data[i + 2]);
    for (let o = 0; o + CENTERLINE_STRIDE <= data.length; o += CENTERLINE_STRIDE) {
      lines.push({ shape, target: data[o], radius: data[o + 1], anchor: v(o + 2), normal: v(o + 5), tip: v(o + 8) });
    }
    if (lines.length) this.centerlines.set(shape, lines);
    else this.centerlines.delete(shape);
    this.rebuildCenterlines(data, shape);
  }

  setCenterlinesVisible(visible: boolean) {
    this.centerlineGroup.visible = visible;
  }

  private rebuildCenterlines(data: Float64Array, shape: number) {
    for (const child of [...this.centerlineGroup.children]) {
      if (child.userData.shape === shape) {
        this.centerlineGroup.remove(child);
        if (child instanceof THREE.Line) child.geometry.dispose();
      }
    }
    for (let o = 0; o + CENTERLINE_STRIDE <= data.length; o += CENTERLINE_STRIDE) {
      const pts: THREE.Vector3[] = [];
      for (let i = 0; i < CENTERLINE_SAMPLES; i++) {
        const k = o + 11 + i * 3;
        pts.push(new THREE.Vector3(data[k], data[k + 1], data[k + 2]));
      }
      const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), this.lineMaterial);
      const tip = new THREE.Mesh(this.handleGeometry, this.handleMaterial);
      tip.position.copy(pts[pts.length - 1]);
      const root = new THREE.Mesh(this.handleGeometry, this.rootMaterial);
      root.position.copy(pts[0]);
      root.scale.setScalar(0.6);
      for (const obj of [line, tip, root]) {
        obj.userData.shape = shape;
        obj.renderOrder = 1;
        this.centerlineGroup.add(obj);
      }
    }
  }

  /** Nærmeste tupp av en senterlinje innenfor `maxPx` piksler fra pekeren. */
  pickCenterlineTip(clientX: number, clientY: number, maxPx: number): Centerline | null {
    if (!this.centerlineGroup.visible) return null;
    const rect = this.renderer.domElement.getBoundingClientRect();
    const mouse = new THREE.Vector2(clientX - rect.left, clientY - rect.top);
    let best: Centerline | null = null;
    let bestDist = maxPx;
    for (const lines of this.centerlines.values()) {
      for (const c of lines) {
        const d = this.toScreen(c.tip).distanceTo(mouse);
        if (d < bestDist) {
          best = c;
          bestDist = d;
        }
      }
    }
    return best;
  }

  /** Stiplet linje fra roten A til grepet B under et drag. */
  showGuide(from: THREE.Vector3, to: THREE.Vector3) {
    this.guide.geometry.setFromPoints([from, to]);
    this.guide.computeLineDistances();
    this.guide.visible = true;
  }

  hideGuide() {
    this.guide.visible = false;
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
    const all = [...this.points.values()].flat();
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
    for (const pts of this.points.values()) {
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
    const hit = this.raycaster.intersectObjects([...this.shapes.values()], false)[0];
    return !!hit && hit.distance < dist - 0.5;
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
