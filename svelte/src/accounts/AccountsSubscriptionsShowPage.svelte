<!-- One of the signed-in person's plans, at /accounts/subscriptions/:id:
     its price, its status, and its next renewal or the day it ends. See
     docs/design/purchasing.md, P6. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsSubscriptionsGet } from "../lib/core/index.js";
  import { pathParams } from "../lib/router.js";
  import { accountsSubscriptionsIndexUrl } from "../lib/routes.js";
  import { priceFormat } from "./money.js";
  import { subscriptionStatusSay } from "./subscription_status.js";

  const id = pathParams().id;
  let subscription = $state();
  let error = $state("");

  accountsSubscriptionsGet(id).then(
    (shown) => (subscription = shown),
    (problem) => (error = String(problem)),
  );

  /** @param {string} at */
  const day = (at) => new Date(at).toLocaleDateString([], { dateStyle: "medium" });
</script>

<main class="page">
  <AppNav />

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if subscription}
    <h1>{subscription.product_name}</h1>
    <p>{priceFormat(subscription.price)}</p>
    <p>{subscriptionStatusSay(subscription.status)}</p>
    {#if subscription.ended_at}
      <p class="muted">Ended {day(subscription.ended_at)}</p>
    {:else if subscription.current_period_end && subscription.cancel_at_period_end}
      <p class="muted">Ends {day(subscription.current_period_end)}</p>
    {:else if subscription.current_period_end && subscription.gives_plan}
      <p class="muted">Renews {day(subscription.current_period_end)}</p>
    {/if}
  {/if}

  <a href={accountsSubscriptionsIndexUrl()}>Back to plans</a>
</main>
