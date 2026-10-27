import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { MeshResult } from './protocol';

/** 3D-arbeidsflaten. Én scene-enhet er én millimeter. */
export class Viewport {
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera = new THREE.PerspectiveCamera(40, 1, 0.5, 5000);
  private controls: OrbitControls;
  private shapes = new THREE.Group();
  private material = new THREE.MeshStandardMaterial({ color: 0xd9c7a8, roughness: 0.65 });

  constructor(private container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    container.prepend(this.renderer.domElement);

    this.scene.background = new THREE.Color(0xf4f5f2);
    this.camera.position.set(120, 90, 160);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;

    // Rutenett på 200 × 200 mm med 10 mm ruter, liggende i XZ-planet.
    const grid = new THREE.GridHelper(200, 20, 0xb9bdb3, 0xdcdfd8);
    this.scene.add(grid);
    this.scene.add(new THREE.HemisphereLight(0xffffff, 0xc8c2b4, 1.6));
    const sun = new THREE.DirectionalLight(0xffffff, 1.4);
    sun.position.set(80, 160, 120);
    this.scene.add(sun);
    this.scene.add(this.shapes);

    new ResizeObserver(() => this.resize()).observe(container);
    this.resize();
    this.renderer.setAnimationLoop(() => {
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
    });
  }

  get shapeCount() {
    return this.shapes.children.length;
  }

  addMesh(result: MeshResult, positionMm: THREE.Vector3Like) {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(result.positions, 3));
    geometry.setAttribute('normal', new THREE.BufferAttribute(result.normals, 3));
    geometry.setIndex(new THREE.BufferAttribute(result.indices, 1));
    const mesh = new THREE.Mesh(geometry, this.material);
    mesh.position.copy(positionMm);
    this.shapes.add(mesh);
    return geometry.index!.count / 3;
  }

  private resize() {
    const { clientWidth: w, clientHeight: h } = this.container;
    if (w === 0 || h === 0) return;
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }
}
