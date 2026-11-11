<!-- The signed-in person's account, at /accounts/settings: their email,
     the way to their password, and a placeholder for the rest (plan)
     until it is built, see docs/design/multi-app.md. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { currentUserLoad } from "../lib/current_user.js";
  import { accountsPasswordEditUrl } from "../lib/routes.js";

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
  <p><a href={accountsPasswordEditUrl()} class="link">Password</a></p>
</main>
