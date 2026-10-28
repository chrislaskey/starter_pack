// The ribbons flowing at a tenth of their usual speed. An event speeds the
// flow up around where it started for a few seconds, easing back down, and
// sends the usual swell along the lines.
import {makeRibbon} from "./ribbon"

export default makeRibbon({rest: "slow"})
