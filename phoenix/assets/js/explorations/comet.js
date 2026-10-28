// A thin bar across the very top of the page, faint purple, like a progress
// bar that never loads. Each event sends a comet along it in its color, a
// bright head with a long fading tail: your own run left to right, other
// people's right to left. Post also flashes the bar; celebrate is three comets.
import {canvasStage} from "./stage"

export default function comet(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const comets = []
  let flash = 0

  loop(dt => {
    ctx.clearRect(0, 0, size.w, size.h)
    flash = Math.max(0, flash - dt / 0.7)
    ctx.fillStyle = palette.rgba(palette.purple, 0.15 + flash * 0.3)
    ctx.fillRect(0, 0, size.w, 3)
    ctx.globalCompositeOperation = "lighter"
    for (let i = comets.length - 1; i >= 0; i--) {
      const c = comets[i]
      c.age += dt
      if (c.age < 0) continue
      const t = c.age / c.life
      if (t >= 1) {
        comets.splice(i, 1)
        continue
      }
      const span = size.w + c.tail * 2
      const head = c.dir > 0 ? span * t - c.tail : size.w + c.tail - span * t
      const back = head - c.dir * c.tail
      const left = Math.min(head, back)
      const tail = ctx.createLinearGradient(head, 0, back, 0)
      tail.addColorStop(0, palette.rgba(c.color, c.alpha))
      tail.addColorStop(1, palette.rgba(c.color, 0))
      ctx.fillStyle = tail
      ctx.fillRect(left, 0, c.tail, 3)
      ctx.globalAlpha = 0.3
      ctx.fillRect(left, -3, c.tail, 9)
      ctx.globalAlpha = 1
      const glow = ctx.createRadialGradient(head, 1.5, 0, head, 1.5, c.glow)
      glow.addColorStop(0, palette.rgba(c.color, c.alpha))
      glow.addColorStop(0.3, palette.rgba(c.color, c.alpha * 0.5))
      glow.addColorStop(1, palette.rgba(c.color, 0))
      ctx.fillStyle = glow
      ctx.fillRect(head - c.glow, 1.5 - c.glow, c.glow * 2, c.glow * 2)
    }
    ctx.globalCompositeOperation = "source-over"
  })

  const send = (color, mine, extra = {}) =>
    comets.push({color, dir: mine ? 1 : -1, age: 0, life: 1.4, tail: size.w * 0.3, alpha: 0.9, glow: 14, ...extra})

  return {
    fire(event) {
      const color = palette.colorOf(event.kind)
      switch (event.kind) {
        case "post":
          flash = 1
          send(color, event.mine, {alpha: 1, tail: size.w * 0.4, glow: 18})
          break
        case "leave":
          send(color, event.mine, {alpha: 0.5, tail: size.w * 0.15, glow: 9, life: 1.2})
          break
        case "react":
          send(color, event.mine, {alpha: 0.7, tail: size.w * 0.2, glow: 10})
          break
        case "celebrate":
          palette.trio.forEach((c, i) => send(c, event.mine, {age: -i * 0.25, alpha: 1, glow: 16}))
          break
        default:
          send(color, event.mine)
      }
    },
    destroy
  }
}
