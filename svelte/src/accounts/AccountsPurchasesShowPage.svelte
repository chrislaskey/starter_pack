<!-- One of the signed-in person's purchases, at /accounts/purchases/:id.
     The receipt is the email Stripe sent. See docs/design/purchasing.md, P5. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsPurchasesGet } from "../lib/core/index.js";
  import { pathParams } from "../lib/router.js";
  import { accountsPurchasesIndexUrl } from "../lib/routes.js";
  import { moneyFormat } from "./money.js";

  const id = pathParams().id;
  let purchase = $state();
  let error = $state("");

  accountsPurchasesGet(id).then(
    (shown) => (purchase = shown),
    (problem) => (error = String(problem)),
  );
</script>

<main class="page">
  <AppNav />

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if purchase}
    <h1>{purchase.product_name}</h1>
    <p>{moneyFormat(purchase.amount, purchase.currency)}, tax included</p>
    <p class="muted">Bought {new Date(purchase.inserted_at).toLocaleString()}</p>
    {#if purchase.refunded_at}
      <p>Refunded {new Date(purchase.refunded_at).toLocaleString()}</p>
    {/if}
    <p class="muted">The receipt is in the email sent when you paid.</p>
  {/if}

  <a href={accountsPurchasesIndexUrl()}>Back to purchases</a>
</main>
