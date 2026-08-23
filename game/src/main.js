import * as THREE from 'three';
import * as CANNON from 'cannon-es';
import './style.css';
import { createScene } from './game/scene.js';
import { Player } from './game/player.js';
import { World } from './game/world.js';
import { Enemies } from './game/enemies.js';
import { Combat } from './game/combat.js';
import { Powerups } from './game/powerups.js';
import { ParticleSystem } from './game/particles.js';
import { AudioSystem } from './game/audio.js';
import { GameState } from './game/state.js';
import { UI } from './game/ui.js';
import { clamp, lerp } from './game/utils.js';

const canvas = document.getElementById('scene');
const { renderer, scene, camera, engineGlow, tickBackdrop } = createScene(canvas);

const physicsWorld = new CANNON.World({ gravity: new CANNON.Vec3(0, -1.2, 0) });
physicsWorld.broadphase = new CANNON.SAPBroadphase(physicsWorld);

const audio = new AudioSystem();
const particles = new ParticleSystem(scene, physicsWorld);
const player = new Player(scene, particles, audio, engineGlow);
const world = new World(scene);
const combat = new Combat(scene, particles, audio);
const enemies = new Enemies(scene, particles, audio, combat);
const powerups = new Powerups(scene, particles, audio);
const state = new GameState();
const ui = new UI();

ui.setBest(state.best);
ui.showStart();

let firing = false;
let paused = false;
let running = false;
let shakeTime = 0;
let shakeMag = 0;

// ---------------------------------------------------------------- input --
ui.bindSteering();
ui.bindFire(
  () => (firing = true),
  () => (firing = false),
);
ui.bindBoost(() => player.activateBoost());
ui.bindStart(() => startGame());
ui.bindResume(() => setPaused(false));
ui.bindQuit(() => {
  setPaused(false);
  endToMenu();
});
ui.bindRetry(() => startGame());
ui.bindPause(() => setPaused(!paused));

document.addEventListener('visibilitychange', () => {
  if (document.hidden && running && !paused) setPaused(true);
});

// keyboard fallback for desktop testing
const keys = new Set();
window.addEventListener('keydown', (e) => {
  keys.add(e.code);
  if (e.code === 'Space') firing = true;
  if (e.code === 'ShiftLeft' || e.code === 'ShiftRight') player.activateBoost();
  if (e.code === 'Escape' && running) setPaused(!paused);
});
window.addEventListener('keyup', (e) => {
  keys.delete(e.code);
  if (e.code === 'Space') firing = false;
});
let keyboardNX = 0;
let keyboardNY = 0;

// ------------------------------------------------------------- game flow --
function startGame() {
  audio.resume();
  audio.startMusic();
  world.reset();
  enemies.reset();
  powerups.reset();
  combat.reset();
  state.reset();
  Object.assign(player, {
    hull: player.hullMax,
    shield: 0,
    boost: player.boostMax,
    z: 0,
    speed: player.baseSpeed,
    multiShotTimer: 0,
    rapidFireTimer: 0,
  });
  player.pos.set(0, 0);
  player.target.set(0, 0);
  state.running = true;
  running = true;
  paused = false;
  ui.showHud();
  ui.hideBoss();
}

function endToMenu() {
  running = false;
  audio.stopMusic();
  ui.showStart();
}

function setPaused(v) {
  if (!running) return;
  paused = v;
  if (v) {
    ui.showPause();
  } else {
    ui.hidePause();
    audio.resume();
  }
}

function gameOver() {
  running = false;
  audio.stopMusic();
  audio.gameOver();
  const isNewBest = state.finalize();
  ui.setBest(state.best);
  ui.showGameOver({ score: state.score, wave: state.wave, best: state.best, isNewBest });
}

state.on('wave', (w) => {
  ui.toastWave(`DALGA ${w}`);
  audio.waveStart();
});
state.on('bossIncoming', (w) => {
  ui.toastWave('UYARI: MUHAFIZ YAKLAŞIYOR');
  audio.waveStart();
  const boss = enemies.spawnBoss(player.z + 55, w);
  ui.showBoss();
});

function triggerShake(mag, time) {
  shakeMag = Math.max(shakeMag, mag);
  shakeTime = Math.max(shakeTime, time);
}

// ------------------------------------------------------------ main loop --
const clock = new THREE.Clock();

function animate() {
  requestAnimationFrame(animate);
  const dt = Math.min(clock.getDelta(), 0.05);

  if (running && !paused) {
    update(dt);
  }
  tickBackdrop(dt, player.mesh.position.z);
  particles.update(dt);
  updateCamera(dt);
  renderer.render(scene, camera);
}

