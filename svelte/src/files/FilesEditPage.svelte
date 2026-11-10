<!-- Renaming a file, at /files/:id/edit, the one thing about a file
     that changes; done, it shows the file -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import FieldErrors from "../lib/FieldErrors.svelte";
  import { FormError, filesGet, filesUpdate } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { filesShowUrl } from "../lib/routes.js";

  const id = pathParams().id;
  let loaded = $state(false);
  let name = $state("");
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);

  async function load() {
    try {
      name = (await filesGet(id)).file.name;
      loaded = true;
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {SubmitEvent} event */
  async function save(event) {
    event.preventDefault();
    busy = true;
    errors = {};
    error = "";
    try {
      await filesUpdate(id, name);
      await navigateToUrl(filesShowUrl(id));
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }

  load();
</script>

<main class="page">
  <AppNav />
  <h1>Rename file</h1>
  {#if loaded}
    <form onsubmit={save} class="flex flex-col gap-3">
      <label class="flex flex-col gap-1">
        <span>Name</span>
        <input type="text" class="input w-full" bind:value={name} disabled={busy} />
        <FieldErrors {errors} fields={["name"]} />
      </label>
      {#if error && Object.keys(errors).length === 0}
        <p class="error">{error}</p>
      {/if}
      <div>
        <button type="submit" class="btn btn-primary" disabled={busy}>Save</button>
      </div>
    </form>
  {:else if error}
    <p class="error">{error}</p>
  {/if}
  <a href={filesShowUrl(id)}>Back to the file</a>
</main>
