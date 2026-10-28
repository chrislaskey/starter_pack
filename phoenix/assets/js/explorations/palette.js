// The one palette every exploration draws with, so the ideas compare on
// motion alone: magenta, purple and blue, with a lavender for small touches.
export const magenta = "#e935c1"
export const purple = "#8b5cf6"
export const blue = "#3b82f6"
export const lavender = "#c4b5fd"

export const trio = [magenta, purple, blue]

// The color each event draws in. Celebrate uses the whole trio.
const byKind = {join: blue, leave: purple, post: magenta, react: lavender, celebrate: magenta}

export function colorOf(kind) {
  return byKind[kind] || purple
}

export function rgba(hex, alpha) {
  const n = parseInt(hex.slice(1), 16)
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`
}

export function dark() {
  return document.documentElement.getAttribute("data-theme") === "dark"
}
