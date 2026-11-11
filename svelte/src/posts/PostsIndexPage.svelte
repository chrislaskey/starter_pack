<!-- Every post the person sees, newest first, at /posts: their own, and
     those in their communities -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { onSynced, postsList } from "../lib/core/index.js";
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

  $effect(() => onSynced(load));
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
    {#each posts as { post, owner, communities } (post.id)}
      <li class="post">
        <p class="muted">
          {owner.name} · {post.inserted_at}
          {#if communities.length > 0}
            · in {communities.map((community) => community.name).join(", ")}
          {/if}
        </p>
        <p class="whitespace-pre-wrap">{post.body}</p>
        <a href={postsShowUrl(post.id)}>Show</a>
      </li>
    {/each}
  </ul>
</main>
