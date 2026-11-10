<!-- Editing a community, at /communities/:id/edit; done, it shows the
     community -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { FormError, communitiesGet, communitiesUpdate } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { communitiesShowUrl } from "../lib/routes.js";
  import CommunitiesForm from "./CommunitiesForm.svelte";

  const id = pathParams().id;
  let loaded = $state(false);
  let name = $state("");
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);

  async function load() {
    try {
      name = (await communitiesGet(id)).community.name;
      loaded = true;
    } catch (problem) {
      error = String(problem);
    }
  }

  async function save() {
    busy = true;
    errors = {};
    error = "";
    try {
      await communitiesUpdate(id, name);
      await navigateToUrl(communitiesShowUrl(id));
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
  <h1>Edit community</h1>
  {#if loaded}
    <CommunitiesForm bind:name submitText="Save" {errors} {error} {busy} onsubmit={save} />
  {:else if error}
    <p class="error">{error}</p>
  {/if}
  <a href={communitiesShowUrl(id)}>Back to the community</a>
</main>
