// The ribbons frozen at rest. An event sets them flowing from where it
// started, the motion spreading outward along the lines and damping back
// to complete stillness over a few seconds; more events keep it moving.
import {makeRibbon} from "./ribbon"

export default makeRibbon({rest: "still"})
