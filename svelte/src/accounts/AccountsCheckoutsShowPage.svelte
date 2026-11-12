<!-- Where Stripe's page returns to, at /accounts/checkouts/:id: the
     checkout as Stripe has it now. A payment that settles later shows
     Processing, and the page asks again every few seconds until it is
     paid or failed. In the app, Stripe's page is in the browser, and the
     page asks again when the app has the focus. See
     docs/design/purchasing.md, P3. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsCheckoutsGet, hasDeviceAccounts } from "../lib/core/index.js";
  import { currentUserReload } from "../lib/current_user.js";
  import { pathParams } from "../lib/router.js";
  import {
    accountsProductsShowUrl,
    accountsPurchasesShowUrl,
    accountsSubscriptionsIndexUrl,
  } from "../lib/routes.js";

  const ASK_AGAIN_MS = 3000;
  const ASK_AT_MOST = 40;

  const id = pathParams().id;
  let checkout = $state();
  let error = $state("");
  let asked = 0;
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let timer;

  async function load() {
    try {
      checkout = await accountsCheckoutsGet(id);
      asked += 1;
      const pending = checkout.status === "open" || checkout.status === "processing";
      if (pending && asked < ASK_AT_MOST) timer = setTimeout(load, ASK_AGAIN_MS);
      if (checkout.status === "paid") await currentUserReload();
    } catch (problem) {
      error = String(problem);
    }
  }

  // Back from paying in the browser, in the app: ask again
  function onFocus() {
    clearTimeout(timer);
    asked = 0;
    load();
  }

  $effect(() => () => clearTimeout(timer));
  load();
</script>

<svelte:window onfocus={onFocus} />

<main class="page">
  <AppNav />
  <h1>Checkout</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if checkout}
    {#if checkout.status === "paid"}
      <p>Thank you. {checkout.product_name} is yours.</p>
      {#if checkout.purchase_id}
        <p><a href={accountsPurchasesShowUrl(checkout.purchase_id)} class="link">See the purchase</a></p>
      {:else if checkout.kind === "plan"}
        <p><a href={accountsSubscriptionsIndexUrl()} class="link">See your plan</a></p>
      {/if}
    {:else if checkout.status === "processing"}
      <p>Your payment for {checkout.product_name} is processing. This page updates when it is done.</p>
    {:else if checkout.status === "open" && hasDeviceAccounts}
      <p>Finish paying for {checkout.product_name} in your browser. This page updates when you come back.</p>
    {:else if checkout.status === "open"}
      <p>{checkout.product_name} is not paid yet.</p>
      <p><a href={accountsProductsShowUrl(checkout.product_id)} class="link">Back to {checkout.product_name}</a></p>
    {:else}
      <p>
        {checkout.status === "failed" ? "The payment did not go through" : "This checkout expired"}, and
        nothing was charged.
      </p>
      <p><a href={accountsProductsShowUrl(checkout.product_id)} class="link">Try again</a></p>
    {/if}
  {/if}
</main>
