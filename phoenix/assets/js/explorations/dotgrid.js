// Dotted paper: a grid of tiny dots over the whole page, barely there. An
// event sends a ring out through the grid from where it happened; each dot
// brightens, swells and takes the color as the ring passes, then settles.
// Leave's ring is dimmer and shrinks the dots. Celebrate is three from center.
import {canvasStage, originOf} from "./stage"

const looks = {
  join: {band: 170, alpha: 0.7, grow: 1.3},
  leave: {band: 150, alpha: 0.4, grow: -0.6},
  post: {band: 240, alpha: 0.95, grow: 2},
  react: {band: 110, alpha: 0.55, grow: 0.9},
  celebrate: {band: 220, alpha: 0.9, grow: 1.8}
}

export default function dotgrid(stage, palette) {
  const {ctx, size, loop, destroy} = canvasStage(stage)
  const gap = 28
  const radius = 1.3
  const speed = 520
  const waves = []
  let dots = []
  let laidOut = ""

  const layout = () => {
    laidOut = `${size.w}x${size.h}`
    dots = []
    const ox = (size.w % gap) / 2
    const oy = (size.h % gap) / 2
    for (let y = oy; y <= size.h; y += gap) {
      for (let x = ox; x <= size.w; x += gap) dots.push({x, y})
    }
  }

  loop(dt => {
    if (laidOut !== `${size.w}x${size.h}`) layout()
    ctx.clearRect(0, 0, size.w, size.h)
    for (let i = waves.length - 1; i >= 0; i--) {
      waves[i].age += dt
      if (waves[i].age > waves[i].life) waves.splice(i, 1)
    }
    const base = palette.dark() ? 0.18 : 0.11
    const lit = []
    ctx.beginPath()
    for (const dot of dots) {
      let hit = 0
      let wave = null
      for (const w of waves) {
        if (w.age < 0) continue
        const behind = w.age * speed - Math.hypot(dot.x - w.x, dot.y - w.y)
        if (behind < 0 || behind > w.band) continue
        const u = behind / w.band
        const s = (u < 0.2 ? u / 0.2 : 1 - ((u - 0.2) / 0.8) ** 2) * (1 - w.age / w.life)
        if (s > hit) {
          hit = s
          wave = w
        }
      }
      if (hit < 0.02) {
        ctx.moveTo(dot.x + radius, dot.y)
        ctx.arc(dot.x, dot.y, radius, 0, Math.PI * 2)
      } else {
        lit.push({dot, wave, hit})
      }
    }
    ctx.fillStyle = palette.rgba(palette.purple, base)
    ctx.fill()
    for (const {dot, wave, hit} of lit) {
      ctx.beginPath()
      ctx.arc(dot.x, dot.y, radius * (1 + wave.grow * hit), 0, Math.PI * 2)
      ctx.fillStyle = palette.rgba(wave.color, base + (wave.alpha - base) * hit)
      ctx.fill()
    }
  })

  return {
    fire(event) {
      const look = looks[event.kind] || looks.join
      const life = Math.hypot(size.w, size.h) / speed
      if (event.kind === "celebrate") {
        palette.trio.forEach((color, i) =>
          waves.push({...look, color, x: size.w / 2, y: size.h / 2, age: -i * 0.22, life})
        )
      } else {
        const {x, y} = originOf(event, size)
        waves.push({...look, color: palette.colorOf(event.kind), x, y, age: 0, life})
      }
    },
    destroy
  }
}
