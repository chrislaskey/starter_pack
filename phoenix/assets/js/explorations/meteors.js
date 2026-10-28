// Shooting stars: soft diagonal streaks crossing the page for events, in the
// event's color, a bright head with a long tapered tail sliding down and to
// the right. Join falls from the top edge, leave climbs away and fades, post
// is brighter and longer, react a tiny flick. Celebrate is a shower of twelve.
import {canvasStage, originOf, pick, random} from "./stage"

export default function meteors(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const streaks = []

  const launch = (color, x, y, extra = {}) => {
    const angle = random(0.35, 0.55)
    const s = {color, x, y, angle, speed: 700, len: 180, width: 2, alpha: 0.85, age: 0, life: 1.4, ...extra}
    s.nx = Math.cos(s.angle)
    s.ny = Math.sin(s.angle)
    streaks.push(s)
  }

  loop(dt => {
    ctx.clearRect(0, 0, size.w, size.h)
    for (let i = streaks.length - 1; i >= 0; i--) {
      const s = streaks[i]
      s.age += dt
      if (s.age < 0) continue
      const t = s.age / s.life
      if (t >= 1) {
        streaks.splice(i, 1)
        continue
      }
      s.x += s.nx * s.speed * dt
      s.y += s.ny * s.speed * dt
      const alpha = s.alpha * Math.min(1, s.age / 0.15) * (1 - t) ** 1.5
      const tx = s.x - s.nx * s.len
      const ty = s.y - s.ny * s.len
      const px = -s.ny * s.width
      const py = s.nx * s.width
      const tail = ctx.createLinearGradient(s.x, s.y, tx, ty)
      tail.addColorStop(0, palette.rgba(s.color, alpha))
      tail.addColorStop(1, palette.rgba(s.color, 0))
      ctx.beginPath()
      ctx.moveTo(s.x + px, s.y + py)
      ctx.lineTo(tx, ty)
      ctx.lineTo(s.x - px, s.y - py)
      ctx.closePath()
      ctx.fillStyle = tail
      ctx.fill()
      const glow = ctx.createRadialGradient(s.x, s.y, 0, s.x, s.y, s.width * 4)
      glow.addColorStop(0, palette.rgba(s.color, alpha))
      glow.addColorStop(0.35, palette.rgba(s.color, alpha * 0.4))
      glow.addColorStop(1, palette.rgba(s.color, 0))
      ctx.beginPath()
      ctx.arc(s.x, s.y, s.width * 4, 0, Math.PI * 2)
      ctx.fillStyle = glow
      ctx.fill()
    }
  })

  return {
    fire(event) {
      const color = palette.colorOf(event.kind)
      switch (event.kind) {
        case "join":
          launch(color, size.w * random(0.1, 0.7), -10)
          break
        case "leave": {
          const {x, y} = originOf(event, size)
          launch(color, x, y, {angle: Math.PI + random(0.35, 0.55), speed: 500, len: 140, alpha: 0.5, life: 1.2})
          break
        }
        case "post":
          launch(color, size.w * random(0.05, 0.5), size.h * random(0.05, 0.3), {speed: 900, len: 320, width: 3, alpha: 1, life: 1.6})
          break
        case "react": {
          const {x, y} = originOf(event, size)
          launch(color, x, y, {speed: 450, len: 70, width: 1.5, alpha: 0.7, life: 0.7})
          break
        }
        case "celebrate":
          for (let i = 0; i < 12; i++) {
            launch(pick(palette.trio), size.w * random(-0.1, 0.8), -10, {age: -random(0, 1), speed: random(650, 900), len: random(160, 260)})
          }
      }
    },
    destroy
  }
}
