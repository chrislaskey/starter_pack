// The LiveView hooks behind /explorations/:slug. `Exploration` runs one
// exploration in the stage element and feeds it every "viz" event the
// server pushes, plus the ones `ExplorationControls` makes up when
// simulating other people.
import {explorations} from "./index"
import * as palette from "./palette"
import {pick, random} from "./stage"

export const Exploration = {
  mounted() {
    const make = explorations[this.el.dataset.exploration]
    this.viz = make(this.el, palette)
    this.handleEvent("viz", event => this.viz.fire(event))
    this.onLocal = e => this.viz.fire(e.detail)
    window.addEventListener("viz:fire", this.onLocal)
  },

  destroyed() {
    window.removeEventListener("viz:fire", this.onLocal)
    this.viz.destroy()
  }
}

// Fires events as if other people were here: a stream of joins, leaves,
// posts and reactions at uneven intervals while the checkbox is ticked.
export const ExplorationControls = {
  mounted() {
    const box = this.el.querySelector("input[type=checkbox][name=simulate]")
    const ghosts = []
    let timer = null

    const fire = detail => window.dispatchEvent(new CustomEvent("viz:fire", {detail}))

    const next = () => {
      const kind = pick(["join", "post", "react", "react", "post", "leave", "join", "post"])
      if (kind === "join") {
        const id = `ghost-${Math.random().toString(36).slice(2, 8)}`
        ghosts.push(id)
        fire({kind, id, mine: false})
      } else if (kind === "leave") {
        if (ghosts.length) fire({kind, id: ghosts.pop(), mine: false})
      } else {
        fire({kind, id: pick(ghosts) || null, mine: false})
      }
      timer = setTimeout(next, random(700, 2600))
    }

    box.addEventListener("change", () => {
      if (box.checked) next()
      else clearTimeout(timer)
    })
    this.stop = () => clearTimeout(timer)
  },

  destroyed() {
    this.stop()
  }
}
