// A small solar system in the bottom-right corner: you are the magenta core
// and everyone else is a satellite on its own faint orbit, swinging across
// the corner. Joining spirals in from far out and leaving spirals away, a
// post flares with a comet tail (the core flares when it is yours), a
// reaction sparks, and celebrate flashes each ring in turn in the trio.
import {canvasStage, easeOut, lerp, random} from "./stage"

const TAU = Math.PI * 2

export default function orbit(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const satellites = new Map()
  const flashes = []
  const core = {flare: 0}
  const center = () => ({x: size.w - 28, y: size.h - 28})
  const radiusOf = slot => 100 + slot * 58

  const add = id => {
    if (satellites.has(id)) return
    const taken = new Set([...satellites.values()].map(s => s.slot))
    let slot = 0
    while (taken.has(slot)) slot++
    satellites.set(id, {
      id, slot, r: radiusOf(slot) * 2.4, phase: random(0, TAU), speed: 0.9 / Math.sqrt(slot + 1),
      alpha: 0, flare: 0, spark: 0, trail: [], leaving: false
    })
  }

  const place = (sat, c) => {
    const angle = Math.PI + (Math.PI / 2) * (0.5 + 0.5 * Math.sin(sat.phase))
    return {x: c.x + Math.cos(angle) * sat.r, y: c.y + Math.sin(angle) * sat.r, depth: Math.cos(sat.phase)}
  }

  const glowDot = (x, y, radius, color, alpha) => {
    const g = ctx.createRadialGradient(x, y, 0, x, y, radius)
    g.addColorStop(0, palette.rgba(color, alpha))
    g.addColorStop(0.3, palette.rgba(color, alpha * 0.4))
    g.addColorStop(1, palette.rgba(color, 0))
    ctx.fillStyle = g
    ctx.beginPath()
    ctx.arc(x, y, radius, 0, TAU)
    ctx.fill()
  }

  loop(dt => {
    ctx.clearRect(0, 0, size.w, size.h)
    const c = center()
    const dim = palette.dark() ? 1 : 0.8

    for (const sat of satellites.values()) {
      const ring = radiusOf(sat.slot)
      ctx.beginPath()
      ctx.arc(c.x, c.y, ring, 0, TAU)
      ctx.lineWidth = 1
      ctx.strokeStyle = palette.rgba(palette.purple, 0.12 * sat.alpha * dim)
      ctx.stroke()
    }
    for (let i = flashes.length - 1; i >= 0; i--) {
      const flash = flashes[i]
      flash.age += dt
      if (flash.age < 0) continue
      const p = flash.age / flash.life
      if (p >= 1) {
        flashes.splice(i, 1)
        continue
      }
      ctx.beginPath()
      ctx.arc(c.x, c.y, flash.ring, 0, TAU)
      ctx.lineWidth = 2.5
      ctx.strokeStyle = palette.rgba(flash.color, 0.7 * Math.sin(Math.PI * p))
      ctx.stroke()
    }

    core.flare = Math.max(0, core.flare - dt * 0.9)
    glowDot(c.x, c.y, 70 + core.flare * 60, palette.magenta, (0.5 + core.flare * 0.5) * dim)
    glowDot(c.x, c.y, 12 + core.flare * 6, palette.magenta, 0.95)

    for (const sat of satellites.values()) {
      sat.phase += dt * sat.speed * (1 + (sat.alpha < 1 && !sat.leaving ? 1.5 : 0))
      sat.alpha = sat.leaving ? Math.max(0, sat.alpha - dt * 0.5) : Math.min(1, sat.alpha + dt * 0.5)
      if (sat.leaving && sat.alpha === 0) {
        satellites.delete(sat.id)
        continue
      }
      const target = sat.leaving ? radiusOf(sat.slot) * 2.6 : radiusOf(sat.slot)
      sat.r = lerp(sat.r, target, 1 - Math.exp(-dt * (sat.leaving ? 0.6 : 1.3)))
      sat.flare = Math.max(0, sat.flare - dt)
      sat.spark = Math.max(0, sat.spark - dt * 2)
      const {x, y, depth} = place(sat, c)

      if (sat.flare > 0) sat.trail.push({x, y, age: 0})
      for (let i = sat.trail.length - 1; i >= 0; i--) {
        sat.trail[i].age += dt
        if (sat.trail[i].age > 0.9) sat.trail.splice(i, 1)
      }
      for (let i = 1; i < sat.trail.length; i++) {
        const a = sat.trail[i - 1]
        const b = sat.trail[i]
        ctx.beginPath()
        ctx.moveTo(a.x, a.y)
        ctx.lineTo(b.x, b.y)
        ctx.lineWidth = 4 * (1 - b.age / 0.9)
        ctx.strokeStyle = palette.rgba(palette.magenta, 0.6 * (1 - b.age / 0.9) * sat.alpha)
        ctx.stroke()
      }

      const near = 0.75 + 0.25 * depth
      const alpha = sat.alpha * near * dim
      const color = sat.flare > 0 ? palette.magenta : palette.blue
      glowDot(x, y, (18 + sat.flare * 16) * near, color, 0.6 * alpha + sat.flare * 0.4)
      ctx.fillStyle = palette.rgba(color, Math.min(1, alpha + 0.2))
      ctx.beginPath()
      ctx.arc(x, y, (3 + sat.flare * 1.5) * near, 0, TAU)
      ctx.fill()
      if (sat.spark > 0) {
        const p = 1 - sat.spark
        ctx.beginPath()
        ctx.arc(x, y, 4 + easeOut(p) * 16, 0, TAU)
        ctx.lineWidth = 1.5
        ctx.strokeStyle = palette.rgba(palette.lavender, 0.8 * (1 - p) * sat.alpha)
        ctx.stroke()
        glowDot(x, y, 14, palette.lavender, 0.8 * (1 - p) * sat.alpha)
      }
    }
  })

  return {
    fire(event) {
      if (event.kind === "celebrate") {
        const slots = [...satellites.values()].map(s => s.slot).sort((a, b) => a - b)
        slots.forEach((slot, i) => flashes.push({ring: radiusOf(slot), color: palette.trio[i % 3], age: -i * 0.16, life: 0.9}))
        if (slots.length === 0) core.flare = 1
        return
      }
      if (event.mine) {
        if (event.kind === "post") core.flare = 1
        if (event.kind === "react") core.flare = Math.max(core.flare, 0.35)
        return
      }
      if (event.kind === "join") return void add(event.id || `guest-${random(0, 1e6) | 0}`)
      const sat = event.id ? satellites.get(event.id) : [...satellites.values()].filter(s => !s.leaving).pop()
      if (!sat) return
      if (event.kind === "leave") sat.leaving = true
      if (event.kind === "post") sat.flare = 1
      if (event.kind === "react") sat.spark = 1
    },
    destroy
  }
}
