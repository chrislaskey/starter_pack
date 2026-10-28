// Helpers the explorations share: a full-viewport canvas that follows the
// stage's size and device pixel ratio, a frame loop, and small math.

export function canvasStage(stage) {
  const canvas = document.createElement("canvas")
  canvas.style.cssText = "position:absolute;inset:0;width:100%;height:100%"
  stage.appendChild(canvas)
  const ctx = canvas.getContext("2d")
  const size = {w: 0, h: 0, dpr: 1}

  const resize = () => {
    size.dpr = Math.min(window.devicePixelRatio || 1, 2)
    size.w = stage.clientWidth
    size.h = stage.clientHeight
    canvas.width = Math.round(size.w * size.dpr)
    canvas.height = Math.round(size.h * size.dpr)
    ctx.setTransform(size.dpr, 0, 0, size.dpr, 0, 0)
  }
  resize()
  const observer = new ResizeObserver(resize)
  observer.observe(stage)

  const frames = frameLoop()
  return {
    canvas,
    ctx,
    size,
    loop: frames.start,
    destroy() {
      frames.stop()
      observer.disconnect()
      canvas.remove()
    }
  }
}

// Calls draw(dt, t) every animation frame, both in seconds, dt capped so a
// background tab coming back doesn't jump.
export function frameLoop() {
  let frame = null
  let last = null
  return {
    start(draw) {
      const tick = now => {
        const dt = last === null ? 0 : Math.min((now - last) / 1000, 0.1)
        last = now
        draw(dt, now / 1000)
        frame = requestAnimationFrame(tick)
      }
      frame = requestAnimationFrame(tick)
    },
    stop() {
      if (frame !== null) cancelAnimationFrame(frame)
    }
  }
}

export const lerp = (a, b, t) => a + (b - a) * t
export const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v))
export const easeOut = t => 1 - Math.pow(1 - t, 3)
export const random = (lo, hi) => lo + Math.random() * (hi - lo)
export const pick = list => list[Math.floor(Math.random() * list.length)]

// Where an event starts on screen, in pixels. The user's own actions come
// from the bottom, near where they act; other people's from anywhere above.
export function originOf(event, size) {
  if (event.at) return {x: event.at.x * size.w, y: event.at.y * size.h}
  if (event.mine) return {x: size.w * random(0.35, 0.65), y: size.h * random(0.8, 0.95)}
  return {x: size.w * random(0.1, 0.9), y: size.h * random(0.1, 0.6)}
}

// A rough id for a presence event that came without one, like the Leave
// button, which stands for "someone".
export function presenceId(event) {
  return event.id || null
}