function update(dt) {
  physicsWorld.step(1 / 60, dt, 3);

  // steering: pointer/touch takes priority, keyboard as fallback for desktop
  const steer = ui.getSteer();
  if (steer.active) {
    player.setPointer(steer.nx, steer.ny);
  } else {
    keyboardNX = clamp(keyboardNX + ((keys.has('ArrowRight') || keys.has('KeyD') ? 1 : 0) - (keys.has('ArrowLeft') || keys.has('KeyA') ? 1 : 0)) * dt * 2.4, -1, 1);
    keyboardNY = clamp(keyboardNY + ((keys.has('ArrowUp') || keys.has('KeyW') ? 1 : 0) - (keys.has('ArrowDown') || keys.has('KeyS') ? 1 : 0)) * dt * 2.4, -1, 1);
    if (!keys.has('ArrowRight') && !keys.has('ArrowLeft') && !keys.has('KeyD') && !keys.has('KeyA')) keyboardNX = lerp(keyboardNX, 0, dt * 2);
    if (!keys.has('ArrowUp') && !keys.has('ArrowDown') && !keys.has('KeyW') && !keys.has('KeyS')) keyboardNY = lerp(keyboardNY, 0, dt * 2);
    player.setPointer(keyboardNX, -keyboardNY);
  }

  player.update(dt);

  if (firing) player.tryFire(combat);

  const difficulty = state.difficulty;
  const isBossWave = state.bossActive;
  world.update(dt, player.z, difficulty);
  enemies.update(dt, player.z, player.mesh.position, difficulty, isBossWave);
  powerups.update(dt, player.z, player);

  combat.update(dt, {
    world,
    enemies,
    player,
    onScore: (v) => state.addScore(v),
    onPlayerHit: (dmg) => {
      const destroyed = player.damage(dmg);
      triggerShake(0.18, 0.25);
      if (destroyed) gameOver();
    },
  });

  // player vs asteroid collisions
  for (const a of world.nearby(player.z, 6)) {
    if (a.mesh.position.distanceTo(player.mesh.position) < a.radius + player.radius * 0.8) {
      particles.burstExplosion(a.mesh.position, { count: 16, color: 0xff6a3d, speed: 5, scale: a.radius });
      audio.explosion(false);
      const destroyed = player.damage(18 + a.radius * 6);
      world.destroyAsteroid(a);
      triggerShake(0.25, 0.3);
      if (destroyed) gameOver();
    }
  }
  // player vs drone ramming
  for (const e of enemies.list) {
    if (e.kind !== 'drone' || !e.alive) continue;
    if (e.mesh.position.distanceTo(player.mesh.position) < e.radius + player.radius * 0.8) {
      enemies.damage(e, 999);
      const destroyed = player.damage(22);
      triggerShake(0.25, 0.3);
      if (destroyed) gameOver();
    }
  }

  // boss defeated check
  if (state.bossActive && enemies.boss === null && enemies.list.every((e) => e.kind !== 'boss')) {
    // boss existed and is now gone -> defeated (guard against the single frame before spawn)
    if (state._bossWasAlive) {
      state.onBossDefeated(player.z);
      ui.hideBoss();
      state._bossWasAlive = false;
    }
  }
  if (enemies.boss) {
    state._bossWasAlive = true;
    ui.updateBoss((enemies.boss.hp / enemies.boss.maxHp) * 100);
  }

  state.tick(dt, player.z);

  ui.updateScore(state.score, state.combo, state.wave);
  ui.updateBars(
    (player.hull / player.hullMax) * 100,
    (player.shield / player.shieldMax) * 100,
    (player.boost / player.boostMax) * 100,
  );

  if (shakeTime > 0) shakeTime -= dt;
}

function updateCamera(dt) {
  const shipZ = player.mesh.position.z;
  const followX = lerp(camera.position.x, player.mesh.position.x * 0.55, dt * 4);
  const followY = lerp(camera.position.y, 1.6 + player.mesh.position.y * 0.35, dt * 4);
  let targetZ = shipZ + 8.2 - (player.boosting ? 1.1 : 0);

  let sx = 0;
  let sy = 0;
  if (shakeTime > 0) {
    sx = (Math.random() - 0.5) * shakeMag;
    sy = (Math.random() - 0.5) * shakeMag;
  } else {
    shakeMag = 0;
  }

  camera.position.set(followX + sx, followY + sy, targetZ);
  camera.lookAt(player.mesh.position.x * 0.4, player.mesh.position.y * 0.4 + 0.6, shipZ - 24);
  camera.fov = lerp(camera.fov, player.boosting ? 76 : 68, dt * 3);
  camera.updateProjectionMatrix();
}

animate();

// register the PWA service worker (best-effort; ignored if unsupported)
if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js').catch(() => {});
  });
}
