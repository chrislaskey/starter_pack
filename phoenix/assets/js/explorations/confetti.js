// The big reaction: nothing until something happens, then paper in the air.
// A post bursts forty pieces from where it landed, a reaction is a small
// sparkle, join a soft puff rising, leave a few pieces sinking away.
// Celebrate fires cannons from both bottom corners, up and inward.
import {canvasStage, originOf, pick, random} from "./stage"

export default function confetti(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const pieces = []

  const spawn = (count, at, look) => {
    for (let i = 0; i < count; i++) {
      const angle = random(look.angle[0], look.angle[1])
      const speed = random(look.speed[0], look.speed[1])
      const scale = look.scale || 1
      pieces.push({
        x: at.x,
        y: at.y,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        gravity: look.gravity,
        drag: look.drag ?? 1.2,
        rot: random(0, Math.PI * 2),
        spin: random(-6, 6),
        tumble: random(0, Math.PI * 2),
        flip: random(3, 9),
        w: random(5, 9) * scale,
        h: random(3, 6) * scale,
        dot: look.dots || Math.random() < 0.3,
        color: look.color || pick(palette.trio),
        age: 0,
        life: random(look.life[0], look.life[1])
      })
    }
  }

  loop(dt => {
    ctx.clearRect(0, 0, size.w, size.h)
    for (let i = pieces.length - 1; i >= 0; i--) {
      const p = pieces[i]
      p.age += dt
      if (p.age >= p.life) {
        pieces.splice(i, 1)
        continue
      }
      const slow = Math.exp(-p.drag * dt)
      p.vx *= slow
      p.vy = p.vy * slow + p.gravity * dt
      p.x += p.vx * dt
      p.y += p.vy * dt
      p.rot += p.spin * dt
      p.tumble += p.flip * dt
      const t = p.age / p.life
      const alpha = t < 0.6 ? 1 : 1 - (t - 0.6) / 0.4
      ctx.save()
      ctx.translate(p.x, p.y)
      ctx.fillStyle = palette.rgba(p.color, alpha * 0.9)
      if (p.dot) {
        ctx.beginPath()
        ctx.arc(0, 0, p.h / 2, 0, Math.PI * 2)
        ctx.fill()
      } else {
        ctx.rotate(p.rot)
        ctx.scale(1, Math.cos(p.tumble))
        ctx.fillRect(-p.w / 2, -p.h / 2, p.w, p.h)
      }
      ctx.restore()
    }
  })

  return {
    fire(event) {
      const at = originOf(event, size)
      const color = palette.colorOf(event.kind)
      const pi = Math.PI
      switch (event.kind) {
        case "post":
          spawn(40, at, {angle: [-pi * 1.1, pi * 0.1], speed: [180, 520], gravity: 900, life: [1.4, 2.4]})
          break
        case "react":
          spawn(6, at, {angle: [-pi, pi], speed: [60, 160], gravity: 300, life: [0.7, 1.1], scale: 0.7})
          break
        case "join":
          spawn(10, at, {angle: [-pi * 0.7, -pi * 0.3], speed: [90, 220], gravity: 90, drag: 1.8, life: [1.2, 1.8], dots: true, color})
          break
        case "leave":
          spawn(10, at, {angle: [pi * 0.3, pi * 0.7], speed: [30, 90], gravity: 60, drag: 0.4, life: [1.6, 2.4], color})
          break
        case "celebrate": {
          const cannon = {speed: [900, 1400], gravity: 1000, drag: 0.8, life: [2, 3]}
          spawn(40, {x: 0, y: size.h}, {...cannon, angle: [-pi * 0.42, -pi * 0.25]})
          spawn(40, {x: size.w, y: size.h}, {...cannon, angle: [-pi * 0.75, -pi * 0.58]})
        }
      }
    },
    destroy
  }
}
