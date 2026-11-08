<!-- Writing a post, at /posts/new; done, it shows the post -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { postsCreate } from "../lib/core/index.js";
  import { navigateToUrl } from "../lib/router.js";
  import { postsShowUrl, postsIndexUrl } from "../lib/routes.js";
  import PostsForm from "./PostsForm.svelte";

  let text = $state("");
  let communityIds = $state([]);
  let error = $state("");
  let busy = $state(false);

  async function save() {
    busy = true;
    try {
      const post = await postsCreate(text, communityIds);
      await navigateToUrl(postsShowUrl(post.id));
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<main class="page">
  <AppNav />
  <h1>New post</h1>
  <PostsForm bind:text bind:communityIds submitText="Post" {error} {busy} onsubmit={save} />
  <a href={postsIndexUrl()}>Back to posts</a>
</main>
