// Every exploration by slug. Each module's default export is
// `mount(stage, palette) => {fire(event), destroy()}`: it draws into the
// stage, a fixed full-viewport element behind the page content, and reacts
// to events `{kind, id, mine, at}` where kind is join, leave, post, react
// or celebrate. The list in App.Web.ExplorationsLive describes each one.
import waves from "./waves"
import wavesSlow from "./waves-slow"
import wavesStill from "./waves-still"
import wavesTide from "./waves-tide"
import wavesLines from "./waves-lines"
import wavesLinesStill from "./waves-lines-still"
import ribbon from "./ribbon"
import ribbonBraid from "./ribbon-braid"
import ribbonThick from "./ribbon-thick"
import ribbonStill from "./ribbon-still"
import ribbonSlow from "./ribbon-slow"
import ribbonSilk from "./ribbon-silk"
import goo from "./goo"
import heartbeat from "./heartbeat"
import ripples from "./ripples"
import fireflies from "./fireflies"
import constellation from "./constellation"
import dotgrid from "./dotgrid"
import confetti from "./confetti"
import bubbles from "./bubbles"
import comet from "./comet"
import orbit from "./orbit"
import meteors from "./meteors"
import seismograph from "./seismograph"
import edgeglow from "./edgeglow"
import aurora from "./aurora"
import sonar from "./sonar"
import terrain from "./terrain"
import nebula from "./nebula"

export const explorations = {
  waves,
  "waves-slow": wavesSlow,
  "waves-still": wavesStill,
  "waves-tide": wavesTide,
  "waves-lines": wavesLines,
  "waves-lines-still": wavesLinesStill,
  ribbon,
  "ribbon-braid": ribbonBraid,
  "ribbon-thick": ribbonThick,
  "ribbon-still": ribbonStill,
  "ribbon-slow": ribbonSlow,
  "ribbon-silk": ribbonSilk,
  goo,
  heartbeat,
  ripples,
  fireflies,
  constellation,
  dotgrid,
  confetti,
  bubbles,
  comet,
  orbit,
  meteors,
  seismograph,
  edgeglow,
  aurora,
  sonar,
  terrain,
  nebula
}
