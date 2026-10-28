// The ribbons sharing one band, so the three lines constantly cross and
// overlap, with a blend mode that deepens the color where they do:
// multiply in the light theme, screen in dark. Events as in ribbon.
import {makeRibbon} from "./ribbon"

export default makeRibbon({level: "shared", blend: true})
