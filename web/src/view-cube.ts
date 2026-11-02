import * as THREE from 'three';
import { RoundedBoxGeometry } from 'three/examples/jsm/geometries/RoundedBoxGeometry.js';
import type { Viewport } from './viewport';

const AXES: { label: string; dir: THREE.Vector3; color: string }[] = [
  { label: 'X', dir: new THREE.Vector3(1, 0, 0), color: '#e5484d' },
  { label: 'Y', dir: new THREE.Vector3(0, 1, 0), color: '#30a46c' },
  { label: 'Z', dir: new THREE.Vector3(0, 0, 1), color: '#3e63dd' },
];

function labelSprite(text: string, color: string) {
  const c = document.createElement('canvas');
  c.width = c.height = 64;
  const g = c.getContext('2d')!;
  g.fillStyle = color;
  g.font = '600 40px Inter, system-ui, sans-serif';
  g.textAlign = 'center';
  g.textBaseline = 'middle';
  g.fillText(text, 32, 34);
  const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: new THREE.CanvasTexture(c), depthTest: false }));
  sprite.scale.setScalar(0.5);
  sprite.renderOrder = 2;
  return sprite;
}

/**
 * Visningskuben øverst til høyre: en liten kube med aksene X, Y og Z som
 * dreier med kameraet. Trykk på en side eller en akse ser rett mot den
 * siden; hjørner og kanter gir skrå visninger.
 */
export class ViewCube {
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera = new THREE.OrthographicCamera(-1.6, 1.6, 1.6, -1.6, 0.1, 20);
  private cube: THREE.Mesh;
  private labels: THREE.Sprite[] = [];
  private raycaster = new THREE.Raycaster();

  constructor(
    private viewport: Viewport,
    private el: HTMLElement,
  ) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    el.append(this.renderer.domElement);

    this.cube = new THREE.Mesh(
      new RoundedBoxGeometry(1, 1, 1, 3, 0.12),
      new THREE.MeshStandardMaterial({ color: 0xf6f7fb, roughness: 0.6 }),
    );
    this.scene.add(this.cube);
    this.scene.add(new THREE.HemisphereLight(0xffffff, 0xaab3c5, 2.2));
    const key = new THREE.DirectionalLight(0xffffff, 1.2);
    key.position.set(2, 4, 3);
    this.scene.add(key);

    for (const a of AXES) {
      const end = a.dir.clone().multiplyScalar(1.05);
      const line = new THREE.Line(
        new THREE.BufferGeometry().setFromPoints([a.dir.clone().multiplyScalar(0.5), end]),
        new THREE.LineBasicMaterial({ color: a.color }),
      );
      this.scene.add(line);
      const sprite = labelSprite(a.label, a.color);
      sprite.position.copy(a.dir.clone().multiplyScalar(1.28));
      sprite.userData.dir = a.dir;
      this.labels.push(sprite);
      this.scene.add(sprite);
    }

    el.addEventListener('click', (e) => this.onClick(e));
    viewport.onFrame(() => this.render());
  }

  /** Kameraet ved forrige tegning; kuben tegnes bare når det har endret seg. */
  private last = new THREE.Matrix4();

  private render() {
    const { clientWidth: w, clientHeight: h } = this.el;
    if (!w || !h) return;
    const size = this.renderer.getSize(new THREE.Vector2());
    const resized = size.x !== w || size.y !== h;
    if (resized) this.renderer.setSize(w, h, false);
    const main = this.viewport.camera;
    if (!resized && main.matrixWorld.equals(this.last)) return;
    this.last.copy(main.matrixWorld);
    const dir = main.position.clone().sub(this.viewport.controls.target).normalize();
    this.camera.position.copy(dir.multiplyScalar(5));
    this.camera.up.copy(main.up);
    this.camera.lookAt(0, 0, 0);
    this.renderer.render(this.scene, this.camera);
  }

  /** Skjermposisjonen (i kubens element) til midten av en side, for testene. */
  faceScreen(axis: 'x' | 'y' | 'z') {
    const p = new THREE.Vector3(axis === 'x' ? 0.5 : 0, axis === 'y' ? 0.5 : 0, axis === 'z' ? 0.5 : 0).project(this.camera);
    return { x: ((p.x + 1) / 2) * this.el.clientWidth, y: ((1 - p.y) / 2) * this.el.clientHeight };
  }

  private onClick(e: MouseEvent) {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    const label = this.raycaster.intersectObjects(this.labels)[0];
    if (label) return this.viewport.viewFrom(label.object.userData.dir as THREE.Vector3);
    const hit = this.raycaster.intersectObject(this.cube)[0];
    if (!hit) return;
    // Nær midten av en side gir rett visning; nær kanter og hjørner gir skrå visning.
    const p = hit.point;
    const dir = new THREE.Vector3(
      Math.abs(p.x) > 0.36 ? Math.sign(p.x) : 0,
      Math.abs(p.y) > 0.36 ? Math.sign(p.y) : 0,
      Math.abs(p.z) > 0.36 ? Math.sign(p.z) : 0,
    );
    if (dir.lengthSq() > 0) this.viewport.viewFrom(dir);
  }
}
