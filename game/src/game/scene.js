import * as THREE from 'three';

/**
 * Builds the renderer, camera, lighting rig, and a layered deep-space
 * backdrop (starfield + drifting nebula sprites) used as the world's
 * permanent background — none of this scrolls with gameplay geometry,
 * it just sits far behind the play corridor to sell depth.
 */
export function createScene(canvas) {
  const renderer = new THREE.WebGLRenderer({
    canvas,
    antialias: true,
    alpha: false,
    powerPreference: 'high-performance',
  });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setSize(window.innerWidth, window.innerHeight);
  renderer.setClearColor(0x05030f, 1);
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.15;

  const scene = new THREE.Scene();
  scene.fog = new THREE.FogExp2(0x05030f, 0.011);

  const camera = new THREE.PerspectiveCamera(
    68,
    window.innerWidth / window.innerHeight,
    0.1,
    600,
  );
  camera.position.set(0, 1.4, 8);
  camera.lookAt(0, 0, -20);

  // --- lighting rig -----------------------------------------------------
  const hemi = new THREE.HemisphereLight(0x6a8bff, 0x100822, 0.65);
  scene.add(hemi);

  const key = new THREE.DirectionalLight(0xbfe0ff, 1.1);
  key.position.set(6, 10, 4);
  scene.add(key);

  const rim = new THREE.PointLight(0xff3d9a, 2.2, 60, 2);
  rim.position.set(-4, 2, -6);
  scene.add(rim);

  const engineGlow = new THREE.PointLight(0x4bf5ff, 1.6, 12, 2);
  engineGlow.position.set(0, 0.2, 9);
  scene.add(engineGlow);

  // --- starfield ----------------------------------------------------------
  const starGeo = new THREE.BufferGeometry();
  const STAR_COUNT = 2600;
  const positions = new Float32Array(STAR_COUNT * 3);
  const colors = new Float32Array(STAR_COUNT * 3);
  const palette = [
    new THREE.Color(0xffffff),
    new THREE.Color(0x9fd8ff),
    new THREE.Color(0xffd6f0),
    new THREE.Color(0xd6c2ff),
  ];
  for (let i = 0; i < STAR_COUNT; i++) {
    const radius = THREE.MathUtils.randFloat(60, 320);
    const theta = Math.random() * Math.PI * 2;
    const phi = Math.acos(THREE.MathUtils.randFloatSpread(2));
    positions[i * 3] = radius * Math.sin(phi) * Math.cos(theta);
    positions[i * 3 + 1] = radius * Math.sin(phi) * Math.sin(theta);
    positions[i * 3 + 2] = -Math.abs(radius * Math.cos(phi)) - 40;
    const c = palette[i % palette.length];
    colors[i * 3] = c.r;
    colors[i * 3 + 1] = c.g;
    colors[i * 3 + 2] = c.b;
  }
  starGeo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  starGeo.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  const starMat = new THREE.PointsMaterial({
    size: 1.4,
    vertexColors: true,
    transparent: true,
    opacity: 0.9,
    sizeAttenuation: true,
    depthWrite: false,
  });
  const stars = new THREE.Points(starGeo, starMat);
  scene.add(stars);

  // --- nebula backdrop: soft additive sprites via canvas texture ---------
  const nebulaTexture = makeNebulaTexture();
  const nebulaGroup = new THREE.Group();
  const nebulaColors = [0x4bf5ff, 0xff3d9a, 0x9d4bff, 0x2ad4ff];
  for (let i = 0; i < 10; i++) {
    const mat = new THREE.SpriteMaterial({
      map: nebulaTexture,
      color: nebulaColors[i % nebulaColors.length],
      transparent: true,
      opacity: THREE.MathUtils.randFloat(0.08, 0.18),
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    const sprite = new THREE.Sprite(mat);
    const scale = THREE.MathUtils.randFloat(80, 220);
    sprite.scale.set(scale, scale, 1);
    sprite.position.set(
      THREE.MathUtils.randFloatSpread(260),
      THREE.MathUtils.randFloatSpread(140),
      -THREE.MathUtils.randFloat(120, 280),
    );
    nebulaGroup.add(sprite);
  }
  scene.add(nebulaGroup);

  function makeNebulaTexture() {
    const size = 256;
    const cnv = document.createElement('canvas');
    cnv.width = cnv.height = size;
    const ctx = cnv.getContext('2d');
    const grad = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
    grad.addColorStop(0, 'rgba(255,255,255,1)');
    grad.addColorStop(0.35, 'rgba(255,255,255,0.55)');
    grad.addColorStop(1, 'rgba(255,255,255,0)');
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, size, size);
    const tex = new THREE.CanvasTexture(cnv);
    tex.colorSpace = THREE.SRGBColorSpace;
    return tex;
  }

  function resize() {
    const w = window.innerWidth;
    const h = window.innerHeight;
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    renderer.setSize(w, h);
  }
  window.addEventListener('resize', resize);

  function tickBackdrop(dt, playerZ) {
    stars.rotation.z += dt * 0.003;
    nebulaGroup.rotation.y += dt * 0.006;
    // keep the backdrop anchored relative to the ship so it never scrolls out
    stars.position.z = playerZ;
    nebulaGroup.position.z = playerZ;
  }

  return { renderer, scene, camera, engineGlow, rim, tickBackdrop, resize };
}
