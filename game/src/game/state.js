import { Emitter } from './utils.js';

const BEST_KEY = 'nebula-drift-best-score';
const WAVE_DISTANCE = 480; // world-z distance per wave before a boss check triggers
const BOSS_EVERY = 3; // every Nth wave is a boss wave

export class GameState extends Emitter {
  constructor() {
    super();
    this.reset();
    this.best = Number(localStorage.getItem(BEST_KEY) || 0);
  }

  reset() {
    this.score = 0;
    this.combo = 1;
    this.comboTimer = 0;
    this.wave = 1;
    this.waveStartZ = 0;
    this.bossActive = false;
    this.bossWaveTriggered = false;
    this.running = false;
    this.paused = false;
    this._bossWasAlive = false;
  }

  get difficulty() {
    return this.wave - 1 + (this.comboTimer > 0 ? 0 : 0);
  }

  isBossWaveNumber(wave = this.wave) {
    return wave % BOSS_EVERY === 0;
  }

  addScore(base) {
    this.score += Math.round(base * this.combo);
    this.combo = Math.min(8, this.combo + 0.15);
    this.comboTimer = 2.2;
    this.emit('score', this.score);
  }

  tick(dt, playerZ) {
    if (this.comboTimer > 0) {
      this.comboTimer -= dt;
      if (this.comboTimer <= 0) {
        this.combo = 1;
        this.emit('combo', this.combo);
      }
    }

    if (!this.bossActive && playerZ - this.waveStartZ > WAVE_DISTANCE) {
      if (this.isBossWaveNumber() && !this.bossWaveTriggered) {
        this.bossWaveTriggered = true;
        this.bossActive = true;
        this.emit('bossIncoming', this.wave);
      } else if (!this.isBossWaveNumber()) {
        this.advanceWave(playerZ);
      }
    }
  }

  advanceWave(playerZ) {
    this.wave += 1;
    this.waveStartZ = playerZ;
    this.bossWaveTriggered = false;
    this.emit('wave', this.wave);
  }

  onBossDefeated(playerZ) {
    this.bossActive = false;
    this.addScore(500);
    this.advanceWave(playerZ);
  }

  finalize() {
    this.running = false;
    if (this.score > this.best) {
      this.best = this.score;
      localStorage.setItem(BEST_KEY, String(this.best));
      return true;
    }
    return false;
  }
}
