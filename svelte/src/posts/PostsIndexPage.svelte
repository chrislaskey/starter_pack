<!-- Every post, newest first, at /posts -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { listPosts } from "../lib/core/index.js";
  import { postShowUrl } from "../lib/routes.js";

  let posts = $state([]);
  let error = $state("");

  async function load() {
    try {
      posts = await listPosts();
    } catch (problem) {
      error = String(problem);
    }
  }

  load();
</script>

<main class="page">
  <AppNav />
  <h1>Posts</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <ul class="posts">
    {#each posts as post (post.id)}
      <li class="post">
        <p class="muted">{post.inserted_at}</p>
        <p>{post.body}</p>
        <a href={postShowUrl(post.id)}>Show</a>
      </li>
    {/each}
  </ul>
</main>
