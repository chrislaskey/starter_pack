<!-- The home page, at / -->
<script>
  import AppNav from "./lib/AppNav.svelte";
  import { postsIndexUrl } from "./lib/routes.js";
</script>

<main class="page">
  <AppNav />
  <h1>Starter Pack</h1>
  <p>
    The same pages on the web, on the desktop and on a phone, over the same Rust application.
    Sign in, then start with the <a href={postsIndexUrl()}>posts</a>.
  </p>
</main>
