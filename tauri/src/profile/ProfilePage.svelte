<!-- The user's own profile, as App.Web.ProfileLive shows it. A save
     queues only the fields that changed; a field someone else changed
     first shows with the other conflicts above, to reapply or let go. -->
<script>
  import { invoke } from "@tauri-apps/api/core";
  import Button from "$phoenix/components/Button.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import TextareaInput from "$phoenix/components/TextareaInput.svelte";
  import TextInput from "$phoenix/components/TextInput.svelte";
  import ThemeToggle from "$phoenix/components/ThemeToggle.svelte";
  import AppNav from "../lib/AppNav.svelte";
  import SyncBar from "../lib/SyncBar.svelte";
  import { write } from "../lib/write.js";

  let page = $state();
  let error = $state("");
  let saved = $state("");
  // Bumped after a save, so the form shows the profile as saved
  let formKey = $state(0);

  async function load() {
    try {
      page = await invoke("get_profile_page");
    } catch (problem) {
      error = String(problem);
    }
  }

  async function save(event) {
    event.preventDefault();
    const fields = new FormData(event.currentTarget);
    error = await write(
      "edit_profile",
      { displayName: fields.get("display_name"), bio: fields.get("bio") },
      load,
    );
    saved = error ? "" : "Profile saved.";
    if (!error) formKey++;
  }
</script>

<main class="page">
  <AppNav />
  <Header>
    Profile
    {#snippet subtitle()}
      Your public details
    {/snippet}
    {#snippet actions()}
      <ThemeToggle />
    {/snippet}
  </Header>

  <SyncBar onrefresh={load} />

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if saved}
    <p class="muted">{saved}{page?.profile?.pending ? " Waiting to sync." : ""}</p>
  {/if}

  {#if page && !page.profile}
    <p class="muted">This device hasn't synced your profile yet.</p>
  {/if}

  {#if page?.profile}
    {#key formKey}
      <form id="profile-form" class="space-y-4" onsubmit={save}>
        <TextInput
          id="profile_display_name"
          name="display_name"
          label="Display name"
          value={page.profile.display_name}
        />
        <TextareaInput id="profile_bio" name="bio" label="Bio" value={page.profile.bio} />
        <Button variant="primary" type="submit">Save</Button>
      </form>
    {/key}
  {/if}
</main>
