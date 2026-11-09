<!-- The person's profile, at /profile: their picture, the one thing they
     change here for now. A picture goes straight to object storage, see
     avatarsUpload in lib/core/. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import FieldErrors from "../lib/FieldErrors.svelte";
  import { FormError, avatarsUpload, usersMe } from "../lib/core/index.js";
  import { forgetCurrentUser } from "../lib/current_user.js";

  /** @type {{email: string, avatar_url: string | null} | null} */
  let me = $state(null);
  /** @type {File | null} */
  let file = $state(null);
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);
  /** From 0 to 1 while the file uploads, else null. */
  let progress = $state(null);

  async function load() {
    try {
      me = await usersMe();
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
      me = await avatarsUpload(file, (fraction) => (progress = fraction));
      forgetCurrentUser();
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
      progress = null;
    }
  }

  load();
</script>

<main class="page">
  <AppNav />
  <h1>Profile</h1>
  {#if me}
    <div class="flex items-center gap-4">
      {#if me.avatar_url}
        <img src={me.avatar_url} alt={me.email} class="size-20 rounded-full object-cover" />
      {:else}
        <div class="bg-base-300 flex size-20 items-center justify-center rounded-full text-2xl uppercase">
          {me.email.slice(0, 1)}
        </div>
      {/if}
      <span class="muted">{me.email}</span>
    </div>
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
