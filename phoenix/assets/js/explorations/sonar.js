// A small radar screen in the bottom-left corner, the least distracting
// idea here: faint range rings, a slow sweep that trails a fading wedge,
// one blip per person present at a steady bearing (you at the center) and
// the head count underneath. Joining blinks a blip in with a small ping,
// leaving blinks it out, a post pings out from the blip in magenta, a
// reaction brightens it for a beat, and celebrate rings out the trio.
import {easeOut, frameLoop, random} from "./stage"

const TAU = Math.PI * 2
const side = 150
const cx = side / 2
const cy = 66
const R = 56

export default function sonar(stage, palette) {
  const canvas = document.createElement("canvas")
  const dpr = Math.min(window.devicePixelRatio || 1, 2)
  canvas.width = side * dpr
  canvas.height = side * dpr
  canvas.style.cssText = `position:absolute;left:16px;bottom:16px;width:${side}px;height:${side}px`
  stage.appendChild(canvas)
  const ctx = canvas.getContext("2d")
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)

  const blips = new Map()
  const pings = []
  blips.set("me", {id: "me", mine: true, x: cx, y: cy, alpha: 1, blink: 0, bright: 0, leaving: false})

  const add = id => {
    if (blips.has(id)) return
    const angle = random(0, TAU)
    const r = random(0.3, 0.9) * R
    const blip = {id, mine: false, x: cx + Math.cos(angle) * r, y: cy + Math.sin(angle) * r, alpha: 0, blink: 0.6, bright: 0, leaving: false}
    blips.set(id, blip)
    pings.push({x: blip.x, y: blip.y, age: 0, life: 0.9, reach: 14, color: palette.blue})
  }

  const frames = frameLoop()
  frames.start((dt, t) => {
    ctx.clearRect(0, 0, side, side)
    const dim = palette.dark() ? 1 : 0.8
    const sweep = (t * 0.7) % TAU

    for (const f of [0.33, 0.66, 1]) {
      ctx.beginPath()
      ctx.arc(cx, cy, R * f, 0, TAU)
      ctx.lineWidth = f === 1 ? 1 : 0.5
      ctx.strokeStyle = palette.rgba(palette.purple, (f === 1 ? 0.3 : 0.15) * dim)
      ctx.stroke()
    }
    ctx.beginPath()
    ctx.moveTo(cx - R, cy)
    ctx.lineTo(cx + R, cy)
    ctx.moveTo(cx, cy - R)
    ctx.lineTo(cx, cy + R)
    ctx.lineWidth = 0.5
    ctx.strokeStyle = palette.rgba(palette.purple, 0.1 * dim)
    ctx.stroke()

    const slices = 24
    const span = 1.3
    for (let i = 0; i < slices; i++) {
      const a = sweep - (i * span) / slices
      ctx.beginPath()
      ctx.moveTo(cx, cy)
      ctx.arc(cx, cy, R, a - span / slices - 0.01, a)
      ctx.fillStyle = palette.rgba(palette.blue, 0.16 * (1 - i / slices) ** 2 * dim)
      ctx.fill()
    }
    ctx.beginPath()
    ctx.moveTo(cx, cy)
    ctx.lineTo(cx + Math.cos(sweep) * R, cy + Math.sin(sweep) * R)
    ctx.lineWidth = 1
    ctx.strokeStyle = palette.rgba(palette.blue, 0.45 * dim)
    ctx.stroke()

    for (const blip of blips.values()) {
      blip.blink = Math.max(0, blip.blink - dt)
      blip.bright = Math.max(0, blip.bright - dt * 2)
      if (blip.leaving) {
        blip.alpha = Math.max(0, blip.alpha - dt * 1.2)
        if (blip.alpha === 0) {
          blips.delete(blip.id)
          continue
        }
      } else {
        blip.alpha = Math.min(1, blip.alpha + dt * 1.5)
      }
      const flicker = blip.blink > 0 ? 0.5 + 0.5 * Math.sin(blip.blink * 40) : 1
      const behind = (sweep - Math.atan2(blip.y - cy, blip.x - cx) + TAU) % TAU
      const lit = blip.mine ? 0 : Math.exp(-behind * 2) * 0.5
      const color = blip.mine ? palette.magenta : palette.blue
      const alpha = blip.alpha * flicker * (0.55 + lit + blip.bright * 0.45) * dim
      const radius = (blip.mine ? 3 : 2.2) + blip.bright * 1.5
      const g = ctx.createRadialGradient(blip.x, blip.y, 0, blip.x, blip.y, radius * 3)
      g.addColorStop(0, palette.rgba(color, alpha * 0.5))
      g.addColorStop(1, palette.rgba(color, 0))
      ctx.fillStyle = g
      ctx.beginPath()
      ctx.arc(blip.x, blip.y, radius * 3, 0, TAU)
      ctx.fill()
      ctx.fillStyle = palette.rgba(color, Math.min(1, alpha))
      ctx.beginPath()
      ctx.arc(blip.x, blip.y, radius, 0, TAU)
      ctx.fill()
    }

    for (let i = pings.length - 1; i >= 0; i--) {
      const ping = pings[i]
      ping.age += dt
      if (ping.age < 0) continue
      const p = ping.age / ping.life
      if (p >= 1) {
        pings.splice(i, 1)
        continue
      }
      ctx.beginPath()
      ctx.arc(ping.x, ping.y, 2 + easeOut(p) * ping.reach, 0, TAU)
      ctx.lineWidth = 1.2 * (1 - p) + 0.3
      ctx.strokeStyle = palette.rgba(ping.color, 0.7 * (1 - p) * dim)
      ctx.stroke()
    }

    ctx.font = "11px sans-serif"
    ctx.textAlign = "center"
    ctx.fillStyle = palette.rgba(palette.lavender, 0.7 * dim)
    ctx.fillText(`${blips.size} here`, cx, side - 10)
  })

  return {
    fire(event) {
      if (event.kind === "celebrate") {
        palette.trio.forEach((color, i) => pings.push({x: cx, y: cy, age: -i * 0.2, life: 1.4, reach: R, color}))
        return
      }
      const id = event.mine ? "me" : event.id
      if (event.kind === "join") return void add(id || `guest-${random(0, 1e6) | 0}`)
      const blip = id ? blips.get(id) : [...blips.values()].filter(b => !b.mine && !b.leaving).pop()
      if (!blip) return
      if (event.kind === "leave" && !blip.mine) {
        blip.leaving = true
        blip.blink = 0.5
      }
      if (event.kind === "react") blip.bright = 1
      if (event.kind === "post") {
        blip.bright = 1
        pings.push({x: blip.x, y: blip.y, age: 0, life: 1.1, reach: 26, color: palette.magenta})
      }
    },
    destroy() {
      frames.stop()
      canvas.remove()
    }
  }
}
