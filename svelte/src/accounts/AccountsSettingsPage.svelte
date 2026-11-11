<!-- The signed-in person's account, at /accounts/settings: their email,
     and a placeholder for the rest (password, plan) until it is built,
     see docs/design/multi-app.md. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { currentUserLoad } from "../lib/current_user.js";

  /** @type {import("../lib/current_user.js").User | null} */
  let currentUser = $state(null);

  currentUserLoad().then(
    (user) => (currentUser = user),
    () => (currentUser = null),
  );
</script>

<main class="page">
  <AppNav />
  <h1>Account</h1>
  {#if currentUser}
    <p>{currentUser.email}</p>
  {/if}
  <p class="muted">Nothing else here yet.</p>
</main>
