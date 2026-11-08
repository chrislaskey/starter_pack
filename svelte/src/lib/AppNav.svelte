<!-- The links between the pages, the theme menu, and inside the app the sync status. -->
<script>
  import SyncStatus from "./SyncStatus.svelte";
  import ThemeMenu from "./ThemeMenu.svelte";
  import { communitiesIndexUrl, homeUrl, postsIndexUrl } from "./routes.js";
</script>

<nav class="flex items-center gap-4">
  <a href={homeUrl()} class="link">Home</a>
  <a href={postsIndexUrl()} class="link">Posts</a>
  <a href={communitiesIndexUrl()} class="link">Communities</a>
  <span class="grow"></span>
  <ThemeMenu />
</nav>
<SyncStatus />
