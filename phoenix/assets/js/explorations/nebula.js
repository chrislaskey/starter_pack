// Fog drifting across the whole page, drawn by a fragment shader from fbm
// noise: blue where it is thin, purple in the middle, magenta where it
// gathers. An event lights a soft ring in the fog in its color that spreads
// and fades; leave darkens a patch instead; celebrate brightens all of the
// fog for a beat and sends out one ring per palette color.
import * as THREE from "three"
import {frameLoop, originOf} from "./stage"

const slots = 8

const vertexShader = `
void main() {
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`

const fragmentShader = `
uniform float time;
uniform float energy;
uniform float alpha;
uniform vec2 resolution;
uniform vec3 blue;
uniform vec3 purple;
uniform vec3 magenta;
uniform vec2 origins[${slots}];
uniform float ages[${slots}];
uniform vec3 colors[${slots}];
uniform float strengths[${slots}];

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(
    mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
    mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x),
    u.y
  );
}

float fbm(vec2 p) {
  float v = 0.0;
  float a = 0.5;
  for (int i = 0; i < 4; i++) {
    v += a * noise(p);
    p = p * 2.03 + vec2(1.7, 9.2);
    a *= 0.5;
  }
  return v;
}

void main() {
  vec2 uv = gl_FragCoord.xy / resolution;
  vec2 stretch = vec2(resolution.x / resolution.y, 1.0);
  vec2 p = uv * stretch;
  float t = time * 0.04;
  vec2 warp = vec2(fbm(p * 1.4 + t), fbm(p * 1.4 - t * 0.7 + 5.0));
  float n = fbm(p * 1.8 + warp * 1.3 + vec2(t * 0.5, -t * 0.3));
  n = smoothstep(0.28, 0.78, n);
  vec3 color = n < 0.5 ? mix(blue, purple, n * 2.0) : mix(purple, magenta, (n - 0.5) * 2.0);
  float density = n * (0.7 + energy * 0.6) + energy * 0.15;
  for (int i = 0; i < ${slots}; i++) {
    float age = ages[i];
    if (age < 0.0 || age > 2.5) continue;
    float dist = length((uv - origins[i]) * stretch);
    float radius = age * 0.3;
    float band = (dist - radius) / (0.06 + age * 0.06);
    float ring = exp(-band * band);
    float glow = exp(-dist * dist / (0.01 + age * 0.03)) * 0.5;
    float life = min(1.0, age * 6.0) * pow(1.0 - age / 2.5, 2.0);
    float k = (ring + glow) * life * strengths[i];
    color = mix(color, colors[i], clamp(k, 0.0, 1.0) * 0.85);
    density += k * (0.4 + n);
  }
  gl_FragColor = vec4(color, clamp(density, 0.0, 1.0) * alpha);
}
`

export default function nebula(stage, palette) {
  const renderer = new THREE.WebGLRenderer({alpha: true, antialias: false})
  renderer.setClearColor(0, 0)
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
  renderer.domElement.style.cssText = "position:absolute;inset:0;width:100%;height:100%"
  stage.appendChild(renderer.domElement)

  const [magenta, purple, blue] = palette.trio.map(hex => new THREE.Color(hex))
  const uniforms = {
    time: {value: 0},
    energy: {value: 0},
    alpha: {value: 0.25},
    resolution: {value: new THREE.Vector2(1, 1)},
    blue: {value: blue},
    purple: {value: purple},
    magenta: {value: magenta},
    origins: {value: Array.from({length: slots}, () => new THREE.Vector2())},
    ages: {value: new Float32Array(slots).fill(99)},
    colors: {value: Array.from({length: slots}, () => new THREE.Color())},
    strengths: {value: new Float32Array(slots)}
  }
  const scene = new THREE.Scene()
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1)
  const geometry = new THREE.PlaneGeometry(2, 2)
  const material = new THREE.ShaderMaterial({
    uniforms,
    vertexShader,
    fragmentShader,
    transparent: true,
    depthTest: false,
    depthWrite: false
  })
  scene.add(new THREE.Mesh(geometry, material))

  const resize = () => {
    renderer.setSize(stage.clientWidth, stage.clientHeight, false)
    renderer.getDrawingBufferSize(uniforms.resolution.value)
  }
  resize()
  const observer = new ResizeObserver(resize)
  observer.observe(stage)

  let energy = 0
  const frames = frameLoop()
  frames.start(dt => {
    uniforms.time.value += dt
    energy = Math.max(0, energy - dt / 1.3)
    uniforms.energy.value = energy * energy * (3 - 2 * energy)
    uniforms.alpha.value = palette.dark() ? 0.45 : 0.25
    const ages = uniforms.ages.value
    for (let i = 0; i < slots; i++) ages[i] += dt
    renderer.render(scene, camera)
  })

  const pulse = (x, y, strength, hex, age = 0) => {
    const ages = uniforms.ages.value
    let slot = 0
    for (let i = 1; i < slots; i++) if (ages[i] > ages[slot]) slot = i
    uniforms.origins.value[slot].set(x, 1 - y)
    uniforms.colors.value[slot].set(hex)
    uniforms.strengths.value[slot] = strength
    ages[slot] = age
  }

  return {
    fire(event) {
      if (event.kind === "celebrate") {
        energy = 1
        palette.trio.forEach((hex, i) => pulse(0.25 + i * 0.25, 0.3 + (i % 2) * 0.35, 0.8, hex, -i * 0.2))
        return
      }
      const {x, y} = originOf(event, {w: 1, h: 1})
      const strength = {join: 0.6, leave: -0.7, post: 1, react: 0.35}[event.kind] || 0.5
      pulse(x, y, strength, palette.colorOf(event.kind))
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
