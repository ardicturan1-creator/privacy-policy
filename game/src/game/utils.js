export function rand(min, max) {
  return min + Math.random() * (max - min);
}

export function randInt(min, max) {
  return Math.floor(rand(min, max + 1));
}

export function clamp(v, min, max) {
  return Math.max(min, Math.min(max, v));
}

export function lerp(a, b, t) {
  return a + (b - a) * t;
}

export function choice(arr) {
  return arr[Math.floor(Math.random() * arr.length)];
}

/** Tiny event emitter used to decouple game systems from UI/audio. */
export class Emitter {
  constructor() {
    this.listeners = new Map();
  }
  on(evt, fn) {
    if (!this.listeners.has(evt)) this.listeners.set(evt, new Set());
    this.listeners.get(evt).add(fn);
    return () => this.listeners.get(evt)?.delete(fn);
  }
  emit(evt, payload) {
    this.listeners.get(evt)?.forEach((fn) => fn(payload));
  }
}

/** Generic fixed-size object pool to avoid GC churn during gameplay. */
export class Pool {
  constructor(factory, size) {
    this.factory = factory;
    this.items = Array.from({ length: size }, () => factory());
    this.free = [...this.items];
  }
  acquire() {
    return this.free.pop() ?? this.factory();
  }
  release(item) {
    this.free.push(item);
  }
}
