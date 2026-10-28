// Gooey blobs huddled in the bottom-left corner, one per person present,
// merged by an SVG goo filter and jiggling in place. Join slides a new blob
// in from off-screen, leave pulls one away and shrinks it, post swells the
// poster's blob, react pops a droplet off it, and celebrate bounces them all.
import * as d3 from "d3"
import {createNoise2D} from "simplex-noise"
import {frameLoop, random} from "./stage"

export default function goo(stage, palette) {
  const svg = d3.select(stage).append("svg").attr("width", "100%").attr("height", "100%")
  const filter = svg.append("defs").append("filter").attr("id", "goo-filter")
    .attr("x", "-50%").attr("y", "-50%").attr("width", "200%").attr("height", "200%")
  filter.append("feGaussianBlur").attr("in", "SourceGraphic").attr("stdDeviation", 9)
  filter.append("feColorMatrix").attr("values", "1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 18 -7")
  const group = svg.append("g").attr("opacity", 0.6).append("g").attr("filter", "url(#goo-filter)")
  const noise = createNoise2D()
  const blobs = []
  const drops = []
  let added = 0
  const corner = () => ({x: 150, y: stage.clientHeight - 150})

  const add = id => {
    const color = palette.trio[added++ % 3]
    const {y} = corner()
    const blob = {id, color, r: id === "me" ? 34 : 26, x: -80, y: y - random(-60, 60), pulse: 0, bounce: 2, scale: 0, leaving: false, hx: 0, hy: y,
      el: group.append("circle").attr("fill", color)}
    blobs.push(blob)
    return blob
  }
  const find = (id, mine) => blobs.find(b => !b.leaving && b.id === (mine ? "me" : id))
  const newest = () => [...blobs].reverse().find(b => !b.leaving && b.id !== "me")
  const actor = event => find(event.id, event.mine) || (event.mine ? add("me") : event.id ? add(event.id) : newest() || add("me"))
  add("me")

  const frames = frameLoop()
  frames.start((dt, t) => {
    const {x: cx, y: cy} = corner()
    const ease = 1 - Math.exp(-dt * 3.2)
    let k = 0
    for (let i = blobs.length - 1; i >= 0; i--) {
      const b = blobs[i]
      let hx = cx, hy = cy, target = 1
      if (b.leaving) {
        hx = b.hx + (b.hx - cx) * 1.5 - 90
        hy = b.hy + (b.hy - cy) * 1.5 + 60
        target = 0
      } else {
        const n = b.id === "me" ? 0 : ++k
        hx = cx + Math.cos(n * 2.4) * 38 * Math.sqrt(n)
        hy = cy + Math.sin(n * 2.4) * 38 * Math.sqrt(n)
      }
      b.hx = hx
      b.hy = hy
      b.x += (hx - b.x) * ease
      b.y += (hy - b.y) * ease
      b.scale += (target - b.scale) * (1 - Math.exp(-dt * 4))
      b.pulse = Math.max(0, b.pulse - dt * 1.4)
      b.bounce += dt * 1.5
      if (b.leaving && b.scale < 0.03) {
        b.el.remove()
        blobs.splice(i, 1)
        continue
      }
      const lift = b.bounce > 0 && b.bounce < 1 ? Math.sin(Math.PI * b.bounce) * 55 : 0
      b.el
        .attr("cx", b.x + noise(i * 7, t * 0.5) * 5)
        .attr("cy", b.y + noise(i * 7 + 3, t * 0.5) * 5 - lift)
        .attr("r", b.r * b.scale * (1 + b.pulse * 0.45))
        .attr("fill", d3.interpolateRgb(b.color, "#fff")(b.pulse * 0.45))
    }
    for (let i = drops.length - 1; i >= 0; i--) {
      const d = drops[i]
      d.vy += 900 * dt
      d.x += d.vx * dt
      d.y += d.vy * dt
      if (d.vy > 0 && d.y > d.from.y) {
        d.el.remove()
        drops.splice(i, 1)
        continue
      }
      d.el.attr("cx", d.x).attr("cy", d.y)
    }
  })

  return {
    fire(event) {
      if (event.kind === "celebrate") {
        blobs.forEach((b, i) => (b.bounce = -i * 0.08))
      } else if (event.kind === "join") {
        if (!find(event.id, event.mine)) add(event.mine ? "me" : event.id || `anon-${added}`)
      } else if (event.kind === "leave") {
        const b = (event.id && find(event.id, event.mine)) || newest()
        if (b) b.leaving = true
      } else if (event.kind === "post") {
        actor(event).pulse = 1
      } else if (event.kind === "react") {
        const from = actor(event)
        drops.push({from, x: from.x, y: from.y, vx: random(-70, 70), vy: random(-260, -340),
          el: group.append("circle").attr("r", 8).attr("fill", from.color)})
      }
    },
    destroy() {
      frames.stop()
      svg.remove()
    }
  }
}
