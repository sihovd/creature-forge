import * as THREE from 'three';
import { RoundedBoxGeometry } from 'three/examples/jsm/geometries/RoundedBoxGeometry.js';
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js';
import type { PrimitiveKind } from './primitives';

/** Formene slik de ser ut i paletten (bare forhåndsvisning; formene lages av kjernen). */
function geometry(kind: PrimitiveKind): THREE.BufferGeometry {
  switch (kind) {
    case 'sphere':
      return new THREE.SphereGeometry(1, 64, 48).translate(0, 1, 0);
    case 'capsule':
      return new THREE.CapsuleGeometry(0.62, 0.9, 16, 48).translate(0, 1.07, 0);
    case 'cone':
      return new THREE.ConeGeometry(1, 1.9, 64, 1).translate(0, 0.95, 0);
    case 'box':
      return new RoundedBoxGeometry(1.6, 1.6, 1.6, 4, 0.08).rotateY(Math.PI / 5).translate(0, 0.8, 0);
  }
}

/**
 * Tegner små bilder av grunnformene i hvit «leire» med myk skygge, slik
 * skissen viser dem. Brukes av scripts/thumbnails.mjs, som lagrer bildene i
 * public/shapes/, så appen slipper å tegne dem ved oppstart.
 */
export function renderThumbnails(kinds: PrimitiveKind[], width = 280, height = 200): Map<PrimitiveKind, string> {
  const out = new Map<PrimitiveKind, string>();
  let renderer: THREE.WebGLRenderer;
  try {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, preserveDrawingBuffer: true });
  } catch {
    return out;
  }
  renderer.setSize(width, height, false);
  renderer.setClearColor(0x000000, 0);
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environmentIntensity = 0.6;
  scene.add(new THREE.HemisphereLight(0xffffff, 0xb9c2d4, 0.8));
  const sun = new THREE.DirectionalLight(0xffffff, 1.3);
  sun.position.set(2.5, 6, 3);
  sun.castShadow = true;
  sun.shadow.mapSize.set(512, 512);
  sun.shadow.radius = 8;
  Object.assign(sun.shadow.camera, { left: -3, right: 3, top: 3, bottom: -3 });
  scene.add(sun);
  const floor = new THREE.Mesh(new THREE.PlaneGeometry(20, 20).rotateX(-Math.PI / 2), new THREE.ShadowMaterial({ opacity: 0.16 }));
  floor.receiveShadow = true;
  scene.add(floor);

  const camera = new THREE.PerspectiveCamera(30, width / height, 0.1, 50);
  camera.position.set(2.8, 2.6, 6.4);
  camera.lookAt(0, 0.9, 0);
  const material = new THREE.MeshStandardMaterial({ color: 0xe9eaf0, roughness: 0.5 });

  for (const kind of kinds) {
    const mesh = new THREE.Mesh(geometry(kind), material);
    mesh.castShadow = true;
    scene.add(mesh);
    renderer.render(scene, camera);
    out.set(kind, renderer.domElement.toDataURL('image/png'));
    scene.remove(mesh);
    mesh.geometry.dispose();
  }
  pmrem.dispose();
  material.dispose();
  renderer.dispose();
  renderer.forceContextLoss();
  return out;
}
