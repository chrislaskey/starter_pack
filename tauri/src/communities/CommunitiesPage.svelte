<!-- The communities page: the user's communities and feed, read from the
     file on this device. Writing a post works with the network off; a sync
     pushes it when the server is back. -->
<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Button from "$phoenix/components/Button.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import ThemeToggle from "$phoenix/components/ThemeToggle.svelte";
  import { homeUrl } from "../lib/routes.js";

  let profile = $state();
  let page = $state();
  let body = $state("");
  let communityIds = $state([]);
  let syncing = $state(false);
  let lastSync = $state("");
  let error = $state("");

  function describe(report) {
    return `Synced: ${report.pushed} pushed, ${report.rejected} rejected, ${report.pulled} pulled, ${report.backfilled} backfilled, ${report.pruned} pruned, ${report.pending} pending`;
  }

  async function load() {
    try {
      profile = await invoke("get_profile");
      error = "";
    } catch (problem) {
      error = String(problem);
      return;
    }
    await refresh();
    sync();
  }

  async function refresh() {
    if (!profile) return;
    try {
      page = await invoke("get_communities_page");
    } catch (problem) {
      error = String(problem);
    }
  }

  async function sync() {
    if (!profile) return;
    syncing = true;
    try {
      lastSync = describe(await invoke("sync_now"));
    } catch (problem) {
      lastSync = `Sync failed, working offline: ${problem}`;
    } finally {
      syncing = false;
      await refresh();
    }
  }

  async function post(event) {
    event.preventDefault();
    try {
      page = await invoke("write_post", { body, communityIds });
      body = "";
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  function toggle(id) {
    communityIds = communityIds.includes(id) ? communityIds.filter((other) => other !== id) : [...communityIds, id];
  }

  let editingId = $state("");
  let editingBody = $state("");

  function startEdit(post) {
    editingId = post.id;
    editingBody = post.body;
  }

  async function saveEdit(event) {
    event.preventDefault();
    try {
      page = await invoke("edit_post", { postId: editingId, body: editingBody });
      editingId = "";
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  onMount(() => {
    const unlisten = listen("synced", (event) => {
      lastSync = event.payload.error ? `Sync failed, working offline: ${event.payload.error}` : describe(event.payload.report);
      refresh();
    });
    load();
    return () => {
      unlisten.then((stop) => stop());
    };
  });
</script>

<main class="page">
  <Header>
    Communities
    {#snippet subtitle()}
      {profile ? `Logged in as ${profile.email}` : "Loading…"}
    {/snippet}
    {#snippet actions()}
      <ThemeToggle />
    {/snippet}
  </Header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if page}
    <section class="actions">
      <Button onclick={sync} disabled={syncing}>{syncing ? "Syncing…" : "Sync now"}</Button>
      <span class="muted">{lastSync}{page.pending ? ` · ${page.pending} change(s) waiting to sync` : ""}</span>
    </section>

    <section>
      <h2>Your communities</h2>
      {#if page.communities.length === 0}
        <p class="muted">Join a community in the web UI; it appears here on the next sync.</p>
      {/if}
      <form id="post-form" onsubmit={post} class="post-form">
        <textarea bind:value={body} rows="3" placeholder="Write a post, even offline"></textarea>
        <div class="communities">
          {#each page.communities as community (community.id)}
            <label>
              <input type="checkbox" checked={communityIds.includes(community.id)} onchange={() => toggle(community.id)} />
              {community.name}
            </label>
          {/each}
        </div>
        <Button variant="primary" type="submit">Post</Button>
      </form>
    </section>

    <section>
      <h2>Feed</h2>
      <ul id="feed" class="feed">
        {#each page.posts as post (post.id)}
          <li class:pending={post.pending}>
            <p class="muted">
              {post.author_email}{post.mine ? " (you)" : ""} · {post.inserted_at.slice(0, 19).replace("T", " ")}
              {#if post.communities.length}· in {post.communities.join(", ")}{/if}
              {#if post.pending}· waiting to sync{/if}
            </p>
            {#if editingId === post.id}
              <form onsubmit={saveEdit} class="post-form">
                <textarea bind:value={editingBody} rows="3"></textarea>
                <div class="actions">
                  <Button variant="primary" type="submit">Save</Button>
                  <Button type="button" onclick={() => (editingId = "")}>Cancel</Button>
                </div>
              </form>
            {:else}
              <p>{post.body}</p>
              {#if post.mine}
                <Button type="button" onclick={() => startEdit(post)}>Edit</Button>
              {/if}
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <section>
    <p><a href={homeUrl()}>← Status</a></p>
  </section>
</main>

<style>
  .post-form {
    display: grid;
    gap: 0.5rem;
    max-width: 40rem;
  }
  textarea {
    width: 100%;
    padding: 0.5rem;
    border: 1px solid color-mix(in oklab, currentColor 30%, transparent);
    border-radius: 0.5rem;
    background: transparent;
  }
  .communities {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
  }
  .feed {
    list-style: none;
    padding: 0;
    display: grid;
    gap: 0.75rem;
    max-width: 40rem;
  }
  .feed li {
    padding: 0.75rem 0;
    border-top: 1px solid color-mix(in oklab, currentColor 15%, transparent);
  }
  .feed li.pending {
    opacity: 0.7;
  }
  .error {
    color: var(--color-error, #b91c1c);
  }
</style>
