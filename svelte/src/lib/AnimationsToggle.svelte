<!-- The switch for images that move, see animations.js: on, an animated
     GIF plays; off, every image shows its first frame. -->
<script>
  import { animationsOn, onAnimationsChanged, setAnimationsOn } from "./animations.js";

  let on = $state(animationsOn());

  $effect(() => onAnimationsChanged((now) => (on = now)));
</script>

<label class="flex items-center gap-2 text-sm">
  <input
    type="checkbox"
    class="toggle toggle-sm"
    checked={on}
    onchange={(event) => setAnimationsOn(event.currentTarget.checked)}
  />
  Play animations
</label>
