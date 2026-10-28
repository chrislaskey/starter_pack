// Bubbles rising from the bottom edge, one or a few per event, in its color:
// a thin ring with a small highlight, wobbling as it climbs, slowing near
// the top and popping somewhere along the way. Join rises on the left, leave
// sinks down the right, and celebrate is a stream in all three colors.
import {canvasStage, easeOut, originOf, pick, random} from "./stage"

export default function bubbles(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const bubbles = []
  const pops = []

  const blow = (color, x, r, extra = {}) =>
    bubbles.push({
      color, x, r,
      y: size.h + r,
      dir: -1,
      alpha: 1,
      popAt: random(0.12, 0.55),
      phase: random(0, Math.PI * 2),
      wobble: random(8, 18),
      speed: random(70, 110),
      age: 0,
      ...extra
    })

  loop(dt => {
    ctx.clearRect(0, 0, size.w, size.h)
    for (let i = bubbles.length - 1; i >= 0; i--) {
      const b = bubbles[i]
      b.age += dt
      if (b.age < 0) continue
      const progress = b.dir < 0 ? b.y / size.h : 1 - b.y / size.h
      b.y += b.dir * b.speed * (0.3 + 0.7 * progress) * dt
      const x = b.x + Math.sin(b.age * 2.2 + b.phase) * b.wobble
      if (progress <= b.popAt) {
        bubbles.splice(i, 1)
        pops.push({x, y: b.y, r: b.r, color: b.color, age: 0})
        continue
      }
      const fade = b.dir > 0 ? Math.min(1, progress * 3) : 1
      const alpha = Math.min(1, b.age / 0.4) * fade * b.alpha
      ctx.beginPath()
      ctx.arc(x, b.y, b.r, 0, Math.PI * 2)
      ctx.fillStyle = palette.rgba(b.color, alpha * 0.08)
      ctx.fill()
      ctx.lineWidth = 1.5
      ctx.strokeStyle = palette.rgba(b.color, alpha * 0.7)
      ctx.stroke()
      ctx.beginPath()
      ctx.arc(x, b.y, b.r * 0.68, -Math.PI * 0.8, -Math.PI * 0.55)
      ctx.lineWidth = 2
      ctx.lineCap = "round"
      ctx.strokeStyle = palette.rgba(palette.lavender, alpha * 0.9)
      ctx.stroke()
    }
    for (let i = pops.length - 1; i >= 0; i--) {
      const p = pops[i]
      p.age += dt
      const t = p.age / 0.45
      if (t >= 1) {
        pops.splice(i, 1)
        continue
      }
      ctx.beginPath()
      ctx.arc(p.x, p.y, p.r * (1 + easeOut(t) * 0.8), 0, Math.PI * 2)
      ctx.lineWidth = 1.5 * (1 - t)
      ctx.strokeStyle = palette.rgba(p.color, 0.7 * (1 - t))
      ctx.stroke()
    }
  })

  return {
    fire(event) {
      const color = palette.colorOf(event.kind)
      const {x} = originOf(event, size)
      switch (event.kind) {
        case "post":
          blow(color, x, random(22, 30))
          blow(color, x + random(-50, 50), random(8, 12), {age: -0.2})
          blow(color, x + random(-50, 50), random(8, 12), {age: -0.4})
          break
        case "react":
          blow(color, x, random(8, 11))
          break
        case "join":
          blow(color, size.w * random(0.05, 0.2), random(14, 18))
          break
        case "leave":
          blow(color, size.w * random(0.8, 0.95), random(14, 18), {y: size.h * 0.25, dir: 1, alpha: 0.7, popAt: 0.03})
          break
        case "celebrate":
          for (let i = 0; i < 20; i++) {
            blow(pick(palette.trio), size.w * random(0.05, 0.95), random(8, 20), {age: -random(0, 1)})
          }
      }
    },
    destroy
  }
}
