<!-- Every post, newest first, at /posts -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { postsList } from "../lib/core/index.js";
  import { postsNewUrl, postsShowUrl } from "../lib/routes.js";

  let posts = $state([]);
  let error = $state("");

  async function load() {
    try {
      posts = await postsList();
    } catch (problem) {
      error = String(problem);
    }
  }

  load();
</script>

<main class="page">
  <AppNav />
  <div class="flex items-center justify-between">
    <h1>Posts</h1>
    <a href={postsNewUrl()} class="btn btn-primary btn-sm">New post</a>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if posts.length === 0 && !error}
    <p class="muted">No posts yet.</p>
  {/if}

  <ul class="posts">
    {#each posts as post (post.id)}
      <li class="post">
        <p class="muted">{post.inserted_at}</p>
        <p class="whitespace-pre-wrap">{post.body}</p>
        <a href={postsShowUrl(post.id)}>Show</a>
      </li>
    {/each}
  </ul>
</main>
