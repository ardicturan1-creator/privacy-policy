import * as THREE from 'three';
import { rand, randInt } from './utils.js';
import { LANE_HALF_WIDTH, LANE_HALF_HEIGHT } from './player.js';

function buildDroneMesh() {
  const group = new THREE.Group();
  const bodyMat = new THREE.MeshStandardMaterial({ color: 0x2a2140, metalness: 0.6, roughness: 0.35 });
  const eyeMat = new THREE.MeshStandardMaterial({ color: 0xff3d5a, emissive: 0xff2050, emissiveIntensity: 2 });

  const core = new THREE.Mesh(new THREE.OctahedronGeometry(0.5, 0), bodyMat);
  group.add(core);
  const eye = new THREE.Mesh(new THREE.SphereGeometry(0.16, 10, 10), eyeMat);
  eye.position.z = 0.42;
  group.add(eye);
  const ringGeo = new THREE.TorusGeometry(0.62, 0.05, 8, 16);
  const ring = new THREE.Mesh(ringGeo, eyeMat);
  ring.rotation.x = Math.PI / 2;
  group.add(ring);
  return group;
}

function buildBossMesh() {
  const group = new THREE.Group();
  const shellMat = new THREE.MeshStandardMaterial({ color: 0x3a2050, metalness: 0.7, roughness: 0.3 });
  const coreMat = new THREE.MeshStandardMaterial({ color: 0xff3d9a, emissive: 0xff1a70, emissiveIntensity: 2.2 });

  const body = new THREE.Mesh(new THREE.DodecahedronGeometry(1.6, 0), shellMat);
  group.add(body);
  const core = new THREE.Mesh(new THREE.IcosahedronGeometry(0.6, 0), coreMat);
  group.add(core);
  for (let i = 0; i < 6; i++) {
    const spike = new THREE.Mesh(new THREE.ConeGeometry(0.22, 1, 5), shellMat);
    const angle = (i / 6) * Math.PI * 2;
    spike.position.set(Math.cos(angle) * 1.7, Math.sin(angle) * 1.7, 0);
    spike.rotation.z = angle + Math.PI / 2;
    group.add(spike);
  }
  group.userData.core = core;
  return group;
}

export class Enemies {
  constructor(scene, particles, audio, combat) {
    this.scene = scene;
    this.particles = particles;
    this.audio = audio;
    this.combat = combat;
    this.list = []; // { mesh, hp, maxHp, radius, kind, fireTimer, alive, phase }
    this.nextSpawnZ = 60;
    this.boss = null;
  }

  spawnDrone(centerZ, difficulty) {
    const mesh = buildDroneMesh();
    this.scene.add(mesh);
    mesh.position.set(
      rand(-LANE_HALF_WIDTH, LANE_HALF_WIDTH),
      rand(-LANE_HALF_HEIGHT * 0.8, LANE_HALF_HEIGHT * 0.8),
      -centerZ,
    );
    const hp = 30 + difficulty * 4;
    this.list.push({
      mesh,
      hp,
      maxHp: hp,
      radius: 0.65,
      kind: 'drone',
      fireTimer: rand(0.5, 2),
      alive: true,
      weavePhase: rand(0, Math.PI * 2),
      baseX: mesh.position.x,
      baseY: mesh.position.y,
    });
  }

  spawnBoss(centerZ, wave) {
    const mesh = buildBossMesh();
    this.scene.add(mesh);
    mesh.position.set(0, 0.5, -centerZ);
    const hp = 260 + wave * 70;
    const boss = {
      mesh,
      hp,
      maxHp: hp,
      radius: 1.9,
      kind: 'boss',
      fireTimer: 1,
      alive: true,
      weavePhase: 0,
      baseX: 0,
      baseY: 0.5,
      burstTimer: rand(2, 3),
    };
    this.list.push(boss);
    this.boss = boss;
    return boss;
  }

  damage(enemy, amount) {
    enemy.hp -= amount;
    if (enemy.hp <= 0 && enemy.alive) {
      enemy.alive = false;
      const big = enemy.kind === 'boss';
      this.particles.burstExplosion(enemy.mesh.position, {
        count: big ? 60 : 20,
        color: big ? 0xff3d9a : 0xff6a3d,
        speed: big ? 10 : 5,
        scale: big ? 2.2 : 1,
      });
      this.audio.explosion(big);
      this.scene.remove(enemy.mesh);
      const idx = this.list.indexOf(enemy);
      if (idx >= 0) this.list.splice(idx, 1);
      if (enemy.kind === 'boss') this.boss = null;
    }
  }

  update(dt, playerZ, playerPos, difficulty, isBossWave) {
    if (!isBossWave) {
      while (this.nextSpawnZ < playerZ + 100) {
        if (Math.random() < Math.min(0.5 + difficulty * 0.05, 0.9)) {
          this.spawnDrone(this.nextSpawnZ, difficulty);
        }
        this.nextSpawnZ += rand(22, 34) / (1 + difficulty * 0.06);
      }
    }

    for (let i = this.list.length - 1; i >= 0; i--) {
      const e = this.list[i];
      if (!e.alive) continue;
      const worldZ = -e.mesh.position.z;

      if (e.kind === 'drone') {
        e.weavePhase += dt * 1.6;
        e.mesh.position.x = e.baseX + Math.sin(e.weavePhase) * 1.4;
        e.mesh.position.y = e.baseY + Math.cos(e.weavePhase * 0.7) * 0.6;
        e.mesh.rotation.y += dt * 1.5;

        // despawn far behind
        if (worldZ < playerZ - 12) {
          this.scene.remove(e.mesh);
          this.list.splice(i, 1);
          continue;
        }
        // fire when reasonably close & roughly ahead of player
        const distZ = worldZ - playerZ;
        if (distZ < 55 && distZ > -4) {
          e.fireTimer -= dt;
          if (e.fireTimer <= 0) {
            e.fireTimer = rand(1.4, 2.6) / (1 + difficulty * 0.04);
            this.combat.spawnEnemyBolt(e.mesh.position.clone(), playerPos);
          }
        }
      } else if (e.kind === 'boss') {
        e.weavePhase += dt;
        e.mesh.position.x = Math.sin(e.weavePhase * 0.6) * LANE_HALF_WIDTH * 0.7;
        e.mesh.position.y = 0.5 + Math.cos(e.weavePhase * 0.4) * 1.2;
        e.mesh.rotation.y += dt * 0.4;
        if (e.mesh.userData.core) e.mesh.userData.core.rotation.x += dt * 2;

        e.burstTimer -= dt;
        if (e.burstTimer <= 0) {
          e.burstTimer = rand(1.6, 2.4);
          const shots = 3 + randInt(0, 2);
          for (let s = 0; s < shots; s++) {
            const offset = new THREE.Vector3(rand(-1.2, 1.2), rand(-0.8, 0.8), 0);
            this.combat.spawnEnemyBolt(e.mesh.position.clone().add(offset), playerPos);
          }
        }
      }
    }
  }

  reset() {
    for (const e of this.list) this.scene.remove(e.mesh);
    this.list = [];
    this.nextSpawnZ = 60;
    this.boss = null;
  }
}
