import * as THREE from 'three';
import { rand, randInt } from './utils.js';
import { LANE_HALF_WIDTH, LANE_HALF_HEIGHT } from './player.js';

const SPAWN_AHEAD = 140; // distance ahead of player (in world z) to spawn content
const DESPAWN_BEHIND = 10;

function makeAsteroidGeometries() {
  const geos = [];
  for (let i = 0; i < 4; i++) {
    const geo = new THREE.IcosahedronGeometry(1, 1);
    const pos = geo.attributes.position;
    for (let v = 0; v < pos.count; v++) {
      const n = 1 + (Math.random() - 0.5) * 0.5;
      pos.setXYZ(v, pos.getX(v) * n, pos.getY(v) * n, pos.getZ(v) * n);
    }
    geo.computeVertexNormals();
    geos.push(geo);
  }
  return geos;
}

/**
 * Procedurally scrolling asteroid field. Obstacles are generated in chunks
 * as the player advances and recycled once they fall behind the camera —
 * an endless corridor without ever allocating unbounded geometry.
 */
export class World {
  constructor(scene) {
    this.scene = scene;
    this.asteroids = []; // { mesh, radius, angVel, alive }
    this.nextSpawnZ = 30;
    this.geometries = makeAsteroidGeometries();
    this.material = new THREE.MeshStandardMaterial({
      color: 0x8f8fa8,
      roughness: 0.85,
      metalness: 0.15,
      flatShading: true,
    });
    this.materialHot = new THREE.MeshStandardMaterial({
      color: 0xff6a3d,
      emissive: 0xff3d1a,
      emissiveIntensity: 0.6,
      roughness: 0.7,
      flatShading: true,
    });
    this.pool = [];
  }

  _obtainMesh(hot) {
    const reusable = this.pool.pop();
    const geo = this.geometries[randInt(0, this.geometries.length - 1)];
    if (reusable) {
      reusable.geometry = geo;
      reusable.material = hot ? this.materialHot : this.material;
      reusable.visible = true;
      return reusable;
    }
    const mesh = new THREE.Mesh(geo, hot ? this.materialHot : this.material);
    mesh.castShadow = false;
    this.scene.add(mesh);
    return mesh;
  }

  spawnChunk(centerZ, difficulty) {
    const count = randInt(5 + Math.floor(difficulty), 9 + Math.floor(difficulty * 1.5));
    for (let i = 0; i < count; i++) {
      const hot = Math.random() < Math.min(0.05 + difficulty * 0.01, 0.25);
      const mesh = this._obtainMesh(hot);
      const scale = rand(0.5, 1.5 + Math.min(difficulty * 0.05, 1));
      mesh.scale.setScalar(scale);
      mesh.position.set(
        rand(-LANE_HALF_WIDTH * 1.15, LANE_HALF_WIDTH * 1.15),
        rand(-LANE_HALF_HEIGHT * 1.1, LANE_HALF_HEIGHT * 1.1),
        -(centerZ + rand(0, 26)),
      );
      mesh.rotation.set(rand(0, Math.PI * 2), rand(0, Math.PI * 2), rand(0, Math.PI * 2));
      const angVel = new THREE.Vector3(rand(-1, 1), rand(-1, 1), rand(-1, 1));
      this.asteroids.push({
        mesh,
        radius: scale * 0.85,
        angVel,
        hot,
        alive: true,
      });
    }
  }

  update(dt, playerZ, difficulty) {
    while (this.nextSpawnZ < playerZ + SPAWN_AHEAD) {
      this.spawnChunk(this.nextSpawnZ, difficulty);
      this.nextSpawnZ += rand(16, 24) / (1 + difficulty * 0.08);
    }
    for (let i = this.asteroids.length - 1; i >= 0; i--) {
      const a = this.asteroids[i];
      a.mesh.rotation.x += a.angVel.x * dt * 0.4;
      a.mesh.rotation.y += a.angVel.y * dt * 0.4;
      a.mesh.rotation.z += a.angVel.z * dt * 0.4;
      const worldZ = -a.mesh.position.z;
      if (worldZ < playerZ - DESPAWN_BEHIND) {
        this.scene.remove(a.mesh);
        this.pool.push(a.mesh);
        this.asteroids.splice(i, 1);
      }
    }
  }

  /** Returns nearby asteroids within a z-window for cheap collision checks. */
  nearby(playerZ, window = 6) {
    return this.asteroids.filter((a) => {
      const worldZ = -a.mesh.position.z;
      return Math.abs(worldZ - playerZ) < window;
    });
  }

  destroyAsteroid(a) {
    a.alive = false;
    const idx = this.asteroids.indexOf(a);
    if (idx >= 0) this.asteroids.splice(idx, 1);
    this.scene.remove(a.mesh);
    this.pool.push(a.mesh);
  }

  reset() {
    for (const a of this.asteroids) {
      this.scene.remove(a.mesh);
      this.pool.push(a.mesh);
    }
    this.asteroids = [];
    this.nextSpawnZ = 30;
  }
}
