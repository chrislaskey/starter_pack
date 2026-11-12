<!-- The signed-in person's one-time purchases, newest first, at
     /accounts/purchases. See docs/design/purchasing.md, P5. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsPurchasesList } from "../lib/core/index.js";
  import { accountsProductsIndexUrl, accountsPurchasesShowUrl } from "../lib/routes.js";
  import { moneyFormat } from "./money.js";

  let purchases = $state([]);
  let loaded = $state(false);
  let error = $state("");

  accountsPurchasesList().then(
    (list) => {
      purchases = list;
      loaded = true;
    },
    (problem) => (error = String(problem)),
  );
</script>

<main class="page">
  <AppNav />
  <h1>Purchases</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if loaded && purchases.length === 0}
    <p class="muted">Nothing bought yet. <a href={accountsProductsIndexUrl()} class="link">See what is for sale</a></p>
  {/if}

  <ul class="flex flex-col gap-2">
    {#each purchases as purchase (purchase.id)}
      <li>
        <a href={accountsPurchasesShowUrl(purchase.id)} class="link">{purchase.product_name}</a>
        <span class="muted">
          · {moneyFormat(purchase.amount, purchase.currency)} · {new Date(purchase.inserted_at).toLocaleDateString()}
          {#if purchase.refunded_at}· Refunded{/if}
        </span>
      </li>
    {/each}
  </ul>
</main>
