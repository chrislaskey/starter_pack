<!-- The user's tags, with how many posts carry each, as App.Web.TagsLive
     shows them: make one, rename one, or delete one, which takes it off
     its posts and leaves the posts. -->
<script>
  import { invoke } from "@tauri-apps/api/core";
  import Button from "$phoenix/components/Button.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import TextInput from "$phoenix/components/TextInput.svelte";
  import ThemeToggle from "$phoenix/components/ThemeToggle.svelte";
  import AppNav from "../lib/AppNav.svelte";
  import { postsIndexUrl } from "../lib/routes.js";
  import SyncBar from "../lib/SyncBar.svelte";
  import { submitTo, write } from "../lib/write.js";

  let page = $state();
  let error = $state("");

  async function load() {
    try {
      page = await invoke("get_tags_page");
    } catch (problem) {
      error = String(problem);
    }
  }

  const showError = (message) => (error = message);

  async function remove(tag) {
    if (!confirm(`Delete ${tag.name}? It comes off every post that has it.`)) return;
    error = await write("delete_tag", { tagId: tag.id }, load);
  }

  // Renaming keeps what the user typed when it is refused, as the web does
  async function rename(event, tag) {
    event.preventDefault();
    const name = new FormData(event.currentTarget).get("name");
    error = await write("rename_tag", { tagId: tag.id, name }, load);
  }
</script>

<main class="page">
  <AppNav />
  <Header>
    Tags
    {#snippet subtitle()}
      Yours, for your own posts
    {/snippet}
    {#snippet actions()}
      <div class="flex items-center gap-4">
        <a href={postsIndexUrl()} class="link">Posts</a>
        <ThemeToggle />
      </div>
    {/snippet}
  </Header>

  <SyncBar onrefresh={load} />

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <form
    id="new-tag-form"
    class="flex items-start gap-2"
    onsubmit={submitTo("create_tag", (fields) => fields, load, showError)}
  >
    <TextInput id="new-tag-name" name="name" value="" placeholder="New tag" aria-label="New tag" />
    <Button type="submit" variant="primary">Make tag</Button>
  </form>

  {#if page}
    {#if page.tags.length === 0}
      <p class="text-base-content/60">No tags yet.</p>
    {/if}

    <ul id="tags" class="divide-y divide-base-200">
      {#each page.tags as tag (tag.id)}
        <li id="tag-{tag.id}" class="flex flex-wrap items-center gap-3 py-3" class:pending={tag.pending}>
          <span class="font-medium">{tag.name}</span>
          <span class="text-sm text-base-content/60">{tag.posts === 1 ? "1 post" : `${tag.posts} posts`}</span>
          <form id="rename-tag-{tag.id}" class="flex items-start gap-2" onsubmit={(event) => rename(event, tag)}>
            {#key tag.name}
              <TextInput
                id="tag-name-{tag.id}"
                name="name"
                value={tag.name}
                aria-label="New name for {tag.name}"
                class="input input-sm"
              />
            {/key}
            <Button type="submit" class="btn btn-sm">Rename</Button>
          </form>
          <Button type="button" onclick={() => remove(tag)}>Delete</Button>
        </li>
      {/each}
    </ul>
  {/if}
</main>
