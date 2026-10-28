// An ECG trace scrolling left along the bottom, flat until something
// happens. Each event writes a shape at the right edge in its color: react
// a small bump, join a spike, leave a dip, post a full PQRST beat, and
// celebrate three beats in the trio colors. Old trace fades out on the left.
import {canvasStage, clamp} from "./stage"

const bump = (u, a, b, height) => (u < a || u > b ? 0 : Math.sin((Math.PI * (u - a)) / (b - a)) * height)
const spike = (u, a, b, height) => (u < a || u > b ? 0 : (1 - Math.abs((u - a) / (b - a) - 0.5) * 2) * height)
const beat = u => bump(u, 0.08, 0.26, 0.16) + spike(u, 0.34, 0.4, -0.12) + spike(u, 0.4, 0.5, 1.1) +
  spike(u, 0.5, 0.58, -0.32) + bump(u, 0.68, 0.92, 0.3)
const shapes = {
  react: {life: 0.4, f: u => bump(u, 0.1, 0.9, 0.24)},
  join: {life: 0.3, f: u => spike(u, 0.2, 0.8, 0.8)},
  leave: {life: 0.35, f: u => spike(u, 0.2, 0.8, -0.5)},
  post: {life: 1, f: beat},
  celebrate: {life: 0.7, f: u => beat(clamp(u * 1.1, 0, 1))}
}

export default function heartbeat(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const speed = 110
  const step = 1 / 180
  const samples = []
  const queue = []
  let now = size.w / speed + 1
  let cursor = 0
  let current = null

  const sample = ts => {
    if (!current && queue.length) current = {...queue.shift(), start: ts}
    if (!current) return {t: ts, y: 0, color: palette.purple, hot: false}
    const u = (ts - current.start) / current.life
    if (u >= 1) {
      current = null
      return sample(ts)
    }
    return {t: ts, y: current.f(u), color: current.color, hot: true}
  }

  loop(dt => {
    now += dt
    while (cursor < now) {
      cursor += step
      samples.push(sample(cursor))
    }
    while (samples.length && size.w - (now - samples[0].t) * speed < -2) samples.shift()
    ctx.clearRect(0, 0, size.w, size.h)
    const base = size.h - 50
    const fade = size.w * 0.3
    const dim = palette.dark() ? 0.45 : 0.4
    const gradients = {}
    const gradient = (color, alpha) => {
      const key = color + alpha
      if (!gradients[key]) {
        gradients[key] = ctx.createLinearGradient(0, 0, fade, 0)
        gradients[key].addColorStop(0, palette.rgba(color, 0))
        gradients[key].addColorStop(1, palette.rgba(color, alpha))
      }
      return gradients[key]
    }
    ctx.lineWidth = 1.5
    ctx.lineJoin = "round"
    let run = null
    let prev = null
    const stroke = () => {
      ctx.strokeStyle = gradient(run.color, run.hot ? 0.85 : dim)
      ctx.stroke()
    }
    for (const s of samples) {
      const x = size.w - (now - s.t) * speed
      const y = base - s.y * 40
      if (!run || run.color !== s.color || run.hot !== s.hot) {
        if (run) stroke()
        run = {color: s.color, hot: s.hot}
        ctx.beginPath()
        if (prev) ctx.moveTo(prev.x, prev.y)
        else ctx.moveTo(x, y)
      }
      ctx.lineTo(x, y)
      prev = {x, y}
    }
    if (run) stroke()
  })

  return {
    fire(event) {
      const shape = shapes[event.kind]
      if (!shape) return
      if (event.kind === "celebrate") {
        palette.trio.forEach(color => queue.push({...shape, color}))
      } else {
        queue.push({...shape, color: palette.colorOf(event.kind)})
      }
    },
    destroy
  }
}
