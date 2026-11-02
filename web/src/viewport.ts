import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { Projector } from './grip-math';
import type { MeshResult } from './protocol';

export interface ControlPoint {
  shape: number;
  position: THREE.Vector3;
  normal: THREE.Vector3;
}

const POINT_COLOR = 0x5b6178;
const ACTIVE_COLOR = 0x6d4fe6;
/** Utgangsvisningen: kameraet skrått ovenfra, rettet mot midt på en ny form. */
const HOME_POSITION = new THREE.Vector3(75, 70, 105);
const HOME_TARGET = new THREE.Vector3(0, 25, 0);

/**
 * Rutenett på gulvet (y = 0) som toner ut mot horisonten: 10 mm ruter og en
 * litt tydeligere linje hver 50 mm. Linjene tegnes med skjermderiverte, så
 * de er like tynne uansett avstand.
 */
function fadingGrid() {
  const material = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    side: THREE.DoubleSide,
    uniforms: {
      uColor: { value: new THREE.Color(0x8a94a8) },
      uCenter: { value: new THREE.Vector3() },
      uFade: { value: 400 },
    },
    vertexShader: `
      varying vec3 vWorld;
      void main() {
        vec4 w = modelMatrix * vec4(position, 1.0);
        vWorld = w.xyz;
        gl_Position = projectionMatrix * viewMatrix * w;
      }`,
    fragmentShader: `
      uniform vec3 uColor;
      uniform vec3 uCenter;
      uniform float uFade;
      varying vec3 vWorld;
      float line(vec2 p, float size) {
        vec2 q = p / size;
        vec2 w = fwidth(q);
        vec2 g = abs(fract(q - 0.5) - 0.5) / w;
        // Ruter som blir mindre enn noen få piksler, tones ut i stedet for å flimre.
        float lod = 1.0 - smoothstep(0.08, 0.3, max(w.x, w.y));
        return (1.0 - min(min(g.x, g.y), 1.0)) * lod;
      }
      void main() {
        float minor = line(vWorld.xz, 10.0);
        float major = line(vWorld.xz, 50.0);
        float fade = 1.0 - smoothstep(uFade * 0.35, uFade, length(vWorld.xz - uCenter.xz));
        float a = max(minor * 0.28, major * 0.5) * fade;
        if (a < 0.003) discard;
        gl_FragColor = vec4(uColor, a);
      }`,
  });
  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(8000, 8000).rotateX(-Math.PI / 2), material);
  mesh.renderOrder = -1;
  return mesh;
}

