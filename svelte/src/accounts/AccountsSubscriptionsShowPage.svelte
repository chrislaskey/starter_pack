<!-- One of the signed-in person's plans, at /accounts/subscriptions/:id:
     its price, its status, its next renewal or the day it ends, and what
     they can do with it: cancel at the end of the period, resume, undo a
     switch waiting for then, and change their card on Stripe's Customer
     Portal. Switching is on the plan's product page. See
     docs/design/purchasing.md, P6 to P9. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import {
    accountsSubscriptionsGet,
    accountsSubscriptionsPortalCreate,
    accountsSubscriptionsUpdate,
  } from "../lib/core/index.js";
  import { currentUserReload } from "../lib/current_user.js";
  import { pathParams } from "../lib/router.js";
  import { accountsProductsShowUrl, accountsSubscriptionsIndexUrl } from "../lib/routes.js";
  import { priceFormat } from "./money.js";
  import { subscriptionStatusSay } from "./subscription_status.js";

  const id = pathParams().id;
  let subscription = $state();
  let busy = $state(false);
  let inBrowser = $state(false);
  let error = $state("");

  function load() {
    accountsSubscriptionsGet(id).then(
      (shown) => (subscription = shown),
      (problem) => (error = String(problem)),
    );
  }

  // Back from Stripe's portal in the browser, in the app: ask again
  function onFocus() {
    if (!inBrowser) return;
    inBrowser = false;
    load();
    currentUserReload();
  }

  load();

  /** @param {string} at */
  const day = (at) => new Date(at).toLocaleDateString([], { dateStyle: "medium" });

  /** @param {{action: "switch_undo" | "cancel" | "resume"}} change */
  async function update(change) {
    if (change.action === "cancel" && !confirm("Cancel your plan at the end of this period?")) return;
    busy = true;
    error = "";
    try {
      subscription = await accountsSubscriptionsUpdate(id, change);
      await currentUserReload();
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }

  async function portal() {
    busy = true;
    error = "";
    try {
      const { url } = await accountsSubscriptionsPortalCreate(id);
      if (url) {
        window.location.assign(url);
        return;
      }
      inBrowser = true;
    } catch (problem) {
      error = String(problem);
    }
    busy = false;
  }
</script>

<svelte:window onfocus={onFocus} />

<main class="page">
  <AppNav />

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if subscription}
    {@const ended = Boolean(subscription.ended_at) || subscription.status === "canceled"}
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
    {#if subscription.scheduled_price && subscription.scheduled_at}
      <p>Changes to {priceFormat(subscription.scheduled_price)} on {day(subscription.scheduled_at)}.</p>
    {/if}

    {#if inBrowser}
      <p class="muted">Stripe's page is open in your browser. This page updates when you come back.</p>
    {/if}
    {#if !ended}
      <div class="flex flex-wrap gap-2">
        {#if subscription.scheduled_price}
          <button type="button" class="btn btn-sm" disabled={busy} onclick={() => update({ action: "switch_undo" })}>
            Keep {priceFormat(subscription.price)}
          </button>
        {/if}
        {#if subscription.cancel_at_period_end}
          <button type="button" class="btn btn-primary btn-sm" disabled={busy} onclick={() => update({ action: "resume" })}>
            Resume
          </button>
        {:else if subscription.gives_plan}
          <a href={accountsProductsShowUrl(subscription.product_id)} class="btn btn-sm">Switch</a>
          <button type="button" class="btn btn-ghost btn-sm" disabled={busy} onclick={() => update({ action: "cancel" })}>
            Cancel at the end of this period
          </button>
        {/if}
        <button type="button" class="btn btn-sm" disabled={busy} onclick={portal}>
          {subscription.gives_plan ? "Card and invoices" : "Update your card"}
        </button>
      </div>
    {/if}
  {/if}

  <a href={accountsSubscriptionsIndexUrl()}>Back to plans</a>
</main>
