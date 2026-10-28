// Thin overlapping wave lines that sit still until an event sets them
// moving and thickens them where the ripple passes, settling back to rest.
import {makeWaves} from "./waves"

export default makeWaves({style: "line", rest: "still"})
