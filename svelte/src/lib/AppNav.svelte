<!-- The links between the pages. -->
<script>
  import { homeUrl, postsIndexUrl } from "./routes.js";
</script>

<nav>
  <a href={homeUrl()}>Home</a>
  <a href={postsIndexUrl()}>Posts</a>
</nav>
