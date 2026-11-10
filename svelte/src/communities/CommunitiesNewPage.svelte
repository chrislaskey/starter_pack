<!-- Making a community, at /communities/new, which the person joins;
     done, it shows the community -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { FormError, communitiesCreate } from "../lib/core/index.js";
  import { navigateToUrl } from "../lib/router.js";
  import { communitiesIndexUrl, communitiesShowUrl } from "../lib/routes.js";
  import CommunitiesForm from "./CommunitiesForm.svelte";

  let name = $state("");
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);

  async function save() {
    busy = true;
    errors = {};
    error = "";
    try {
      const community = await communitiesCreate(name);
      await navigateToUrl(communitiesShowUrl(community.id));
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<main class="page">
  <AppNav />
  <h1>New community</h1>
  <CommunitiesForm bind:name submitText="Create" {errors} {error} {busy} onsubmit={save} />
  <a href={communitiesIndexUrl()}>Back to communities</a>
</main>
