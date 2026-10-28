// Three thin lines flowing sideways across the lower-middle of the page,
// like Stripe's ribbons, one per palette color drifting on noise. An event
// sends a swell along them from where it started, brightest on the line in
// its color; leave pinches them flat instead, and celebrate makes all three
// swell and braid for a couple of seconds. The ribbon-* files are variations
// made with makeRibbon's knobs: how the lines rest, sit, thicken and blend.
import * as d3 from "d3"
import {createNoise2D} from "simplex-noise"
import {clamp, frameLoop, originOf} from "./stage"

const swellOf = {join: 1.4, leave: -0.9, post: 2.2, react: 0.7, celebrate: 1.6}
const bulgeOf = {join: 8, leave: -6, post: 14, react: 4, celebrate: 10}

// rest: "drift" | "slow" | "still", how the lines move with nothing happening
// level: "stacked" | "shared", separate bands or one band they cross in
// width: "stroke" | "band" | "silk", a line, a line whose width is the response, or wide translucent bands
// blend: multiply (light) or screen (dark) where the lines overlap
export function makeRibbon({rest = "drift", level = "stacked", width = "stroke", blend = false} = {}) {
  const restSpeed = {drift: 1, slow: 0.1, still: 0}[rest]
  const flowBoost = {drift: 0, slow: 1.2, still: 1.6}[rest]
  const silk = width === "silk"
  const band = width !== "stroke"
  const swellScale = width === "band" ? 0.3 : 1

  return function ribbon(stage, palette) {
    const svg = d3.select(stage).append("svg").attr("width", "100%").attr("height", "100%")
    const noise = createNoise2D()
    const steps = 90
    const lines = palette.trio.map((color, i) => ({
      color,
      path: svg.append("path").attr(band ? "fill" : "stroke", color).attr(band ? "stroke" : "fill", "none")
        .attr("stroke-linecap", "round"),
      level: level === "shared" ? 0.64 : 0.6 + i * 0.06,
      amplitude: level === "shared" ? 42 : 24 + i * 6,
      speed: 0.1 + i * 0.03,
      seed: i * 40,
      base: silk ? 18 + i * 11 : level === "shared" ? 2.5 : band ? 1 : 1.5,
      alpha: silk ? 0.12 + i * 0.03 : band ? 0.6 : 0.5,
      phase: new Float32Array(steps + 1),
      glow: 0
    }))
    const line = d3.line().curve(d3.curveBasis).x(d => d.x).y(d => d.y)
    const area = d3.area().curve(d3.curveBasis).x(d => d.x).y0(d => d.y - d.half).y1(d => d.y + d.half)
    const waves = []
    const braidLife = 2.6
    let braid = 0

    const frames = frameLoop()
    let drawn = null

    frames.start((dt, t) => {
      const w = stage.clientWidth
      const h = stage.clientHeight
      if (blend) svg.style("mix-blend-mode", palette.dark() ? "screen" : "multiply")
      braid = Math.max(0, braid - dt)
      const weave = Math.sin(Math.PI * clamp(Math.min(braid, braidLife - braid) / 0.6, 0, 0.5))
      for (let i = waves.length - 1; i >= 0; i--) {
        waves[i].age += dt
        if (waves[i].age > waves[i].life) waves.splice(i, 1)
      }
      // At rest in "still" there is nothing to redraw, so the frame costs nothing
      const resting = restSpeed === 0 && waves.length === 0 && braid === 0 && lines.every(l => l.glow === 0)
      if (resting && drawn === `${w}x${h}`) return
      drawn = `${w}x${h}`
      lines.forEach((l, i) => {
        l.glow = Math.max(0, l.glow - dt * 0.9)
        const points = []
        for (let s = 0; s <= steps; s++) {
          const x = (w * s) / steps
          let swell = 1, bulge = 0, flow = 0
          for (const wv of waves) {
            const near = Math.exp(-((Math.abs(x - wv.x) / (90 + wv.age * 360)) ** 2))
            const fade = Math.exp(-wv.age * 1.3) * near
            const match = wv.color === l.color ? 1 : 0.4
            swell += wv.swell * fade * match * swellScale
            bulge += wv.bulge * fade * (match === 1 ? 1 : 0.15)
            flow += Math.exp(-wv.age * 0.7) * near * flowBoost
          }
          l.phase[s] += dt * (restSpeed + flow)
          const p = l.phase[s]
          let y = h * l.level + noise(s * 0.05 - p * 0.2 + l.seed, p * l.speed) * l.amplitude * Math.max(0.05, swell)
          y += Math.sin(x * 0.011 + t * 2.4 + (i * Math.PI * 2) / 3) * 44 * weave
          const wide = silk ? l.base * (1 + bulge * 0.05) : Math.max(0.3, l.base + bulge)
          points.push({x, y, half: wide / 2})
        }
        const dim = palette.dark() ? 0.05 : 0
        if (band) {
          l.path.attr("d", area(points)).attr("fill-opacity", Math.min(1, l.alpha + dim + l.glow * (silk ? 0.15 : 0.3)))
        } else {
          l.path
            .attr("d", line(points))
            .attr("stroke-opacity", Math.min(1, l.alpha + dim + l.glow * 0.5))
            .attr("stroke-width", l.base + l.glow * 1.2 + weave * 0.8)
        }
      })
    })

    return {
      fire(event) {
        const {x} = originOf(event, {w: stage.clientWidth, h: stage.clientHeight})
        const kind = event.kind
        const wave = {age: 0, life: 4.5, swell: swellOf[kind] ?? 1, bulge: bulgeOf[kind] ?? 4}
        if (kind === "celebrate") {
          braid = braidLife
          palette.trio.forEach((color, i) => {
            waves.push({...wave, x: stage.clientWidth * (0.25 + i * 0.25), age: -i * 0.12, color})
            lines[i].glow = 1
          })
          return
        }
        const color = palette.colorOf(kind)
        waves.push({...wave, x, color})
        for (const l of lines) l.glow = Math.max(l.glow, l.color === color ? 1 : 0.25)
      },
      destroy() {
        frames.stop()
        svg.remove()
      }
    }
  }
}

export default makeRibbon({})
