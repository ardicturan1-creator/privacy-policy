import * as THREE from 'three';
import * as CANNON from 'cannon-es';

/**
 * Physically-simulated debris/spark bursts. Each burst gets real cannon-es
 * rigid bodies (so chunks tumble and bounce off each other believably)
 * driving a shared InstancedMesh for rendering — one draw call regardless
 * of how many explosions are alive at once.
 */
export class ParticleSystem {
  constructor(scene, physicsWorld) {
    this.scene = scene;
    this.physicsWorld = physicsWorld;
    this.maxDebris = 260;
    this.debris = []; // { body, life, maxLife }

    const geo = new THREE.IcosahedronGeometry(0.09, 0);
    const mat = new THREE.MeshStandardMaterial({
      color: 0xffb84b,
      emissive: 0xff5a2a,
      emissiveIntensity: 1.6,
      roughness: 0.4,
      metalness: 0.3,
    });
    this.mesh = new THREE.InstancedMesh(geo, mat, this.maxDebris);
    this.mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    this.mesh.frustumCulled = false;
    this.dummy = new THREE.Object3D();
    scene.add(this.mesh);

    // simple additive spark points for muzzle flashes / trails
    const sparkGeo = new THREE.BufferGeometry();
    this.sparkCount = 400;
    this.sparkPositions = new Float32Array(this.sparkCount * 3);
    this.sparkVelocities = new Array(this.sparkCount).fill(null).map(() => new THREE.Vector3());
    this.sparkLife = new Float32Array(this.sparkCount);
    this.sparkColor = new Float32Array(this.sparkCount * 3);
    sparkGeo.setAttribute('position', new THREE.BufferAttribute(this.sparkPositions, 3));
    sparkGeo.setAttribute('color', new THREE.BufferAttribute(this.sparkColor, 3));
    const sparkMat = new THREE.PointsMaterial({
      size: 0.12,
      vertexColors: true,
      transparent: true,
      opacity: 1,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    this.sparks = new THREE.Points(sparkGeo, sparkMat);
    this.sparks.frustumCulled = false;
    scene.add(this.sparks);
    this.sparkCursor = 0;
  }

  burstExplosion(position, { count = 18, color = 0xffb84b, speed = 6, scale = 1 } = {}) {
    for (let i = 0; i < count; i++) {
      if (this.debris.length >= this.maxDebris) {
        const oldest = this.debris.shift();
        this.physicsWorld.removeBody(oldest.body);
      }
      const body = new CANNON.Body({
        mass: 0.05,
        shape: new CANNON.Sphere(0.09 * scale),
        position: new CANNON.Vec3(position.x, position.y, position.z),
        linearDamping: 0.35,
        angularDamping: 0.5,
      });
      const dir = new THREE.Vector3(
        Math.random() * 2 - 1,
        Math.random() * 2 - 1,
        Math.random() * 2 - 1,
      ).normalize();
      const s = speed * (0.5 + Math.random());
      body.velocity.set(dir.x * s, dir.y * s, dir.z * s + 4);
      body.angularVelocity.set(Math.random() * 6, Math.random() * 6, Math.random() * 6);
      this.physicsWorld.addBody(body);
      this.debris.push({ body, life: 0, maxLife: 0.9 + Math.random() * 0.6, color });
    }
    this.emitSparks(position, { count: 24, color, speed: speed * 1.4 });
  }

  emitSparks(position, { count = 12, color = 0x4bf5ff, speed = 5 } = {}) {
    const c = new THREE.Color(color);
    for (let i = 0; i < count; i++) {
      const idx = this.sparkCursor;
      this.sparkCursor = (this.sparkCursor + 1) % this.sparkCount;
      const dir = new THREE.Vector3(
        Math.random() * 2 - 1,
        Math.random() * 2 - 1,
        Math.random() * 2 - 1,
      ).normalize();
      this.sparkVelocities[idx].copy(dir).multiplyScalar(speed * (0.4 + Math.random()));
      this.sparkPositions[idx * 3] = position.x;
      this.sparkPositions[idx * 3 + 1] = position.y;
      this.sparkPositions[idx * 3 + 2] = position.z;
      this.sparkLife[idx] = 0.5 + Math.random() * 0.4;
      this.sparkColor[idx * 3] = c.r;
      this.sparkColor[idx * 3 + 1] = c.g;
      this.sparkColor[idx * 3 + 2] = c.b;
    }
  }

  update(dt) {
    // debris chunks
    for (let i = this.debris.length - 1; i >= 0; i--) {
      const d = this.debris[i];
      d.life += dt;
      if (d.life >= d.maxLife) {
        this.physicsWorld.removeBody(d.body);
        this.debris.splice(i, 1);
      }
    }
    for (let i = 0; i < this.maxDebris; i++) {
      if (i < this.debris.length) {
        const d = this.debris[i];
        const t = d.life / d.maxLife;
        const scale = Math.max(0.001, 1 - t);
        this.dummy.position.copy(d.body.position);
        this.dummy.quaternion.copy(d.body.quaternion);
        this.dummy.scale.setScalar(scale);
        this.dummy.updateMatrix();
        this.mesh.setMatrixAt(i, this.dummy.matrix);
      } else {
        this.dummy.scale.setScalar(0);
        this.dummy.updateMatrix();
        this.mesh.setMatrixAt(i, this.dummy.matrix);
      }
    }
    this.mesh.instanceMatrix.needsUpdate = true;

    // sparks (simple kinematic, no physics needed)
    const pos = this.sparks.geometry.attributes.position;
    for (let i = 0; i < this.sparkCount; i++) {
      if (this.sparkLife[i] > 0) {
        this.sparkLife[i] -= dt;
        this.sparkPositions[i * 3] += this.sparkVelocities[i].x * dt;
        this.sparkPositions[i * 3 + 1] += this.sparkVelocities[i].y * dt;
        this.sparkPositions[i * 3 + 2] += this.sparkVelocities[i].z * dt;
        this.sparkVelocities[i].multiplyScalar(0.94);
      } else {
        this.sparkPositions[i * 3 + 1] = -9999;
      }
    }
    pos.needsUpdate = true;
  }
}
