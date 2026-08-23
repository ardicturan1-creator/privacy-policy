import * as THREE from 'three';
import { rand } from './utils.js';
import { LANE_HALF_WIDTH, LANE_HALF_HEIGHT } from './player.js';

const TYPES = [
  { id: 'shield', color: 0x6a8bff, weight: 3 },
  { id: 'repair', color: 0x4bff88, weight: 2 },
  { id: 'multishot', color: 0xff3d9a, weight: 2 },
  { id: 'rapid', color: 0xffe14b, weight: 2 },
  { id: 'boostfull', color: 0xffb84b, weight: 2 },
];

function pickType() {
  const total = TYPES.reduce((s, t) => s + t.weight, 0);
  let r = Math.random() * total;
  for (const t of TYPES) {
    if (r < t.weight) return t;
    r -= t.weight;
  }
  return TYPES[0];
}

function buildPickupMesh(color) {
  const group = new THREE.Group();
  const geo = new THREE.OctahedronGeometry(0.4, 0);
  const mat = new THREE.MeshStandardMaterial({
    color,
    emissive: color,
    emissiveIntensity: 1.2,
    metalness: 0.3,
    roughness: 0.2,
  });
  const core = new THREE.Mesh(geo, mat);
  group.add(core);
  const haloMat = new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.25, side: THREE.DoubleSide });
  const halo = new THREE.Mesh(new THREE.RingGeometry(0.55, 0.68, 20), haloMat);
  group.add(halo);
  group.userData.core = core;
  group.userData.halo = halo;
  return group;
}

export class Powerups {
  constructor(scene, particles, audio) {
    this.scene = scene;
    this.particles = particles;
    this.audio = audio;
    this.items = []; // { mesh, type, radius }
    this.nextSpawnZ = 45;
  }

  spawn(centerZ) {
    const type = pickType();
    const mesh = buildPickupMesh(type.color);
    mesh.position.set(
      rand(-LANE_HALF_WIDTH * 0.85, LANE_HALF_WIDTH * 0.85),
      rand(-LANE_HALF_HEIGHT * 0.7, LANE_HALF_HEIGHT * 0.7),
      -centerZ,
    );
    this.scene.add(mesh);
    this.items.push({ mesh, type: type.id, radius: 0.65 });
  }

  update(dt, playerZ, player) {
    while (this.nextSpawnZ < playerZ + 90) {
      this.spawn(this.nextSpawnZ);
      this.nextSpawnZ += rand(26, 40);
    }
    for (let i = this.items.length - 1; i >= 0; i--) {
      const p = this.items[i];
      p.mesh.userData.core.rotation.y += dt * 2.2;
      p.mesh.userData.core.rotation.x += dt * 1.1;
      p.mesh.userData.halo.rotation.z += dt * 0.8;
      p.mesh.position.y += Math.sin((playerZ + i) * 0.5) * 0.0; // static bob handled by rotation only

      const worldZ = -p.mesh.position.z;
      if (worldZ < playerZ - 8) {
        this.scene.remove(p.mesh);
        this.items.splice(i, 1);
        continue;
      }
      if (p.mesh.position.distanceTo(player.mesh.position) < p.radius + player.radius) {
        player.applyPowerup(p.type);
        this.particles.burstExplosion(p.mesh.position, { count: 14, color: p.mesh.userData.core.material.color.getHex(), speed: 3, scale: 0.5 });
        this.audio.powerup();
        this.scene.remove(p.mesh);
        this.items.splice(i, 1);
      }
    }
  }

  reset() {
    for (const p of this.items) this.scene.remove(p.mesh);
    this.items = [];
    this.nextSpawnZ = 45;
  }
}
