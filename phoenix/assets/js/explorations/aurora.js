// Three big blurred blobs, one per palette color, drifting slowly around the
// page like light behind frosted glass. An event swells the blob in its
// color and lets it relax; leave dims its blob instead; react gives the
// purple one a small nudge; celebrate swells all three in turn.
export default function aurora(stage, palette) {
  const rest = () => (palette.dark() ? 0.3 : 0.18)
  const routes = [
    {dx: 1, dy: 1, duration: 47000},
    {dx: -1, dy: 1, duration: 63000},
    {dx: 1, dy: -1, duration: 55000}
  ]
  const waypoints = [[0, 0, 1], [28, 18, 1.12], [-8, 42, 0.9], [24, -12, 1.06], [-18, 12, 1]]
  const timers = new Set()

  const blobs = palette.trio.map((color, i) => {
    const drift = document.createElement("div")
    drift.style.cssText =
      "position:absolute;width:50vw;height:50vw;will-change:transform;" +
      `left:${[-12, 48, 18][i]}vw;top:${[-18, 8, 52][i]}vh`
    const el = document.createElement("div")
    el.style.cssText =
      `width:100%;height:100%;border-radius:50%;background:${color};` +
      `filter:blur(90px);will-change:transform,opacity;opacity:${rest()}`
    drift.appendChild(el)
    stage.appendChild(drift)
    const {dx, dy, duration} = routes[i]
    drift.animate(
      waypoints.map(([x, y, s]) => ({transform: `translate(${x * dx}vw, ${y * dy}vh) scale(${s})`})),
      {duration, iterations: Infinity, direction: "alternate", easing: "ease-in-out", delay: -i * 9000}
    )
    return {color, el}
  })
  const byColor = Object.fromEntries(blobs.map(blob => [blob.color, blob]))

  const swell = ({el}, lift, scale) => {
    const from = rest()
    const to = Math.min(1, Math.max(0, from + lift))
    el.style.opacity = from
    el.animate(
      [
        {opacity: from, transform: "scale(1)", easing: "ease-out"},
        {opacity: to, transform: `scale(${scale})`, offset: 0.21, easing: "ease-in-out"},
        {opacity: from, transform: "scale(1)"}
      ],
      {duration: 1900}
    )
  }

  const later = (fn, ms) => {
    const id = setTimeout(() => {
      timers.delete(id)
      fn()
    }, ms)
    timers.add(id)
  }

  const theme = new MutationObserver(() => {
    for (const {el} of blobs) el.style.opacity = rest()
  })
  theme.observe(document.documentElement, {attributes: true, attributeFilter: ["data-theme"]})

  return {
    fire(event) {
      if (event.kind === "celebrate") {
        blobs.forEach((blob, i) => later(() => swell(blob, 0.3, 1.25), i * 260))
        return
      }
      if (event.kind === "leave") return swell(byColor[palette.colorOf("leave")], -rest() * 0.7, 0.92)
      if (event.kind === "react") return swell(byColor[palette.purple], 0.1, 1.07)
      swell(byColor[palette.colorOf(event.kind)] || byColor[palette.purple], 0.25, 1.2)
    },
    destroy() {
      theme.disconnect()
      for (const id of timers) clearTimeout(id)
      for (const {el} of blobs) el.parentNode.remove()
    }
  }
}
