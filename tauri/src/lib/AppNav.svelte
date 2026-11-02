<!-- The links between the main window's pages. -->
<script>
  import {
    communitiesIndexUrl,
    homeUrl,
    postsIndexUrl,
    profileUrl,
    tagsIndexUrl,
  } from "./routes.js";
</script>

<nav class="flex flex-wrap gap-4 text-sm">
  <a href={homeUrl()} class="link">Status</a>
  <a href={communitiesIndexUrl()} class="link">Communities</a>
  <a href={postsIndexUrl()} class="link">Posts</a>
  <a href={tagsIndexUrl()} class="link">Tags</a>
  <a href={profileUrl()} class="link">Profile</a>
</nav>
