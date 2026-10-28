// One star per person present, loosely clustered and drifting on noise,
// with faint lines joining stars that are near each other; you are the
// magenta one. Joining draws a star and its lines in, leaving fades them, a
// post sends a pulse of light down each line, a reaction flickers the
// star's lines once, and celebrate lights every line in the trio colors.
import {createNoise2D} from "simplex-noise"
import {canvasStage, easeOut, lerp, pick, random} from "./stage"

const TAU = Math.PI * 2
const reach = 170

export default function constellation(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const noise = createNoise2D()
  const stars = new Map()
  const pulses = []
  const lit = {age: 2, life: 2}

  const edgePoint = () => {
    const side = pick(["top", "bottom", "left", "right"])
    if (side === "top") return {x: random(0, size.w), y: -40}
    if (side === "bottom") return {x: random(0, size.w), y: size.h + 40}
    if (side === "left") return {x: -40, y: random(0, size.h)}
    return {x: size.w + 40, y: random(0, size.h)}
  }

  const add = (id, mine) => {
    if (stars.has(id)) return stars.get(id)
    const from = edgePoint()
    const star = {id, mine, x: from.x, y: from.y, seed: random(0, 1000), alpha: 0, flicker: 0, bright: 0, leaving: false}
    stars.set(id, star)
    return star
  }

  const me = add("me", true)
  me.x = size.w / 2
  me.y = size.h / 2

  const distance = (a, b) => Math.hypot(a.x - b.x, a.y - b.y)
  const neighbours = star => [...stars.values()].filter(o => o !== star && !o.leaving && distance(o, star) < reach)

  loop((dt, t) => {
    ctx.clearRect(0, 0, size.w, size.h)
    const base = palette.dark() ? 0.5 : 0.4
    const cx = size.w * (0.5 + 0.3 * noise(1, t * 0.03))
    const cy = size.h * (0.5 + 0.25 * noise(2, t * 0.03))
    lit.age = Math.min(lit.life, lit.age + dt)
    const glow = Math.sin(Math.PI * easeOut(lit.age / lit.life))

    for (const star of stars.values()) {
      if (star.leaving) {
        star.alpha = Math.max(0, star.alpha - dt * 0.8)
        if (star.alpha === 0) {
          stars.delete(star.id)
          continue
        }
      } else {
        star.alpha = Math.min(1, star.alpha + dt * 0.6)
      }
      const tx = cx + 140 * noise(star.seed, t * 0.06)
      const ty = cy + 120 * noise(star.seed + 5, t * 0.06 + 3)
      const k = 1 - Math.exp(-dt * 1.4)
      star.x = lerp(star.x, tx, k)
      star.y = lerp(star.y, ty, k)
      star.flicker = Math.max(0, star.flicker - dt)
      star.bright = Math.max(0, star.bright - dt)
    }

    const list = [...stars.values()]
    let n = 0
    for (let i = 0; i < list.length; i++) {
      for (let j = i + 1; j < list.length; j++) {
        const a = list[i]
        const b = list[j]
        const d = distance(a, b)
        if (d >= reach) continue
        const flick = Math.max(a.flicker, b.flicker)
        const flash = flick > 0 ? (0.5 + 0.5 * Math.sin(flick * 30)) * flick * 2 : 0
        const alpha = (1 - d / reach) * Math.min(a.alpha, b.alpha) * (base + flash + glow)
        const color = glow > 0.05 ? palette.trio[n++ % 3] : palette.lavender
        ctx.beginPath()
        ctx.moveTo(a.x, a.y)
        ctx.lineTo(b.x, b.y)
        ctx.lineWidth = 1 + glow * 1.2
        ctx.strokeStyle = palette.rgba(color, Math.min(1, alpha))
        ctx.stroke()
      }
    }

    for (let i = pulses.length - 1; i >= 0; i--) {
      const pulse = pulses[i]
      pulse.age += dt
      const p = pulse.age / pulse.life
      if (p >= 1 || !stars.has(pulse.from.id) || !stars.has(pulse.to.id)) {
        pulses.splice(i, 1)
        continue
      }
      const x = lerp(pulse.from.x, pulse.to.x, easeOut(p))
      const y = lerp(pulse.from.y, pulse.to.y, easeOut(p))
      const g = ctx.createRadialGradient(x, y, 0, x, y, 12)
      g.addColorStop(0, palette.rgba(palette.magenta, 0.9 * (1 - p * p)))
      g.addColorStop(1, palette.rgba(palette.magenta, 0))
      ctx.fillStyle = g
      ctx.beginPath()
      ctx.arc(x, y, 12, 0, TAU)
      ctx.fill()
    }

    for (const star of list) {
      const color = star.mine ? palette.magenta : palette.blue
      const scale = 1 + star.bright * 0.8
      const alpha = star.alpha * (base + 0.3 + star.bright * 0.4)
      const g = ctx.createRadialGradient(star.x, star.y, 0, star.x, star.y, 16 * scale)
      g.addColorStop(0, palette.rgba(color, alpha * 0.5))
      g.addColorStop(1, palette.rgba(color, 0))
      ctx.fillStyle = g
      ctx.beginPath()
      ctx.arc(star.x, star.y, 16 * scale, 0, TAU)
      ctx.fill()
      ctx.fillStyle = palette.rgba(color, Math.min(1, alpha + 0.2))
      ctx.beginPath()
      ctx.arc(star.x, star.y, 2.5 * scale, 0, TAU)
      ctx.fill()
    }
  })

  return {
    fire(event) {
      const id = event.mine ? "me" : event.id
      if (event.kind === "join") return void add(id || `guest-${random(0, 1e6) | 0}`, event.mine)
      if (event.kind === "celebrate") return void (lit.age = 0)
      const star = id ? stars.get(id) : [...stars.values()].filter(s => s.id !== "me" && !s.leaving).pop()
      if (!star) return
      if (event.kind === "leave") star.leaving = true
      if (event.kind === "react") star.flicker = 0.6
      if (event.kind === "post") {
        star.bright = 1
        for (const other of neighbours(star)) pulses.push({from: star, to: other, age: 0, life: 0.8})
      }
    },
    destroy
  }
}
