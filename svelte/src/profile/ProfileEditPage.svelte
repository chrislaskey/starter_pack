<!-- The person's profile, at /profile: their name and their picture. A
     name is written where the page runs, so the app changes it offline
     and syncs it later; a picture goes straight to object storage, see
     avatarsUpload in lib/core/. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import FieldErrors from "../lib/FieldErrors.svelte";
  import { FormError, avatarsUpload, currentUserGet, currentUserUpdate } from "../lib/core/index.js";
  import { currentUserForget } from "../lib/current_user.js";

  /** @type {{name: string, avatar_url?: string | null} | null} */
  let currentUser = $state(null);
  /** @type {File | null} */
  let file = $state(null);
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);
  let name = $state("");
  let nameError = $state("");
  let nameBusy = $state(false);
  /** From 0 to 1 while the file uploads, else null. */
  let progress = $state(null);

  async function load() {
    try {
      currentUser = await currentUserGet();
      name = currentUser?.name ?? "";
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {Event & {currentTarget: HTMLInputElement}} event */
  function pick(event) {
    file = event.currentTarget.files?.[0] ?? null;
    errors = {};
    error = "";
  }

  /** @param {SubmitEvent} event */
  async function save(event) {
    event.preventDefault();
    if (!file) return;
    busy = true;
    errors = {};
    error = "";
    try {
      currentUser = await avatarsUpload(file, (fraction) => (progress = fraction));
      currentUserForget();
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
      progress = null;
    }
  }

  /** @param {SubmitEvent} event */
  async function saveName(event) {
    event.preventDefault();
    nameBusy = true;
    nameError = "";
    try {
      currentUser = await currentUserUpdate(name);
      name = currentUser?.name ?? "";
      currentUserForget();
    } catch (problem) {
      nameError = problem instanceof Error ? problem.message : String(problem);
    } finally {
      nameBusy = false;
    }
  }

  load();
</script>

<main class="page">
  <AppNav />
  <h1>Profile</h1>
  {#if currentUser}
    <div class="flex items-center gap-4">
      {#if currentUser.avatar_url}
        <img src={currentUser.avatar_url} alt={currentUser.name} class="size-20 rounded-full object-cover" />
      {:else}
        <div class="bg-base-300 flex size-20 items-center justify-center rounded-full text-2xl uppercase">
          {currentUser.name.slice(0, 1)}
        </div>
      {/if}
      <span>{currentUser.name}</span>
    </div>
    <form onsubmit={saveName} class="flex flex-col gap-3">
      <label class="flex flex-col gap-1">
        <span>Name</span>
        <input bind:value={name} type="text" autocomplete="name" class="input w-full" maxlength="80" required />
      </label>
      {#if nameError}
        <p class="error">{nameError}</p>
      {/if}
      <div>
        <button type="submit" class="btn btn-primary" disabled={nameBusy || name.trim() === currentUser.name}>Save name</button>
      </div>
    </form>
    <form onsubmit={save} class="flex flex-col gap-3">
      <label class="flex flex-col gap-1">
        <span>Picture</span>
        <input
          type="file"
          accept="image/png,image/jpeg,image/gif,image/webp"
          class="file-input w-full"
          onchange={pick}
          disabled={busy}
        />
        <FieldErrors {errors} fields={["content_type", "size"]} />
      </label>
      {#if progress !== null}
        <progress class="progress w-full" value={progress} max="1"></progress>
      {/if}
      {#if error && Object.keys(errors).length === 0}
        <p class="error">{error}</p>
      {/if}
      <div>
        <button type="submit" class="btn btn-primary" disabled={busy || !file}>Save picture</button>
      </div>
    </form>
  {:else if error}
    <p class="error">{error}</p>
  {/if}
</main>
