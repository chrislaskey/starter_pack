<!-- The signed-in person's account, at /accounts/settings: their email,
     the way to their password, and what they can buy, see
     docs/design/purchasing.md. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import AccountsNeeded from "./AccountsNeeded.svelte";
  import { currentUserLoad } from "../lib/current_user.js";
  import {
    accountsPasswordEditUrl,
    accountsProductsIndexUrl,
    accountsPurchasesIndexUrl,
    accountsSubscriptionsIndexUrl,
  } from "../lib/routes.js";

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
  {#if currentUser && currentUser.kind !== "full"}
    <AccountsNeeded kind={currentUser.kind} />
  {:else}
    {#if currentUser}
      <p>{currentUser.email}</p>
    {/if}
    <p><a href={accountsPasswordEditUrl()} class="link">Password</a></p>
    {#if currentUser?.plan}
      <p>Your plan: {currentUser.plan.product_name}</p>
    {/if}
    <p><a href={accountsSubscriptionsIndexUrl()} class="link">Plans</a></p>
    <p><a href={accountsProductsIndexUrl()} class="link">Plans and products for sale</a></p>
    <p><a href={accountsPurchasesIndexUrl()} class="link">Purchases</a></p>
  {/if}
</main>
