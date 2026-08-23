/**
 * Fully procedural WebAudio sound design — no external asset files, so the
 * game keeps working offline / inside a WebView with zero network fetches.
 */
export class AudioSystem {
  constructor() {
    this.ctx = null;
    this.master = null;
    this.musicNodes = [];
    this.enabled = true;
    this.musicPlaying = false;
  }

  ensure() {
    if (this.ctx) return;
    const Ctx = window.AudioContext || window.webkitAudioContext;
    this.ctx = new Ctx();
    this.master = this.ctx.createGain();
    this.master.gain.value = 0.55;
    this.master.connect(this.ctx.destination);
  }

  resume() {
    this.ensure();
    if (this.ctx.state === 'suspended') this.ctx.resume();
  }

  setEnabled(v) {
    this.enabled = v;
    if (this.master) this.master.gain.value = v ? 0.55 : 0;
  }

  _env(gain, t0, attack, decay, peak = 1) {
    gain.gain.cancelScheduledValues(t0);
    gain.gain.setValueAtTime(0.0001, t0);
    gain.gain.exponentialRampToValueAtTime(peak, t0 + attack);
    gain.gain.exponentialRampToValueAtTime(0.0001, t0 + attack + decay);
  }

  fire() {
    if (!this.enabled) return;
    this.ensure();
    const t0 = this.ctx.currentTime;
    const osc = this.ctx.createOscillator();
    const gain = this.ctx.createGain();
    osc.type = 'square';
    osc.frequency.setValueAtTime(880, t0);
    osc.frequency.exponentialRampToValueAtTime(220, t0 + 0.12);
    this._env(gain, t0, 0.005, 0.12, 0.18);
    osc.connect(gain).connect(this.master);
    osc.start(t0);
    osc.stop(t0 + 0.15);
  }

  explosion(big = false) {
    if (!this.enabled) return;
    this.ensure();
    const t0 = this.ctx.currentTime;
    const dur = big ? 1.1 : 0.5;
    const bufferSize = this.ctx.sampleRate * dur;
    const buffer = this.ctx.createBuffer(1, bufferSize, this.ctx.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < bufferSize; i++) {
      data[i] = (Math.random() * 2 - 1) * Math.pow(1 - i / bufferSize, 2);
    }
    const noise = this.ctx.createBufferSource();
    noise.buffer = buffer;
    const filter = this.ctx.createBiquadFilter();
    filter.type = 'lowpass';
    filter.frequency.setValueAtTime(big ? 1800 : 1200, t0);
    filter.frequency.exponentialRampToValueAtTime(80, t0 + dur);
    const gain = this.ctx.createGain();
    gain.gain.setValueAtTime(big ? 0.9 : 0.55, t0);
    gain.gain.exponentialRampToValueAtTime(0.001, t0 + dur);
    noise.connect(filter).connect(gain).connect(this.master);
    noise.start(t0);
    noise.stop(t0 + dur);
  }

  hit() {
    if (!this.enabled) return;
    this.ensure();
    const t0 = this.ctx.currentTime;
    const osc = this.ctx.createOscillator();
    const gain = this.ctx.createGain();
    osc.type = 'sawtooth';
    osc.frequency.setValueAtTime(140, t0);
    osc.frequency.exponentialRampToValueAtTime(60, t0 + 0.2);
    this._env(gain, t0, 0.005, 0.2, 0.5);
    osc.connect(gain).connect(this.master);
    osc.start(t0);
    osc.stop(t0 + 0.22);
  }

  powerup() {
    if (!this.enabled) return;
    this.ensure();
    const t0 = this.ctx.currentTime;
    const osc = this.ctx.createOscillator();
    const gain = this.ctx.createGain();
    osc.type = 'triangle';
    osc.frequency.setValueAtTime(440, t0);
    osc.frequency.exponentialRampToValueAtTime(1320, t0 + 0.25);
    this._env(gain, t0, 0.01, 0.28, 0.35);
    osc.connect(gain).connect(this.master);
    osc.start(t0);
    osc.stop(t0 + 0.3);
  }

  boost() {
    if (!this.enabled) return;
    this.ensure();
    const t0 = this.ctx.currentTime;
    const osc = this.ctx.createOscillator();
    const gain = this.ctx.createGain();
    osc.type = 'sawtooth';
    osc.frequency.setValueAtTime(120, t0);
    osc.frequency.exponentialRampToValueAtTime(900, t0 + 0.4);
    this._env(gain, t0, 0.02, 0.45, 0.28);
    osc.connect(gain).connect(this.master);
    osc.start(t0);
    osc.stop(t0 + 0.45);
  }

  waveStart() {
    if (!this.enabled) return;
    this.ensure();
    const t0 = this.ctx.currentTime;
    [0, 0.12, 0.24].forEach((delay, i) => {
      const osc = this.ctx.createOscillator();
      const gain = this.ctx.createGain();
      osc.type = 'triangle';
      osc.frequency.value = 330 * Math.pow(1.25, i);
      this._env(gain, t0 + delay, 0.01, 0.3, 0.22);
      osc.connect(gain).connect(this.master);
      osc.start(t0 + delay);
      osc.stop(t0 + delay + 0.32);
    });
  }

  gameOver() {
    if (!this.enabled) return;
    this.ensure();
    const t0 = this.ctx.currentTime;
    const osc = this.ctx.createOscillator();
    const gain = this.ctx.createGain();
    osc.type = 'sawtooth';
    osc.frequency.setValueAtTime(320, t0);
    osc.frequency.exponentialRampToValueAtTime(40, t0 + 1.2);
    this._env(gain, t0, 0.02, 1.2, 0.4);
    osc.connect(gain).connect(this.master);
    osc.start(t0);
    osc.stop(t0 + 1.25);
  }

  startMusic() {
    if (!this.enabled || this.musicPlaying) return;
    this.ensure();
    this.musicPlaying = true;
    const bpm = 96;
    const beat = 60 / bpm;
    const bassNotes = [55, 55, 62, 49];
    let step = 0;
    const musicGain = this.ctx.createGain();
    musicGain.gain.value = 0.22;
    musicGain.connect(this.master);
    this.musicGain = musicGain;

    this.musicTimer = setInterval(() => {
      if (!this.musicPlaying) return;
      const t0 = this.ctx.currentTime;
      const osc = this.ctx.createOscillator();
      const gain = this.ctx.createGain();
      osc.type = 'sine';
      osc.frequency.value = bassNotes[step % bassNotes.length];
      gain.gain.setValueAtTime(0.0001, t0);
      gain.gain.exponentialRampToValueAtTime(0.5, t0 + 0.05);
      gain.gain.exponentialRampToValueAtTime(0.0001, t0 + beat * 0.9);
      osc.connect(gain).connect(musicGain);
      osc.start(t0);
      osc.stop(t0 + beat);

      if (step % 2 === 0) {
        const hat = this.ctx.createOscillator();
        const hg = this.ctx.createGain();
        hat.type = 'square';
        hat.frequency.value = 4000;
        hg.gain.setValueAtTime(0.03, t0);
        hg.gain.exponentialRampToValueAtTime(0.0001, t0 + 0.05);
        hat.connect(hg).connect(musicGain);
        hat.start(t0);
        hat.stop(t0 + 0.06);
      }
      step++;
    }, beat * 1000);
  }

  stopMusic() {
    this.musicPlaying = false;
    if (this.musicTimer) clearInterval(this.musicTimer);
    if (this.musicGain) {
      this.musicGain.disconnect();
      this.musicGain = null;
    }
  }
}