/** 3D-arbeidsflaten. Én scene-enhet er én millimeter. */
export class Viewport {
  readonly renderer: THREE.WebGLRenderer;
  readonly camera = new THREE.PerspectiveCamera(40, 1, 0.5, 5000);
  readonly controls: OrbitControls;
  private scene = new THREE.Scene();
  private shapes = new Map<number, THREE.Mesh>();
  /** Lys, matt «leire», som grunnformene i paletten. */
  private material = new THREE.MeshStandardMaterial({ color: 0xe6e7ee, roughness: 0.55, metalness: 0 });
  /** Låste lag tegnes litt nedtonet. */
  private lockedMaterial = new THREE.MeshStandardMaterial({ color: 0xc4c7d2, roughness: 0.8, transparent: true, opacity: 0.75 });
  /** Negative former tegnes gjennomsiktig, så målet de skjærer i synes. */
  private cutterMaterial = new THREE.MeshStandardMaterial({
    color: 0x7c5cf0,
    roughness: 0.5,
    transparent: true,
    opacity: 0.28,
    depthWrite: false,
    side: THREE.DoubleSide,
  });
  /** Skjulte og låste lag og negative former (kroppen har samme id som rotsplinen). */
  private hiddenShapes = new Set<number>();
  private lockedShapes = new Set<number>();
  private cutterShapes = new Set<number>();
  private points = new Map<number, ControlPoint[]>();
  private pointMarkers = new THREE.Group();
  private pointGeometry = new THREE.SphereGeometry(0.55, 10, 8);
  private pointMaterial = new THREE.MeshBasicMaterial({ color: POINT_COLOR });
  private arrow = new THREE.ArrowHelper(new THREE.Vector3(0, 1, 0), new THREE.Vector3(), 10, ACTIVE_COLOR, 3, 2);
  private influence: THREE.Mesh;
  private raycaster = new THREE.Raycaster();
  private frameCallbacks: (() => void)[] = [];
  private grid = fadingGrid();
  /** Myk skygge under formene på gulvet. */
  private shadowFloor = new THREE.Mesh(
    new THREE.PlaneGeometry(600, 600).rotateX(-Math.PI / 2),
    new THREE.ShadowMaterial({ opacity: 0.1, depthWrite: false }),
  );
  private sun = new THREE.DirectionalLight(0xffffff, 1.6);
  /** Pågående kameraovergang (zoom, tilpass, visningskuben). */
  private flight: { from: THREE.Vector3; to: THREE.Vector3; fromTarget: THREE.Vector3; toTarget: THREE.Vector3; up: THREE.Vector3 | null; start: number; ms: number } | null = null;
  /** Under konturredigering tar ikke formgrep og skjelett imot pekeren. */
  interactionLocked = false;
  constructor(private container: HTMLElement) {
    // Gjennomsiktig lerret: bakgrunnen er en CSS-gradient bak scenen.
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.setClearColor(0x000000, 0);
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    // Skyggen tegnes bare på nytt når formene endres, ikke for hvert bilde.
    this.renderer.shadowMap.autoUpdate = false;
    container.prepend(this.renderer.domElement);

    this.camera.position.copy(HOME_POSITION);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.target.copy(HOME_TARGET);
    this.controls.enableDamping = true;
    // Brukeren tar over kameraet: en pågående overgang stoppes.
    this.controls.addEventListener('start', () => (this.flight = null));

    this.scene.add(this.grid);
    this.shadowFloor.receiveShadow = true;
    this.shadowFloor.position.y = -0.05;
    this.scene.add(this.shadowFloor);
    // Himmellys, sollys med skygge og et svakt motlys gir myk «leire» uten tunge miljøkart.
    this.scene.add(new THREE.HemisphereLight(0xffffff, 0xaeb7ca, 2.1));
    const fill = new THREE.DirectionalLight(0xdfe6ff, 0.5);
    fill.position.set(-120, 60, -80);
    this.scene.add(fill);
    this.sun.position.set(60, 180, 90);
    this.sun.castShadow = true;
    this.sun.shadow.mapSize.set(1024, 1024);
    this.sun.shadow.radius = 6;
    this.sun.shadow.bias = -0.0005;
    const sc = this.sun.shadow.camera;
    sc.left = sc.bottom = -160;
    sc.right = sc.top = 160;
    sc.near = 10;
    sc.far = 500;
    this.scene.add(this.sun, this.sun.target);

    this.influence = new THREE.Mesh(
      new THREE.RingGeometry(0.96, 1, 64),
      new THREE.MeshBasicMaterial({ color: ACTIVE_COLOR, side: THREE.DoubleSide, transparent: true, opacity: 0.7, depthTest: false }),
    );
    this.arrow.visible = false;
    this.influence.visible = false;
    this.scene.add(this.pointMarkers, this.arrow, this.influence);

    new ResizeObserver(() => this.resize()).observe(container);
    this.resize();
    this.renderer.setAnimationLoop((time) => {
      this.fly(time);
      this.controls.update();
      this.grid.material.uniforms.uCenter.value.copy(this.controls.target);
      this.grid.material.uniforms.uFade.value = Math.max(300, this.camera.position.distanceTo(this.controls.target) * 3);
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
    if (!result) {
      this.renderer.shadowMap.needsUpdate = true;
      return 0;
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(result.positions, 3));
    geometry.setAttribute('normal', new THREE.BufferAttribute(result.normals, 3));
    geometry.setIndex(new THREE.BufferAttribute(result.indices, 1));
    geometry.computeBoundingSphere();
    const mesh = new THREE.Mesh(geometry, this.materialFor(shape));
    mesh.castShadow = !this.cutterShapes.has(shape);
    mesh.visible = !this.hiddenShapes.has(shape);
    mesh.userData.shape = shape;
    this.shapes.set(shape, mesh);
    this.scene.add(mesh);
    this.renderer.shadowMap.needsUpdate = true;
    return result.indices.length / 3;
  }

  /** Meshene som skal skrives ut: synlige former, uten negative former (de er allerede trukket fra). */
  printableGeometries() {
    return [...this.shapes].filter(([id, m]) => m.visible && !this.cutterShapes.has(id)).map(([, m]) => m.geometry);
  }

  /** Antall trekanter i kroppen og om den vises (for testene). */
  shapeInfo(shape: number) {
    const m = this.shapes.get(shape);
    return { triangles: m ? (m.geometry.index?.count ?? 0) / 3 : 0, visible: !!m?.visible };
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
  setShapeStates(hidden: Set<number>, locked: Set<number>, cutters: Set<number> = this.cutterShapes) {
    this.hiddenShapes = hidden;
    this.lockedShapes = locked;
    this.cutterShapes = cutters;
    for (const [id, mesh] of this.shapes) {
      mesh.visible = !hidden.has(id);
      mesh.material = this.materialFor(id);
      mesh.renderOrder = cutters.has(id) ? 1 : 0;
      mesh.castShadow = !cutters.has(id);
    }
    this.renderer.shadowMap.needsUpdate = true;
    this.rebuildPointMarkers();
  }

  private materialFor(shape: number) {
    if (this.cutterShapes.has(shape)) return this.cutterMaterial;
    return this.lockedShapes.has(shape) ? this.lockedMaterial : this.material;
  }

  private active(shape: number) {
    return !this.hiddenShapes.has(shape) && !this.lockedShapes.has(shape);
  }

  /** Formen har kontrollpunkter. En negativ form formes med skjelettet,
   * ikke med prikkene, siden kuttet følger splinene. */
  private hasPoints(shape: number) {
    return this.active(shape) && !this.cutterShapes.has(shape);
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
    const all = [...this.points].filter(([id]) => this.hasPoints(id)).flatMap(([, pts]) => pts);
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
      if (!this.hasPoints(shape)) continue;
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
    const visible = [...this.shapes.values()].filter((m) => m.visible && !this.cutterShapes.has(m.userData.shape as number));
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

  /** Punktet under pekeren i planet gjennom `origin` vinkelrett på synsretningen. */
  pointInViewPlane(clientX: number, clientY: number, origin: THREE.Vector3): THREE.Vector3 | null {
    this.setRay(clientX, clientY);
    const normal = this.camera.getWorldDirection(new THREE.Vector3());
    const plane = new THREE.Plane().setFromNormalAndCoplanarPoint(normal, origin);
    return this.raycaster.ray.intersectPlane(plane, new THREE.Vector3());
  }

  /**
   * Snittvisning for konturredigering: delen av modellen mellom `plane` og
   * kameraet skjæres bort, og resten tegnes nedtonet og delvis gjennomsiktig
   * som referanse. `null` gir vanlig visning.
   */
  setSection(plane: THREE.Plane | null) {
    this.renderer.localClippingEnabled = !!plane;
    for (const m of [this.material, this.lockedMaterial]) {
      m.clippingPlanes = plane ? [plane] : [];
      m.side = plane ? THREE.DoubleSide : THREE.FrontSide;
      m.transparent = !!plane || m === this.lockedMaterial;
      m.opacity = plane ? 0.45 : m === this.lockedMaterial ? 0.75 : 1;
      m.depthWrite = !plane;
      m.needsUpdate = true;
    }
    this.pointMarkers.visible = !plane;
  }

  /** Kameraets plassering, mål og opp-retning, for å kunne sette det tilbake. */
  cameraState() {
    return { position: this.camera.position.clone(), target: this.controls.target.clone(), up: this.camera.up.clone() };
  }

  setCamera(state: { position: THREE.Vector3; target: THREE.Vector3; up: THREE.Vector3 }) {
    this.camera.up.copy(state.up);
    this.camera.position.copy(state.position);
    this.controls.target.copy(state.target);
    this.camera.lookAt(state.target);
    this.controls.update();
  }

  /** Avstanden fra kameraet som gir en synlig høyde på `height` mm. */
  distanceForHeight(height: number) {
    return height / (2 * Math.tan(THREE.MathUtils.degToRad(this.camera.fov / 2)));
  }

  /** Flytter kameraet tilbake til utgangsvisningen. */
  resetView() {
    this.camera.up.set(0, 1, 0);
    this.camera.position.copy(HOME_POSITION);
    this.controls.target.copy(HOME_TARGET);
  }

  /** Flyr kameraet jevnt til en ny plassering. */
  flyTo(position: THREE.Vector3, target: THREE.Vector3, up: THREE.Vector3 | null = null, ms = 320) {
    this.flight = {
      from: this.camera.position.clone(),
      to: position.clone(),
      fromTarget: this.controls.target.clone(),
      toTarget: target.clone(),
      up,
      start: performance.now(),
      ms,
    };
  }

  private fly(time: number) {
    const f = this.flight;
    if (!f) return;
    const t = Math.min(1, (time - f.start) / f.ms);
    const k = t < 0 ? 0 : 1 - (1 - t) ** 3;
    this.camera.position.lerpVectors(f.from, f.to, k);
    this.controls.target.lerpVectors(f.fromTarget, f.toTarget, k);
    if (t >= 1) {
      if (f.up) this.camera.up.copy(f.up);
      this.flight = null;
    }
  }

  /** Zoomer mot midten av visningen; `factor` < 1 går nærmere. */
  zoom(factor: number) {
    const target = this.controls.target;
    const offset = this.camera.position.clone().sub(target).multiplyScalar(factor);
    const dist = THREE.MathUtils.clamp(offset.length(), 8, 3000);
    this.flyTo(target.clone().add(offset.setLength(dist)), target, null, 220);
  }

  /** Rammer inn alle synlige former, eller går til utgangsvisningen når det ikke er noen. */
  fitAll() {
    const box = new THREE.Box3();
    for (const m of this.shapes.values()) if (m.visible) box.expandByObject(m);
    if (box.isEmpty()) {
      this.camera.up.set(0, 1, 0);
      return this.flyTo(HOME_POSITION, HOME_TARGET);
    }
    const sphere = box.getBoundingSphere(new THREE.Sphere());
    const fov = THREE.MathUtils.degToRad(this.camera.fov / 2);
    const fit = Math.min(fov, Math.atan(Math.tan(fov) * this.camera.aspect));
    const dist = (sphere.radius * 1.35) / Math.sin(fit);
    const dir = this.camera.position.clone().sub(this.controls.target).normalize();
    this.flyTo(sphere.center.clone().addScaledVector(dir, dist), sphere.center);
  }

  /** Ser rett langs en akse (fra visningskuben). Avstanden til målet beholdes. */
  viewFrom(direction: THREE.Vector3) {
    const dir = direction.clone().normalize();
    const dist = this.camera.position.distanceTo(this.controls.target);
    // Rett ovenfra eller nedenfra trenger en annen opp-retning enn y.
    const up = Math.abs(dir.y) > 0.99 ? new THREE.Vector3(0, 0, dir.y > 0 ? -1 : 1) : new THREE.Vector3(0, 1, 0);
    this.camera.up.copy(up);
    this.flyTo(this.controls.target.clone().addScaledVector(dir, dist), this.controls.target, up);
  }

  /** Trådnett over hele modellen. */
  setWireframe(on: boolean) {
    for (const m of [this.material, this.lockedMaterial, this.cutterMaterial]) {
      m.wireframe = on;
      m.needsUpdate = true;
    }
  }

  /** Farger som følger lyst og mørkt tema. */
  setTheme(dark: boolean) {
    this.grid.material.uniforms.uColor.value.set(dark ? 0x8b93a8 : 0x8a94a8);
    (this.shadowFloor.material as THREE.ShadowMaterial).opacity = dark ? 0.3 : 0.1;
    this.material.color.set(dark ? 0xd9dae3 : 0xe6e7ee);
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
