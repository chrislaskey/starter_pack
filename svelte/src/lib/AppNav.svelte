<!-- The links between the pages, and the theme menu. -->
<script>
  import ThemeMenu from "./ThemeMenu.svelte";
  import { homeUrl, postsIndexUrl } from "./routes.js";
</script>

<nav class="flex items-center gap-4">
  <a href={homeUrl()} class="link">Home</a>
  <a href={postsIndexUrl()} class="link">Posts</a>
  <span class="grow"></span>
  <ThemeMenu />
</nav>
