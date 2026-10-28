// A field of points lying across the bottom of the page and receding to
// the horizon, rolling on slow 3D noise: blue in the valleys, purple on
// the slopes, magenta on the peaks. An event drops a ripple that spreads
// out in its color; post is a strong one, react a small one, leave a dip.
// Celebrate swells the whole field up and lets it settle.
import * as THREE from "three"
import {createNoise3D} from "simplex-noise"
import {clamp, frameLoop, lerp, originOf} from "./stage"

export default function terrain(stage, palette) {
  const cols = 120
  const rows = 60
  const near = 4
  const far = 36
  const cameraZ = 6
  const depthAt = f => near * Math.pow(far / near, f)
  const halfWidthAt = d => d * 1.2

  const renderer = new THREE.WebGLRenderer({alpha: true, antialias: true})
  renderer.setClearColor(0, 0)
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
  renderer.domElement.style.cssText = "position:absolute;inset:0;width:100%;height:100%"
  stage.appendChild(renderer.domElement)

  const scene = new THREE.Scene()
  const camera = new THREE.PerspectiveCamera(50, 1, 0.1, 120)
  camera.position.set(0, 2.6, cameraZ)
  camera.lookAt(0, 3.2, cameraZ - 20)

  const count = cols * rows
  const positions = new Float32Array(count * 3)
  const colors = new Float32Array(count * 3)
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3).setUsage(THREE.DynamicDrawUsage))
  geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3).setUsage(THREE.DynamicDrawUsage))
  const material = new THREE.PointsMaterial({
    size: 0.09,
    sizeAttenuation: true,
    vertexColors: true,
    transparent: true,
    opacity: 0.7,
    depthWrite: false
  })
  const points = new THREE.Points(geometry, material)
  points.frustumCulled = false
  scene.add(points)

  const noise = createNoise3D()
  const [magenta, purple, blue] = palette.trio.map(hex => new THREE.Color(hex))
  const shade = new THREE.Color()
  const ripples = []
  let swell = 0
  let lift = 0

  const resize = () => {
    renderer.setSize(stage.clientWidth, stage.clientHeight, false)
    camera.aspect = stage.clientWidth / Math.max(stage.clientHeight, 1)
    camera.updateProjectionMatrix()
  }
  resize()
  const observer = new ResizeObserver(resize)
  observer.observe(stage)

  const frames = frameLoop()
  frames.start((dt, t) => {
    swell = Math.max(0, swell - dt / 1.4)
    lift = lerp(lift, swell, 1 - Math.exp(-dt * 2.5))
    for (let i = ripples.length - 1; i >= 0; i--) {
      ripples[i].age += dt
      if (ripples[i].age > ripples[i].life) ripples.splice(i, 1)
    }
    const amplitude = 1.1 + lift * 0.9
    let i = 0
    for (let r = 0; r < rows; r++) {
      const d = depthAt(r / (rows - 1))
      const z = cameraZ - d
      const half = halfWidthAt(d)
      for (let c = 0; c < cols; c++) {
        const x = (c / (cols - 1) - 0.5) * 2 * half
        const n = noise(x * 0.07, z * 0.07, t * 0.12)
        let y = n * amplitude + lift * 1.8
        const h = clamp((n + 1) / 2 + lift * 0.4, 0, 1)
        if (h < 0.5) shade.lerpColors(blue, purple, h * 2)
        else shade.lerpColors(purple, magenta, (h - 0.5) * 2)
        for (const ripple of ripples) {
          if (ripple.age < 0) continue
          const dx = x - ripple.x
          const dz = z - ripple.z
          const band = (Math.sqrt(dx * dx + dz * dz) - ripple.age * ripple.speed) / (1.5 + ripple.age)
          const crest = Math.exp(-band * band) * Math.exp(-ripple.age * 1.1) * ripple.strength
          y += crest * 1.6
          shade.lerp(ripple.color, clamp(Math.abs(crest), 0, 1))
        }
        positions[i * 3] = x
        positions[i * 3 + 1] = y
        positions[i * 3 + 2] = z
        colors[i * 3] = shade.r
        colors[i * 3 + 1] = shade.g
        colors[i * 3 + 2] = shade.b
        i++
      }
    }
    geometry.attributes.position.needsUpdate = true
    geometry.attributes.color.needsUpdate = true
    material.opacity = palette.dark() ? 0.8 : 0.7
    renderer.render(scene, camera)
  })

  const drop = (depth, across, strength, hex, age = 0) => {
    const d = depthAt(depth)
    const x = (across - 0.5) * 2 * halfWidthAt(d) * 0.7
    ripples.push({x, z: cameraZ - d, age, life: 3.4, speed: 5 + d * 0.2, strength, color: new THREE.Color(hex)})
  }

  return {
    fire(event) {
      if (event.kind === "celebrate") {
        swell = 1
        palette.trio.forEach((hex, i) => drop(0.3 + i * 0.25, 0.2 + i * 0.3, 1.2, hex, -i * 0.2))
        return
      }
      const {x, y} = originOf(event, {w: 1, h: 1})
      const strength = {join: 0.9, leave: -0.7, post: 1.5, react: 0.4}[event.kind] || 0.8
      drop(1 - y, x, strength, palette.colorOf(event.kind))
    },
    destroy() {
      frames.stop()
      observer.disconnect()
      geometry.dispose()
      material.dispose()
      renderer.dispose()
      renderer.domElement.remove()
    }
  }
}
