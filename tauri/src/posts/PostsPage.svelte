<!-- Everything the user can read across their communities, newest first,
     with each post's links and tags, and the post form, as
     App.Web.PostsLive shows it. On their own posts the user adds and
     removes links and tags, edits and deletes. Read from the file on this
     device, so it all works with the network off. -->
<script>
  import { invoke } from "@tauri-apps/api/core";
  import Button from "$phoenix/components/Button.svelte";
  import DynamicForm from "$phoenix/components/DynamicForm.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import TextInput from "$phoenix/components/TextInput.svelte";
  import ThemeToggle from "$phoenix/components/ThemeToggle.svelte";
  import AppNav from "../lib/AppNav.svelte";
  import SyncBar from "../lib/SyncBar.svelte";
  import { communitiesIndexUrl, tagsIndexUrl } from "../lib/routes.js";
  import { submitTo, write } from "../lib/write.js";

  let page = $state();
  let error = $state("");

  async function load() {
    try {
      page = await invoke("get_posts_page");
    } catch (problem) {
      error = String(problem);
    }
    await showPostForm();
  }

  const showError = (message) => (error = message);
  const run = async (command, args) => showError(await write(command, args, load));

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
        postFormKey++;
        postForm = null;
        postEntries = null;
        await load();
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

  async function saveEdit(event) {
    event.preventDefault();
    await run("edit_post", { postId: editingId, body: editingBody });
    if (!error) editingId = "";
  }

  function remove(post) {
    if (confirm("Delete this post?")) run("delete_post", { postId: post.id });
  }
</script>

<main class="page">
  <AppNav />
  <Header>
    Posts
    {#snippet subtitle()}
      Everything you can read in your communities
    {/snippet}
    {#snippet actions()}
      <div class="flex items-center gap-4">
        <a href={communitiesIndexUrl()} class="link">Communities</a>
        <a href={tagsIndexUrl()} class="link">Tags</a>
        <ThemeToggle />
      </div>
    {/snippet}
  </Header>

  <SyncBar onrefresh={load} />

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if postForm}
    <div id="post-form">
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

  {#if page}
    <ul id="posts" class="divide-y divide-base-200">
      {#each page.posts as post (post.id)}
        <li id="posts-{post.id}" class="space-y-2 py-3" class:pending={post.pending}>
          <p class="text-sm text-base-content/60">
            {post.owner_email}
            {#if post.communities.length}· in {post.communities.join(", ")}{/if}
            {#if post.pending}· waiting to sync{/if}
          </p>

          {#if editingId === post.id}
            <form onsubmit={saveEdit} class="space-y-2">
              <textarea bind:value={editingBody} rows="3" class="w-full textarea" aria-label="Post"></textarea>
              <div class="flex gap-2">
                <Button variant="primary" type="submit">Save</Button>
                <Button type="button" onclick={() => (editingId = "")}>Cancel</Button>
              </div>
            </form>
          {:else}
            <p class="whitespace-pre-wrap">{post.body}</p>
          {/if}

          {#if post.links.length}
            <ul class="space-y-1 text-sm">
              {#each post.links as link (link.id)}
                <li id="link-{link.id}" class="flex items-center gap-2" class:pending={link.pending}>
                  <a href={link.url} class="link" target="_blank" rel="noopener noreferrer">{link.label}</a>
                  {#if post.mine}
                    <Button
                      type="button"
                      class="btn btn-ghost btn-xs"
                      onclick={() => run("remove_link", { linkId: link.id })}
                      aria-label="Remove the link {link.label}"
                    >
                      ×
                    </Button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}

          {#if post.tags.length}
            <div class="flex flex-wrap gap-1">
              {#each post.tags as tag (tag.id)}
                <span id="post-tag-{tag.id}" class="badge badge-soft gap-1" class:pending={tag.pending}>
                  {tag.name}
                  {#if post.mine}
                    <Button
                      type="button"
                      class="btn btn-ghost btn-xs px-1"
                      onclick={() => run("untag_post", { postsTagsId: tag.id })}
                      aria-label="Remove the tag {tag.name}"
                    >
                      ×
                    </Button>
                  {/if}
                </span>
              {/each}
            </div>
          {/if}

          {#if post.mine}
            <div class="flex flex-wrap items-center gap-2">
              <form
                id="link-form-{post.id}"
                class="flex items-start gap-2"
                onsubmit={submitTo("add_link", (fields) => ({ postId: post.id, ...fields }), load, showError)}
              >
                <TextInput
                  id="link-label-{post.id}"
                  name="label"
                  value=""
                  placeholder="Label"
                  aria-label="Label"
                  class="input input-sm w-32"
                />
                <TextInput
                  id="link-url-{post.id}"
                  name="url"
                  value=""
                  placeholder="https://"
                  aria-label="URL"
                  class="input input-sm w-48"
                />
                <Button type="submit" class="btn btn-sm">Add link</Button>
              </form>
              <form
                id="tag-form-{post.id}"
                class="flex items-start gap-2"
                onsubmit={submitTo("tag_post", (fields) => ({ postId: post.id, ...fields }), load, showError)}
              >
                <TextInput
                  id="tag-name-{post.id}"
                  name="name"
                  value=""
                  placeholder="Tag"
                  aria-label="Tag"
                  class="input input-sm w-32"
                />
                <Button type="submit" class="btn btn-sm">Add tag</Button>
              </form>
              <Button
                type="button"
                onclick={() => {
                  editingId = post.id;
                  editingBody = post.body;
                }}
              >
                Edit
              </Button>
              <Button type="button" onclick={() => remove(post)}>Delete</Button>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</main>
