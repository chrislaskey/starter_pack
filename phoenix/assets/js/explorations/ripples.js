// Rings spreading out from a point, the way a drop lands on still water.
// Each event is one faint ring in its color; a post is bolder, and
// celebrate is three rings, one per palette color, a beat apart.
import {canvasStage, easeOut, originOf} from "./stage"

export default function ripples(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const rings = []

  loop(dt => {
    ctx.clearRect(0, 0, size.w, size.h)
    for (let i = rings.length - 1; i >= 0; i--) {
      const ring = rings[i]
      ring.age += dt
      if (ring.age < 0) continue
      const t = ring.age / ring.life
      if (t >= 1) {
        rings.splice(i, 1)
        continue
      }
      const radius = easeOut(t) * ring.reach
      ctx.beginPath()
      ctx.arc(ring.x, ring.y, radius, 0, Math.PI * 2)
      ctx.lineWidth = ring.width * (1 - t) + 0.5
      ctx.strokeStyle = palette.rgba(ring.color, ring.alpha * (1 - t))
      ctx.stroke()
    }
  })

  return {
    fire(event) {
      const {x, y} = originOf(event, size)
      const reach = Math.max(size.w, size.h) * 0.6
      const bold = event.kind === "post" || event.kind === "celebrate"
      const ring = {x, y, age: 0, life: 2.4, reach, width: bold ? 6 : 3, alpha: bold ? 0.7 : 0.45}
      if (event.kind === "celebrate") {
        palette.trio.forEach((color, i) => rings.push({...ring, color, age: -i * 0.18}))
      } else {
        rings.push({...ring, color: palette.colorOf(event.kind)})
      }
    },
    destroy
  }
}
