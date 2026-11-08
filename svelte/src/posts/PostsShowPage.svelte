<!-- One post, at /posts/:id, with its Edit and Delete -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { onSynced, postsDelete, postsGet } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { postsEditUrl, postsIndexUrl } from "../lib/routes.js";

  const id = pathParams().id;
  let post = $state();
  let error = $state("");

  async function load() {
    try {
      post = await postsGet(id);
    } catch (problem) {
      error = String(problem);
    }
  }

  async function remove() {
    if (!confirm("Delete this post?")) return;
    try {
      await postsDelete(id);
      await navigateToUrl(postsIndexUrl());
    } catch (problem) {
      error = String(problem);
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<main class="page">
  <AppNav />
  <h1>Post</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if post}
    <article class="post">
      <p class="muted">{post.inserted_at}</p>
      <p class="whitespace-pre-wrap">{post.body}</p>
    </article>
    <div class="flex gap-2">
      <a href={postsEditUrl(post.id)} class="btn btn-sm">Edit</a>
      <button type="button" class="btn btn-sm btn-ghost" onclick={remove}>Delete</button>
    </div>
  {/if}

  <a href={postsIndexUrl()}>Back to posts</a>
</main>
