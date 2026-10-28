// An activity chart along the bottom, scrolling left over thirty seconds
// like a seismograph: every event adds energy that slowly decays, and the
// line's height follows it with a little noise wobble. The fill tints
// toward the latest event's color and settles back to purple; leave draws
// a short dip, and celebrate flashes the whole area.
import * as d3 from "d3"
import {createNoise2D} from "simplex-noise"
import {frameLoop} from "./stage"

export default function seismograph(stage, palette) {
  const svg = d3.select(stage).append("svg").attr("width", "100%").attr("height", "100%")
  const gradient = svg.append("defs").append("linearGradient").attr("id", "seismo-fill")
    .attr("x1", 0).attr("y1", 0).attr("x2", 0).attr("y2", 1)
  const top = gradient.append("stop").attr("offset", "0%")
  const bottom = gradient.append("stop").attr("offset", "100%")
  const fill = svg.append("path").attr("fill", "url(#seismo-fill)")
  const edge = svg.append("path").attr("fill", "none").attr("stroke-width", 1.2)
  const flash = svg.append("rect").attr("x", 0).attr("width", "100%").attr("opacity", 0)
  const noise = createNoise2D()
  const area = d3.area().curve(d3.curveBasis).x(d => d.x).y1(d => d.y)
  const line = d3.line().curve(d3.curveBasis).x(d => d.x).y(d => d.y)
  const span = 30
  const step = 1 / 30
  const points = []
  let now = 0
  let cursor = 0
  let energy = 0
  let dip = 0
  let tint = palette.purple
  let shown = palette.purple

  const frames = frameLoop()
  frames.start((dt, t) => {
    const w = stage.clientWidth
    const h = stage.clientHeight
    now += dt
    energy *= Math.exp(-dt * 0.35)
    dip = Math.max(0, dip - dt * 2.2)
    while (cursor < now) {
      cursor += step
      const wobble = noise(cursor * 1.6, 0) * (3 + energy * 2.5)
      const height = 12 + 120 * (1 - Math.exp(-energy / 5)) + wobble
      points.push({t: cursor, v: Math.max(2, height * (1 - Math.sin(Math.PI * dip) * 0.8))})
    }
    while (points.length && now - points[0].t > span + 1) points.shift()
    const mix = Math.min(1, energy / 2.5)
    shown = d3.interpolateRgb(shown, d3.interpolateRgb(palette.purple, tint)(mix))(1 - Math.exp(-dt * 2))
    const alpha = palette.dark() ? 0.5 : 0.42
    top.attr("stop-color", shown).attr("stop-opacity", alpha)
    bottom.attr("stop-color", shown).attr("stop-opacity", 0.04)
    const shaped = points.map(p => ({x: w - ((now - p.t) / span) * w, y: h - p.v}))
    fill.attr("d", area.y0(h)(shaped))
    edge.attr("d", line(shaped)).attr("stroke", shown).attr("stroke-opacity", alpha + 0.2)
    flash.attr("y", h - 160).attr("height", 160)
  })

  return {
    fire(event) {
      const weight = {react: 1, join: 2, leave: 1, post: 3, celebrate: 6}[event.kind] || 1
      energy += weight
      tint = palette.colorOf(event.kind)
      if (event.kind === "leave") dip = 1
      if (event.kind === "celebrate") {
        flash.attr("fill", palette.magenta).interrupt().attr("opacity", 0.3)
          .transition().duration(900).ease(d3.easeCubicOut).attr("opacity", 0)
      }
    },
    destroy() {
      frames.stop()
      svg.remove()
    }
  }
}
