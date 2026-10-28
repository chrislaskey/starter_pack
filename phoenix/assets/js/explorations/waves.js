// Three waves along the bottom of the page, one per palette color, drifting
// on noise. An event drops a ripple into them that spreads out and settles;
// the wave in the event's color heaves the most. Celebrate lifts all three.
//
// `makeWaves(options)` is the same drawing with knobs, for the variations:
//   rest   "drift" keeps moving, "slow" barely moves, "still" doesn't move
//          at all until an event, then damps back to rest
//   tide   true makes height and speed follow the room's activity
//   style  "fill" stacks filled waves, "line" draws thin overlapping lines
//          that thicken where a ripple passes
import * as d3 from "d3"
import {createNoise2D} from "simplex-noise"
import {frameLoop, originOf, random} from "./stage"

const restSpeed = {drift: 1, slow: 0.08, still: 0}

export function makeWaves({rest = "drift", tide = false, style = "fill"} = {}) {
  return function waves(stage, palette) {
    const svg = d3.select(stage).append("svg").attr("width", "100%").attr("height", "100%")
    const noise = createNoise2D()
    const line = style === "line"
    const layers = palette.trio.map((color, i) => ({
      color,
      path: line
        ? svg.append("path").attr("fill", "none").attr("stroke", color).attr("stroke-linejoin", "round")
        : svg.append("path").attr("fill", color).attr("fill-opacity", 0.22),
      height: line ? 90 + i * 10 : 70 + i * 28,
      amplitude: 14 + i * 4,
      speed: 0.25 + i * 0.08,
      seed: i * 100
    }))
    const area = d3.area().curve(d3.curveBasis).x(d => d.x).y1(d => d.y)
    const stroke = d3.line().curve(d3.curveBasis).x(d => d.x).y(d => d.y)
    const ripples = []
    const steps = 80
    let clock = random(0, 100)
    let energy = 0

    const frames = frameLoop()
    let drawn = null

    frames.start(dt => {
      const w = stage.clientWidth
      const h = stage.clientHeight
      energy = energy < 0.002 ? 0 : energy * Math.exp(-dt * 0.7)
      // At rest in "still" there is nothing to redraw, so the frame costs nothing
      const resting = restSpeed[rest] === 0 && energy === 0 && ripples.length === 0
      if (resting && drawn === `${w}x${h}`) return
      drawn = `${w}x${h}`
      clock += dt * (restSpeed[rest] + energy * (rest === "drift" ? 0.4 : 1.2))
      const lift = tide ? energy * 36 : 0
      const swell = tide ? 1 + energy * 1.2 : 1
      area.y0(h)
      for (let i = ripples.length - 1; i >= 0; i--) {
        ripples[i].age += dt
        if (ripples[i].age > ripples[i].life) ripples.splice(i, 1)
      }
      for (const layer of layers) {
        const points = []
        let thick = 0
        for (let s = 0; s <= steps; s++) {
          const x = (w * s) / steps
          const drift = noise(s * 0.08 + layer.seed, clock * layer.speed) * layer.amplitude * swell
          let y = h - layer.height - lift + drift
          for (const r of ripples) {
            const spread = 60 + r.age * 420
            const distance = Math.abs(x - r.x)
            const fade = Math.exp(-r.age * 1.6) * Math.exp(-((distance / spread) ** 2))
            const match = r.color === layer.color ? 1 : 0.45
            y -= Math.sin(r.age * 9 - distance * 0.03) * r.strength * fade * match
            thick = Math.max(thick, fade * match * (r.strength / 40))
          }
          points.push({x, y})
        }
        if (line) {
          layer.path
            .attr("d", stroke(points))
            .attr("stroke-width", 1.2 + thick * 5)
            .attr("stroke-opacity", Math.min(1, (palette.dark() ? 0.6 : 0.5) + thick * 0.5))
        } else {
          layer.path.attr("d", area(points))
        }
      }
    })

    return {
      fire(event) {
        const {x} = originOf(event, {w: stage.clientWidth, h: stage.clientHeight})
        const strength = {join: 40, leave: 22, post: 55, react: 16, celebrate: 90}[event.kind] || 30
        energy = Math.min(3, energy + strength / 45)
        if (event.kind === "celebrate") {
          palette.trio.forEach((color, i) =>
            ripples.push({x: stage.clientWidth * (0.2 + i * 0.3), age: -i * 0.15, life: 3.5, strength, color})
          )
        } else {
          ripples.push({x, age: 0, life: 3, strength, color: palette.colorOf(event.kind)})
        }
      },
      destroy() {
        frames.stop()
        svg.remove()
      }
    }
  }
}

export default makeWaves()
