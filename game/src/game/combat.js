import * as THREE from 'three';

const PLAYER_BOLT_SPEED = 60;
const ENEMY_BOLT_SPEED = 26;
const BOLT_LIFETIME = 2.5;

function makeBoltMesh(color) {
  const geo = new THREE.CapsuleGeometry(0.06, 0.5, 4, 6);
  geo.rotateX(Math.PI / 2);
  const mat = new THREE.MeshBasicMaterial({ color });
  return new THREE.Mesh(geo, mat);
}

/**
 * Owns both player and enemy projectile pools. Movement is expressed in the
 * same "world z decreases as things approach camera" convention used by
 * World/Enemies so collision math stays consistent across systems.
 */
export class Combat {
  constructor(scene, particles, audio) {
    this.scene = scene;
    this.particles = particles;
    this.audio = audio;
    this.playerBolts = [];
    this.enemyBolts = [];
    this.playerBoltPool = [];
    this.enemyBoltPool = [];
  }

  spawnPlayerBolt(position) {
    const mesh = this.playerBoltPool.pop() ?? makeBoltMesh(0x4bf5ff);
    mesh.visible = true;
    mesh.position.copy(position);
    if (!mesh.parent) this.scene.add(mesh);
    this.playerBolts.push({ mesh, life: 0, damage: 34 });
  }

  spawnEnemyBolt(position, targetPos) {
    const mesh = this.enemyBoltPool.pop() ?? makeBoltMesh(0xff3d5a);
    mesh.visible = true;
    mesh.position.copy(position);
    if (!mesh.parent) this.scene.add(mesh);
    const dir = new THREE.Vector3().subVectors(targetPos, position).normalize();
    mesh.lookAt(position.clone().add(dir));
    this.enemyBolts.push({ mesh, life: 0, dir, damage: 14 });
  }

  update(dt, { world, enemies, player, onPlayerHit, onScore }) {
    // player bolts: move toward -z (forward), test vs asteroids and enemies
    for (let i = this.playerBolts.length - 1; i >= 0; i--) {
      const b = this.playerBolts[i];
      b.mesh.position.z -= PLAYER_BOLT_SPEED * dt;
      b.life += dt;
      let hit = false;

      const worldZ = -b.mesh.position.z;
      for (const a of world.nearby(worldZ, 4)) {
        if (a.mesh.position.distanceTo(b.mesh.position) < a.radius + 0.3) {
          this.particles.burstExplosion(b.mesh.position, { count: 8, color: 0xffb84b, speed: 4, scale: 0.6 });
          world.destroyAsteroid(a);
          onScore?.(10);
          hit = true;
          break;
        }
      }
      if (!hit) {
        for (const e of enemies.list) {
          if (!e.alive) continue;
          if (e.mesh.position.distanceTo(b.mesh.position) < e.radius) {
            enemies.damage(e, b.damage);
            this.particles.emitSparks(b.mesh.position, { count: 10, color: 0xff3d5a, speed: 3 });
            hit = true;
            break;
          }
        }
      }
      if (hit || b.life > BOLT_LIFETIME) {
        this._recyclePlayerBolt(i);
      }
    }

    // enemy bolts: move along stored dir, test vs player
    for (let i = this.enemyBolts.length - 1; i >= 0; i--) {
      const b = this.enemyBolts[i];
      b.mesh.position.addScaledVector(b.dir, ENEMY_BOLT_SPEED * dt);
      b.life += dt;
      const dist = b.mesh.position.distanceTo(player.mesh.position);
      if (dist < player.radius + 0.25) {
        onPlayerHit?.(b.damage);
        this.particles.emitSparks(b.mesh.position, { count: 8, color: 0xff3d5a, speed: 3 });
        this._recycleEnemyBolt(i);
        continue;
      }
      if (b.life > BOLT_LIFETIME) this._recycleEnemyBolt(i);
    }
  }

  _recyclePlayerBolt(i) {
    const b = this.playerBolts[i];
    b.mesh.visible = false;
    this.playerBoltPool.push(b.mesh);
    this.playerBolts.splice(i, 1);
  }

  _recycleEnemyBolt(i) {
    const b = this.enemyBolts[i];
    b.mesh.visible = false;
    this.enemyBoltPool.push(b.mesh);
    this.enemyBolts.splice(i, 1);
  }

  reset() {
    for (const b of [...this.playerBolts]) this._recyclePlayerBolt(this.playerBolts.indexOf(b));
    for (const b of [...this.enemyBolts]) this._recycleEnemyBolt(this.enemyBolts.indexOf(b));
  }
}
