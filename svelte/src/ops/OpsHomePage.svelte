<!-- The ops app's landing page, at /ops: a placeholder until ops is
     built. Only staff are given the page, see axum/src/ops.rs. -->
<script>
  import { homeUrl } from "../lib/routes.js";
  import { opsCurrentStaffGet } from "./api.js";

  /** @type {{user_id: string, inserted_at: string} | null} */
  let staff = $state(null);
  let error = $state("");

  opsCurrentStaffGet().then(
    (answer) => (staff = answer),
    (problem) => (error = String(problem)),
  );
</script>

<main class="page">
  <nav class="flex flex-wrap items-center gap-4">
    <span class="font-semibold">Ops</span>
    <span class="grow"></span>
    <a href={homeUrl()} class="link">Back to the app</a>
  </nav>
  <h1>Ops</h1>
  {#if error}
    <p class="error">{error}</p>
  {:else if staff}
    <p class="muted">Nothing here yet.</p>
  {/if}
</main>
