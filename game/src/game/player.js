import * as THREE from 'three';
import { clamp, lerp } from './utils.js';

const LANE_HALF_WIDTH = 4.4;
const LANE_HALF_HEIGHT = 2.6;

/** Builds a small low-poly starfighter out of primitives (no external model files). */
function buildShipMesh() {
  const group = new THREE.Group();

  const bodyMat = new THREE.MeshStandardMaterial({ color: 0xd8def5, metalness: 0.7, roughness: 0.25 });
  const accentMat = new THREE.MeshStandardMaterial({
    color: 0x4bf5ff,
    emissive: 0x2ad4ff,
    emissiveIntensity: 1.4,
    metalness: 0.2,
    roughness: 0.3,
  });
  const darkMat = new THREE.MeshStandardMaterial({ color: 0x181c2e, metalness: 0.5, roughness: 0.5 });

  const hull = new THREE.Mesh(new THREE.ConeGeometry(0.42, 1.7, 6), bodyMat);
  hull.rotation.x = Math.PI / 2;
  group.add(hull);

  const cockpit = new THREE.Mesh(new THREE.SphereGeometry(0.22, 12, 10), accentMat);
  cockpit.position.set(0, 0.18, 0.25);
  cockpit.scale.set(1, 0.8, 1.3);
  group.add(cockpit);

  const wingGeo = new THREE.BoxGeometry(1.6, 0.06, 0.6);
  const wingL = new THREE.Mesh(wingGeo, darkMat);
  wingL.position.set(-0.75, -0.02, 0.1);
  wingL.rotation.z = 0.08;
  group.add(wingL);
  const wingR = wingL.clone();
  wingR.position.x = 0.75;
  wingR.rotation.z = -0.08;
  group.add(wingR);

  const finGeo = new THREE.BoxGeometry(0.06, 0.5, 0.5);
  [-0.55, 0.55].forEach((x) => {
    const fin = new THREE.Mesh(finGeo, accentMat);
    fin.position.set(x, 0.02, 0.35);
    group.add(fin);
  });

  const engineGeo = new THREE.CylinderGeometry(0.14, 0.18, 0.4, 10);
  [-0.35, 0.35].forEach((x) => {
    const engine = new THREE.Mesh(engineGeo, darkMat);
    engine.position.set(x, -0.05, 0.7);
    engine.rotation.x = Math.PI / 2;
    group.add(engine);
    const glow = new THREE.Mesh(
      new THREE.CircleGeometry(0.11, 12),
      new THREE.MeshBasicMaterial({ color: 0x4bf5ff, transparent: true, opacity: 0.95 }),
    );
    glow.position.set(x, -0.05, 0.92);
    glow.rotation.y = Math.PI;
    group.add(glow);
  });

  group.scale.setScalar(0.85);
  return group;
}

export class Player {
  constructor(scene, particles, audio, engineGlow) {
    this.scene = scene;
    this.particles = particles;
    this.audio = audio;
    this.engineGlow = engineGlow;

    this.mesh = buildShipMesh();
    scene.add(this.mesh);

    this.target = new THREE.Vector2(0, 0); // desired lane position, -1..1 range scaled
    this.pos = new THREE.Vector2(0, 0);
    this.velocityRoll = 0;

    this.hullMax = 100;
    this.hull = 100;
    this.shieldMax = 100;
    this.shield = 0;
    this.boostMax = 100;
    this.boost = 100;
    this.boosting = false;
    this.invulnTimer = 0;

    this.fireCooldown = 0;
    this.fireRate = 0.22;
    this.multiShotTimer = 0;
    this.rapidFireTimer = 0;

    this.z = 0; // world-space forward progress (increases, asteroids/enemies move toward +z relatively)
    this.baseSpeed = 16;
    this.speed = this.baseSpeed;

    this.thrusterT = 0;
  }

  get radius() {
    return 0.55;
  }

  setPointer(nx, ny) {
    // nx, ny in -1..1 (screen space), map to world lane offsets
    this.target.x = clamp(nx, -1, 1) * LANE_HALF_WIDTH;
    this.target.y = clamp(-ny, -1, 1) * LANE_HALF_HEIGHT;
  }

