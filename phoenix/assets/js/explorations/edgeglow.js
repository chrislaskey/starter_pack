// A soft glow along the viewport edges, the quietest idea here: nothing
// moves until an event breathes color into one edge. The user's own actions
// light the bottom edge, other people's the top, and celebrate lights all
// four in turn.
export default function edgeglow(stage, palette) {
  const edges = {}
  for (const side of ["top", "bottom", "left", "right"]) {
    const el = document.createElement("div")
    const vertical = side === "left" || side === "right"
    el.style.cssText = `position:absolute;${side}:0;opacity:0;` +
      (vertical ? "top:0;bottom:0;width:22vw;" : "left:0;right:0;height:22vh;")
    stage.appendChild(el)
    edges[side] = {el, side, vertical}
  }

  const glow = ({el, side, vertical}, color, strength) => {
    const to = {top: "bottom", bottom: "top", left: "right", right: "left"}[side]
    el.style.background = `linear-gradient(to ${to}, ${palette.rgba(color, 0.55)}, transparent)`
    el.animate(
      [
        {opacity: 0, transform: vertical ? "scaleX(0.6)" : "scaleY(0.6)"},
        {opacity: strength, offset: 0.25},
        {opacity: 0, transform: "scale(1)"}
      ],
      {duration: 1800, easing: "ease-out", fill: "forwards"}
    )
    el.style.transformOrigin = `${side} center`
  }

  return {
    fire(event) {
      if (event.kind === "celebrate") {
        ;["bottom", "right", "top", "left"].forEach((side, i) =>
          setTimeout(() => glow(edges[side], palette.trio[i % 3], 1), i * 180)
        )
        return
      }
      const strength = {join: 0.8, leave: 0.5, post: 1, react: 0.4}[event.kind] || 0.6
      glow(edges[event.mine ? "bottom" : "top"], palette.colorOf(event.kind), strength)
    },
    destroy() {
      for (const {el} of Object.values(edges)) el.remove()
    }
  }
}
