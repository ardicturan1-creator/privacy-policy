import { clamp } from './utils.js';

const $ = (id) => document.getElementById(id);

export class UI {
  constructor() {
    this.el = {
      scoreVal: $('score'),
      comboVal: $('combo'),
      waveVal: $('wave'),
      hullFill: $('hull-fill'),
      shieldFill: $('shield-fill'),
      boostFill: $('boost-fill'),
      bossWrap: $('boss-bar-wrap'),
      bossFill: $('boss-fill'),
      bossName: $('boss-name'),
      startScreen: $('start-screen'),
      pauseScreen: $('pause-screen'),
      gameoverScreen: $('gameover-screen'),
      startBtn: $('start-btn'),
      resumeBtn: $('resume-btn'),
      quitBtn: $('quit-btn'),
      retryBtn: $('retry-btn'),
      pauseBtn: $('pause-btn'),
      fireBtn: $('fire-btn'),
      boostBtn: $('boost-btn'),
      bestScore: $('best-score'),
      finalScore: $('final-score'),
      finalWave: $('final-wave'),
      finalBest: $('final-best'),
      waveToast: $('wave-toast'),
      canvas: $('scene'),
      hud: $('hud'),
    };

    this.pointerActive = false;
    this.pointerNX = 0;
    this.pointerNY = 0;
    this.firing = false;
  }

  bindStart(fn) {
    this.el.startBtn.addEventListener('click', fn);
  }
  bindResume(fn) {
    this.el.resumeBtn.addEventListener('click', fn);
  }
  bindQuit(fn) {
    this.el.quitBtn.addEventListener('click', fn);
  }
  bindRetry(fn) {
    this.el.retryBtn.addEventListener('click', fn);
  }
  bindPause(fn) {
    this.el.pauseBtn.addEventListener('click', fn);
  }

  bindBoost(fn) {
    const trigger = (e) => {
      e.preventDefault();
      fn();
    };
    this.el.boostBtn.addEventListener('pointerdown', trigger);
  }

  bindFire(onDown, onUp) {
    this.el.fireBtn.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      this.firing = true;
      onDown?.();
    });
    window.addEventListener('pointerup', () => {
      this.firing = false;
      onUp?.();
    });
  }

  /** Drag-to-steer across the whole canvas; also fires while dragging on mobile for convenience. */
  bindSteering(onFire) {
    const canvas = this.el.canvas;
    const setFromEvent = (e) => {
      const nx = (e.clientX / window.innerWidth) * 2 - 1;
      const ny = (e.clientY / window.innerHeight) * 2 - 1;
      this.pointerNX = clamp(nx, -1, 1);
      this.pointerNY = clamp(ny, -1, 1);
    };
    canvas.addEventListener('pointerdown', (e) => {
      this.pointerActive = true;
      setFromEvent(e);
    });
    window.addEventListener('pointermove', (e) => {
      if (this.pointerActive) setFromEvent(e);
    });
    window.addEventListener('pointerup', () => {
      this.pointerActive = false;
    });
    window.addEventListener('pointercancel', () => {
      this.pointerActive = false;
    });
  }

  getSteer() {
    return { nx: this.pointerNX, ny: this.pointerNY, active: this.pointerActive };
  }

  showStart() {
    this.el.startScreen.classList.remove('hidden');
    this.el.pauseScreen.classList.add('hidden');
    this.el.gameoverScreen.classList.add('hidden');
    this.el.pauseBtn.classList.add('hidden');
    this.el.bestScore.textContent = this._fmt(this._best);
  }

  showHud() {
    this.el.startScreen.classList.add('hidden');
    this.el.pauseScreen.classList.add('hidden');
    this.el.gameoverScreen.classList.add('hidden');
    this.el.pauseBtn.classList.remove('hidden');
  }

  showPause() {
    this.el.pauseScreen.classList.remove('hidden');
  }

  hidePause() {
    this.el.pauseScreen.classList.add('hidden');
  }

  showGameOver({ score, wave, best, isNewBest }) {
    this.el.pauseBtn.classList.add('hidden');
    this.el.gameoverScreen.classList.remove('hidden');
    this.el.finalScore.textContent = this._fmt(score);
    this.el.finalWave.textContent = String(wave);
    this.el.finalBest.textContent = this._fmt(best);
    $('gameover-title').textContent = isNewBest ? 'YENİ REKOR!' : 'GEMİ İMHA EDİLDİ';
  }

  setBest(v) {
    this._best = v;
  }

  updateScore(score, combo, wave) {
    this.el.scoreVal.textContent = this._fmt(score);
    this.el.comboVal.textContent = combo.toFixed(1);
    this.el.waveVal.textContent = String(wave);
  }

  updateBars(hullPct, shieldPct, boostPct) {
    this.el.hullFill.style.width = `${clamp(hullPct, 0, 100)}%`;
    this.el.shieldFill.style.width = `${clamp(shieldPct, 0, 100)}%`;
    this.el.boostFill.style.width = `${clamp(boostPct, 0, 100)}%`;
    this.el.hullFill.style.background =
      hullPct < 30 ? 'linear-gradient(90deg,#ff3d3d,#ff8a4b)' : '';
  }

  showBoss(name = 'SEKTÖR MUHAFIZI') {
    this.el.bossWrap.classList.remove('hidden');
    this.el.bossName.textContent = name;
  }
  hideBoss() {
    this.el.bossWrap.classList.add('hidden');
  }
  updateBoss(pct) {
    this.el.bossFill.style.width = `${clamp(pct, 0, 100)}%`;
  }

  toastWave(text) {
    this.el.waveToast.textContent = text;
    this.el.waveToast.classList.remove('hidden');
    requestAnimationFrame(() => this.el.waveToast.classList.add('show'));
    clearTimeout(this._toastTimer);
    this._toastTimer = setTimeout(() => {
      this.el.waveToast.classList.remove('show');
      setTimeout(() => this.el.waveToast.classList.add('hidden'), 350);
    }, 1600);
  }

  _fmt(n) {
    return Math.round(n).toLocaleString('tr-TR');
  }
}