  tryFire(weaponSystem) {
    if (this.fireCooldown > 0) return;
    const rate = this.rapidFireTimer > 0 ? this.fireRate * 0.45 : this.fireRate;
    this.fireCooldown = rate;
    const muzzle = new THREE.Vector3(this.mesh.position.x, this.mesh.position.y, this.mesh.position.z - 0.9);
    if (this.multiShotTimer > 0) {
      weaponSystem.spawnPlayerBolt(muzzle.clone().add(new THREE.Vector3(-0.4, 0, 0)));
      weaponSystem.spawnPlayerBolt(muzzle.clone());
      weaponSystem.spawnPlayerBolt(muzzle.clone().add(new THREE.Vector3(0.4, 0, 0)));
    } else {
      weaponSystem.spawnPlayerBolt(muzzle);
    }
    this.audio.fire();
  }

  activateBoost() {
    if (this.boost < 20 || this.boosting) return;
    this.boosting = true;
    this.audio.boost();
  }

  applyPowerup(type) {
    if (type === 'shield') {
      this.shield = Math.min(this.shieldMax, this.shield + 50);
    } else if (type === 'repair') {
      this.hull = Math.min(this.hullMax, this.hull + 35);
    } else if (type === 'multishot') {
      this.multiShotTimer = 9;
    } else if (type === 'rapid') {
      this.rapidFireTimer = 8;
    } else if (type === 'boostfull') {
      this.boost = this.boostMax;
    }
  }

  damage(amount) {
    if (this.invulnTimer > 0) return false;
    if (this.shield > 0) {
      this.shield = Math.max(0, this.shield - amount);
      this.invulnTimer = 0.15;
      this.audio.hit();
      return false;
    }
    this.hull -= amount;
    this.invulnTimer = 0.4;
    this.audio.hit();
    if (this.hull <= 0) {
      this.hull = 0;
      return true; // destroyed
    }
    return false;
  }

  update(dt) {
    if (this.invulnTimer > 0) this.invulnTimer -= dt;
    if (this.multiShotTimer > 0) this.multiShotTimer -= dt;
    if (this.rapidFireTimer > 0) this.rapidFireTimer -= dt;
    if (this.fireCooldown > 0) this.fireCooldown -= dt;

    // boost economy
    if (this.boosting) {
      this.boost -= dt * 34;
      if (this.boost <= 0) {
        this.boost = 0;
        this.boosting = false;
      }
    } else {
      this.boost = Math.min(this.boostMax, this.boost + dt * 9);
    }
    const targetSpeed = this.boosting ? this.baseSpeed * 1.85 : this.baseSpeed;
    this.speed = lerp(this.speed, targetSpeed, dt * 3);

    // smooth follow toward pointer target with slight overshoot roll
    const prevX = this.pos.x;
    this.pos.x = lerp(this.pos.x, this.target.x, dt * 7);
    this.pos.y = lerp(this.pos.y, this.target.y, dt * 7);
    const dx = this.pos.x - prevX;
    this.velocityRoll = lerp(this.velocityRoll, clamp(-dx * 18, -0.7, 0.7), dt * 8);

    this.mesh.position.x = this.pos.x;
    this.mesh.position.y = this.pos.y + 0.2;
    this.mesh.rotation.z = this.velocityRoll;
    this.mesh.rotation.x = lerp(this.mesh.rotation.x, clamp((this.pos.y - this.mesh.position.y) * 0.1, -0.3, 0.3), dt * 5);
    this.mesh.rotation.y = lerp(this.mesh.rotation.y, -this.velocityRoll * 0.6, dt * 5);

    this.z += this.speed * dt;

    // engine light + thruster particles
    this.engineGlow.position.set(this.mesh.position.x, this.mesh.position.y, this.mesh.position.z + 0.7);
    this.engineGlow.intensity = this.boosting ? 3.2 : 1.6;
    this.engineGlow.color.set(this.boosting ? 0xffb84b : 0x4bf5ff);

    this.thrusterT += dt;
    if (this.thrusterT > (this.boosting ? 0.02 : 0.045)) {
      this.thrusterT = 0;
      this.particles.emitSparks(
        new THREE.Vector3(this.mesh.position.x, this.mesh.position.y, this.mesh.position.z + 0.85),
        { count: this.boosting ? 4 : 2, color: this.boosting ? 0xffb84b : 0x4bf5ff, speed: 1.5 },
      );
    }

    // ship physically advances through the static world; camera trails it
    this.mesh.position.z = -this.z;
  }

  get worldRadius() {
    return LANE_HALF_WIDTH;
  }
}

export { LANE_HALF_WIDTH, LANE_HALF_HEIGHT };
