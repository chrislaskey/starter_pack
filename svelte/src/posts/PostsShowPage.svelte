<!-- One post, at /posts/:id, with its Edit and Delete for its owner -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { onSynced, postsDelete, postsGet, usersMe } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { postsEditUrl, postsIndexUrl } from "../lib/routes.js";

  const id = pathParams().id;
  let shown = $state();
  let me = $state();
  let error = $state("");

  async function load() {
    try {
      [shown, me] = await Promise.all([postsGet(id), usersMe()]);
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
  {#if shown}
    <article class="post">
      <p class="muted">
        {shown.owner.email} · {shown.post.inserted_at}
        {#if shown.communities.length > 0}
          · in {shown.communities.map((community) => community.name).join(", ")}
        {/if}
      </p>
      <p class="whitespace-pre-wrap">{shown.post.body}</p>
    </article>
    {#if me && shown.post.owner_id === me.id}
      <div class="flex gap-2">
        <a href={postsEditUrl(shown.post.id)} class="btn btn-sm">Edit</a>
        <button type="button" class="btn btn-sm btn-ghost" onclick={remove}>Delete</button>
      </div>
    {/if}
  {/if}

  <a href={postsIndexUrl()}>Back to posts</a>
</main>
