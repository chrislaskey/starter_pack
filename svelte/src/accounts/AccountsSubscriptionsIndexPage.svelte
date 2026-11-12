<!-- The signed-in person's plans, at /accounts/subscriptions: the
     current one and the ended ones, newest first. See
     docs/design/purchasing.md, P6. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsSubscriptionsList } from "../lib/core/index.js";
  import { accountsProductsIndexUrl, accountsSubscriptionsShowUrl } from "../lib/routes.js";
  import { priceFormat } from "./money.js";
  import { subscriptionStatusSay } from "./subscription_status.js";

  let subscriptions = $state([]);
  let loaded = $state(false);
  let error = $state("");

  accountsSubscriptionsList().then(
    (list) => {
      subscriptions = list;
      loaded = true;
    },
    (problem) => (error = String(problem)),
  );
</script>

<main class="page">
  <AppNav />
  <h1>Plans</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if loaded && subscriptions.length === 0}
    <p class="muted">No plan yet. <a href={accountsProductsIndexUrl()} class="link">See the plans</a></p>
  {/if}

  <ul class="flex flex-col gap-2">
    {#each subscriptions as subscription (subscription.id)}
      <li>
        <a href={accountsSubscriptionsShowUrl(subscription.id)} class="link">{subscription.product_name}</a>
        <span class="muted">· {priceFormat(subscription.price)} · {subscriptionStatusSay(subscription.status)}</span>
      </li>
    {/each}
  </ul>
</main>
