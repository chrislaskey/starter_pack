<!-- One post, at /posts/:id -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { getPost } from "../lib/core/index.js";
  import { pathParams } from "../lib/router.js";
  import { postsIndexUrl } from "../lib/routes.js";

  let post = $state();
  let error = $state("");

  async function load() {
    try {
      post = await getPost(pathParams().id);
    } catch (problem) {
      error = String(problem);
    }
  }

  load();
</script>

<main class="page">
  <AppNav />
  <h1>Post</h1>

  {#if error}
    <p class="error">{error}</p>
  {:else if post}
    <article class="post">
      <p class="muted">{post.inserted_at}</p>
      <p>{post.body}</p>
    </article>
  {/if}

  <a href={postsIndexUrl()}>Back to posts</a>
</main>
