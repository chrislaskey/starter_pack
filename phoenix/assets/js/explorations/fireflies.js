// One firefly per person present, drifting across the page on noise, blue
// with a soft glow; the magenta one is you. Joining flies in from an edge
// and leaving drifts back off one, a post brightens and bursts a thin ring,
// a reaction twinkles, and celebrate pulls the swarm together and lets go.
import {createNoise2D} from "simplex-noise"
import {canvasStage, easeOut, lerp, pick, random} from "./stage"

const TAU = Math.PI * 2

export default function fireflies(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const noise = createNoise2D()
  const flies = new Map()
  const rings = []
  const gatherLife = 2.2
  let gather = 0

  const edgePoint = () => {
    const side = pick(["top", "bottom", "left", "right"])
    if (side === "top") return {x: random(0, size.w), y: -60}
    if (side === "bottom") return {x: random(0, size.w), y: size.h + 60}
    if (side === "left") return {x: -60, y: random(0, size.h)}
    return {x: size.w + 60, y: random(0, size.h)}
  }

  const add = (id, mine) => {
    if (flies.has(id)) return flies.get(id)
    const from = edgePoint()
    const fly = {id, mine, x: from.x, y: from.y, seed: random(0, 1000), alpha: 0, bright: 0, twinkle: 0, exit: null}
    flies.set(id, fly)
    return fly
  }

  const me = add("me", true)
  me.x = size.w / 2
  me.y = size.h * 0.7

  const drawFly = fly => {
    const color = fly.mine ? palette.magenta : palette.blue
    const flicker = fly.twinkle > 0 ? (0.5 + 0.5 * Math.sin(fly.twinkle * 45)) * (fly.twinkle / 0.5) : 0
    const scale = 1 + fly.bright * 0.9 + flicker * 0.7
    const glow = (fly.mine ? 30 : 24) * scale
    const alpha = fly.alpha * ((palette.dark() ? 0.7 : 0.55) + 0.4 * fly.bright + 0.3 * flicker)
    const g = ctx.createRadialGradient(fly.x, fly.y, 0, fly.x, fly.y, glow)
    g.addColorStop(0, palette.rgba(color, alpha))
    g.addColorStop(0.35, palette.rgba(color, alpha * 0.35))
    g.addColorStop(1, palette.rgba(color, 0))
    ctx.fillStyle = g
    ctx.beginPath()
    ctx.arc(fly.x, fly.y, glow, 0, TAU)
    ctx.fill()
    ctx.fillStyle = palette.rgba(color, Math.min(1, alpha + 0.3))
    ctx.beginPath()
    ctx.arc(fly.x, fly.y, 2.5 * scale, 0, TAU)
    ctx.fill()
  }

  loop((dt, t) => {
    ctx.clearRect(0, 0, size.w, size.h)
    gather = Math.max(0, gather - dt)
    const pull = gather > 0 ? Math.sin(Math.PI * (1 - gather / gatherLife)) : 0

    for (const fly of flies.values()) {
      let tx = size.w * (0.5 + 0.46 * noise(fly.seed, t * 0.05))
      let ty = size.h * (0.5 + 0.42 * noise(fly.seed + 50, t * 0.05 + 7))
      let rate = 1.2
      if (fly.exit) {
        ;({x: tx, y: ty} = fly.exit)
        rate = 0.7
        fly.alpha = Math.max(0, fly.alpha - dt * 0.6)
        if (fly.alpha === 0) {
          flies.delete(fly.id)
          continue
        }
      } else {
        if (pull > 0) {
          tx = lerp(tx, size.w / 2 + noise(fly.seed + 9, t * 0.8) * 70, pull * 0.92)
          ty = lerp(ty, size.h / 2 + noise(fly.seed + 19, t * 0.8) * 70, pull * 0.92)
          rate += pull * 3
        }
        fly.alpha = Math.min(1, fly.alpha + dt * 0.7)
      }
      const k = 1 - Math.exp(-dt * rate)
      fly.x = lerp(fly.x, tx, k)
      fly.y = lerp(fly.y, ty, k)
      fly.bright = Math.max(0, fly.bright - dt * 0.8)
      fly.twinkle = Math.max(0, fly.twinkle - dt)
      drawFly(fly)
    }

    for (let i = rings.length - 1; i >= 0; i--) {
      const ring = rings[i]
      ring.age += dt
      const p = ring.age / ring.life
      if (p >= 1) {
        rings.splice(i, 1)
        continue
      }
      ctx.beginPath()
      ctx.arc(ring.x, ring.y, 6 + easeOut(p) * ring.reach, 0, TAU)
      ctx.lineWidth = 1.5 * (1 - p) + 0.3
      ctx.strokeStyle = palette.rgba(ring.color, 0.6 * (1 - p))
      ctx.stroke()
    }
  })

  return {
    fire(event) {
      const id = event.mine ? "me" : event.id
      if (event.kind === "join") return void add(id || `guest-${random(0, 1e6) | 0}`, event.mine)
      if (event.kind === "celebrate") return void (gather = gatherLife)
      const fly = id ? flies.get(id) : [...flies.values()].filter(f => f.id !== "me" && !f.exit).pop()
      if (!fly) return
      if (event.kind === "leave") fly.exit = edgePoint()
      if (event.kind === "react") fly.twinkle = 0.5
      if (event.kind === "post") {
        fly.bright = 1
        rings.push({x: fly.x, y: fly.y, age: 0, life: 1.4, reach: 110, color: fly.mine ? palette.magenta : palette.blue})
      }
    },
    destroy
  }
}
