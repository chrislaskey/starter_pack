<!-- The ops app's landing page, at /ops: for staff, a placeholder until
     ops is built; for anyone else, why they cannot use it. -->
<script>
  import { accountsSignInUrl, homeUrl } from "../lib/routes.js";
  import { opsCurrentStaffGet } from "./api.js";

  /** @type {Awaited<ReturnType<typeof opsCurrentStaffGet>> | null} */
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
  {:else if staff === "signed-out"}
    <p><a href={accountsSignInUrl()} class="link">Sign in</a> to use ops.</p>
  {:else if staff === "not-staff"}
    <p>Only staff can use this.</p>
  {:else if staff}
    <p class="muted">Nothing here yet.</p>
  {/if}
</main>
