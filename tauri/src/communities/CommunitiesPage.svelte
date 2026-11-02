<!-- The communities page: the user's communities and feed, read from the
     file on this device. Writing a post works with the network off; a sync
     pushes it when the server is back. -->
<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Button from "$phoenix/components/Button.svelte";
  import DynamicForm from "$phoenix/components/DynamicForm.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import ThemeToggle from "$phoenix/components/ThemeToggle.svelte";
  import { homeUrl } from "../lib/routes.js";

  let profile = $state();
  let page = $state();
  let syncing = $state(false);
  let lastSync = $state("");
  let error = $state("");

  function describe(report) {
    return `Synced: ${report.pushed} pushed, ${report.rejected} rejected, ${report.conflicts} not saved, ${report.pulled} pulled, ${report.backfilled} backfilled, ${report.pruned} pruned, ${report.pending} pending`;
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
    await showPostForm();
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

  // The post form is Phoenix's post_form.dynamic_form.json, validated and
  // laid out by the Rust (src-tauri/src/forms.rs) on every change, as
  // LiveView does on the web. Only the reply to the latest request is
  // shown, so a slow reply never puts back what the user has since typed
  // over. A written post remounts the form, empty, as the web page does.
  let postForm = $state();
  let postFormKey = $state(0);
  let posting = $state(false);
  let postEntries = null;
  let latestPostFormRequest = 0;

  // The form as it first shows, or, once the user has typed, as it is
  // again with the communities as they are now
  async function showPostForm() {
    if (!profile) return;
    if (postForm && postEntries) return changePostForm(postEntries, postForm.state);
    const request = ++latestPostFormRequest;
    try {
      const form = await invoke("show_post_form");
      if (request === latestPostFormRequest) postForm = form;
    } catch (problem) {
      error = String(problem);
    }
  }

  async function changePostForm(entries, state) {
    postEntries = entries;
    const request = ++latestPostFormRequest;
    try {
      const form = await invoke("change_post_form", { form: state, entries });
      if (request === latestPostFormRequest) postForm = form;
    } catch (problem) {
      error = String(problem);
    }
  }

  async function writePost(entries, state) {
    latestPostFormRequest++;
    posting = true;
    try {
      const reply = await invoke("write_post", { form: state, entries });
      if (reply.result === "written") {
        page = reply.page;
        postFormKey++;
        postForm = null;
        postEntries = null;
        await showPostForm();
      } else {
        postForm = reply.form;
      }
      error = "";
    } catch (problem) {
      error = String(problem);
    } finally {
      posting = false;
    }
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

  async function remove(post) {
    if (!confirm("Delete this post?")) return;
    try {
      page = await invoke("delete_post", { postId: post.id });
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  let editingProfile = $state(false);
  let displayName = $state("");
  let bio = $state("");

  function startProfileEdit() {
    displayName = page.profile.display_name;
    bio = page.profile.bio;
    editingProfile = true;
  }

  async function saveProfile(event) {
    event.preventDefault();
    try {
      page = await invoke("edit_profile", { displayName, bio });
      editingProfile = false;
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  let showConflicts = $state(false);

  async function resolve(conflict, resolution) {
    try {
      page = await invoke("resolve_conflict", { eventId: conflict.id, resolution });
      if (page.conflicts.length === 0) showConflicts = false;
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  function shown(value) {
    if (value === null || value === undefined) return "(none)";
    return typeof value === "string" ? value : JSON.stringify(value);
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

    {#if page.conflicts.length}
      <section id="conflicts" class="conflicts">
        <p>
          <strong>
            {page.conflicts.length === 1 ? "1 change was" : `${page.conflicts.length} changes were`} not saved on the server.
          </strong>
          Someone else changed the same thing first.
          <button type="button" class="link" onclick={() => (showConflicts = !showConflicts)}>
            {showConflicts ? "Hide details" : "See details"}
          </button>
        </p>
        {#if showConflicts}
          <ul>
            {#each page.conflicts as conflict (conflict.id)}
              <li>
                <p class="muted">
                  {conflict.summary || "A post"} · changed {conflict.made_at.slice(0, 19).replace("T", " ")}
                </p>
                {#each conflict.fields as field (field.field)}
                  <dl>
                    <dt>Your change to {field.field}</dt>
                    <dd>{shown(field.yours)}</dd>
                    <dt>Now on the server</dt>
                    <dd>{conflict.row_exists ? shown(field.server) : "(the post is gone)"}</dd>
                  </dl>
                {/each}
                <div class="actions">
                  {#if conflict.row_exists}
                    <Button variant="primary" type="button" onclick={() => resolve(conflict, "reapply")}>Reapply</Button>
                  {/if}
                  <Button type="button" onclick={() => resolve(conflict, "ignore")}>Let it go</Button>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/if}

    {#if page.profile}
      <section id="profile" class:pending={page.profile.pending}>
        <h2>Your profile</h2>
        {#if editingProfile}
          <form onsubmit={saveProfile} class="post-form">
            <label>Display name <input bind:value={displayName} /></label>
            <label>Bio <textarea bind:value={bio} rows="3"></textarea></label>
            <div class="actions">
              <Button variant="primary" type="submit">Save</Button>
              <Button type="button" onclick={() => (editingProfile = false)}>Cancel</Button>
            </div>
          </form>
        {:else}
          <p><strong>{page.profile.display_name || "(no display name)"}</strong>{#if page.profile.pending}<span class="muted"> · waiting to sync</span>{/if}</p>
          <p>{page.profile.bio}</p>
          <Button type="button" onclick={startProfileEdit}>Edit profile</Button>
        {/if}
      </section>
    {/if}

    <section>
      <h2>Your communities</h2>
      {#if page.communities.length === 0}
        <p class="muted">Join a community in the web UI; it appears here on the next sync.</p>
      {/if}
      {#if postForm}
        <div class="post-form">
          {#key postFormKey}
            <DynamicForm
              id="post-form"
              form={postForm}
              submit_text="Post"
              disabled={posting}
              onchange={changePostForm}
              onsubmit={writePost}
            />
          {/key}
        </div>
      {/if}
    </section>

    <section>
      <h2>Feed</h2>
      <ul id="feed" class="feed">
        {#each page.posts as post (post.id)}
          <li class:pending={post.pending}>
            <p class="muted">
              {post.owner_email}{post.mine ? " (you)" : ""} · {post.inserted_at.slice(0, 19).replace("T", " ")}
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
                <div class="actions">
                  <Button type="button" onclick={() => startEdit(post)}>Edit</Button>
                  <Button type="button" onclick={() => remove(post)}>Delete</Button>
                </div>
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
  input:not([type="checkbox"]),
  textarea {
    width: 100%;
    padding: 0.5rem;
    border: 1px solid color-mix(in oklab, currentColor 30%, transparent);
    border-radius: 0.5rem;
    background: transparent;
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
  #profile.pending,
  .feed li.pending {
    opacity: 0.7;
  }
  .conflicts {
    max-width: 40rem;
    padding: 0.75rem 1rem;
    border: 1px solid var(--color-warning, #d97706);
    border-radius: 0.5rem;
  }
  .conflicts ul {
    list-style: none;
    padding: 0;
    display: grid;
    gap: 1rem;
  }
  .conflicts dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.25rem 1rem;
    margin: 0.5rem 0;
  }
  .conflicts dt {
    font-weight: 600;
  }
  .conflicts dd {
    margin: 0;
    white-space: pre-wrap;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    text-decoration: underline;
    cursor: pointer;
    color: inherit;
  }
  .error {
    color: var(--color-error, #b91c1c);
  }
</style>
