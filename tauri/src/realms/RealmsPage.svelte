<!-- The realms page: the user's realms, and for the chosen one its communities
     and feed, read from the realm's file on this device. Writing a post
     works with the network off; a sync pushes it when the server is back. -->
<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Button from "$phoenix/components/Button.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import ThemeToggle from "$phoenix/components/ThemeToggle.svelte";

  let identity = $state();
  let slug = $state("");
  let page = $state();
  let body = $state("");
  let communityIds = $state([]);
  let syncing = $state(false);
  let lastSync = $state("");
  let error = $state("");

  function describe(report) {
    return `Synced: ${report.pushed} pushed, ${report.rejected} rejected, ${report.pulled} pulled, ${report.backfilled} backfilled, ${report.pruned} pruned, ${report.pending} pending`;
  }

  async function loadRealms() {
    try {
      identity = await invoke("list_realms");
      error = "";
      if (!slug && identity.realms[0]) await select(identity.realms[0].slug);
    } catch (problem) {
      error = String(problem);
    }
  }

  async function select(next) {
    slug = next;
    communityIds = [];
    await refresh();
    sync();
  }

  async function refresh() {
    if (!slug) return;
    try {
      page = await invoke("get_realm_page", { slug });
    } catch (problem) {
      error = String(problem);
    }
  }

  async function sync() {
    if (!slug) return;
    syncing = true;
    try {
      lastSync = describe(await invoke("sync_realm", { slug }));
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
      page = await invoke("write_post", { slug, body, communityIds });
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
      page = await invoke("edit_post", { slug, postId: editingId, body: editingBody });
      editingId = "";
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  onMount(() => {
    const unlisten = listen("realm-synced", (event) => {
      if (event.payload.slug !== slug) return;
      lastSync = event.payload.error ? `Sync failed, working offline: ${event.payload.error}` : describe(event.payload.report);
      refresh();
    });
    loadRealms();
    return () => {
      unlisten.then((stop) => stop());
    };
  });
</script>

<main class="page">
  <Header>
    Realms
    {#snippet subtitle()}
      {identity ? `Logged in as ${identity.user.email}` : "Loading the realm directory…"}
    {/snippet}
    {#snippet actions()}
      <ThemeToggle />
    {/snippet}
  </Header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if identity}
    <section class="actions" id="realm-picker">
      {#each identity.realms as realm (realm.id)}
        <Button variant={realm.slug === slug ? "primary" : undefined} onclick={() => select(realm.slug)}>
          {realm.slug}
        </Button>
      {/each}
      {#if identity.realms.length === 0}
        <p class="muted">You are not connected to any realm yet.</p>
      {/if}
    </section>
  {/if}

  {#if page}
    <section class="actions">
      <Button onclick={sync} disabled={syncing}>{syncing ? "Syncing…" : "Sync now"}</Button>
      <span class="muted">{lastSync}{page.pending ? ` · ${page.pending} change(s) waiting to sync` : ""}</span>
    </section>

    <section>
      <h2>Your communities in {page.realm.slug}</h2>
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
    <p><a href="index.html">← Status</a></p>
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
