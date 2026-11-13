<!-- What is for sale, at /accounts/products: plans, then one-time
     products, each with its prices and a link to its page, where it is
     bought; on a phone with its store's prices, and on the App Store
     with Restore purchases, as Apple asks. See docs/design/purchasing.md,
     P1, and docs/design/purchasing-phones.md, M2. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsProductsList, accountsStoreTransactionsRestore } from "../lib/core/index.js";
  import { currentUserReload } from "../lib/current_user.js";
  import { accountsProductsShowUrl } from "../lib/routes.js";
  import { priceFormat } from "./money.js";
  import { storePriceFormat } from "./stores.js";

  let products = $state([]);
  let loaded = $state(false);
  let error = $state("");
  let restoring = $state(false);
  let restored = $state("");
  const store = $derived(products[0]?.store ?? "stripe");

  async function restore() {
    restoring = true;
    error = "";
    restored = "";
    try {
      const { recorded, refused } = await accountsStoreTransactionsRestore();
      await currentUserReload();
      restored = recorded === 1 ? "1 purchase restored." : `${recorded} purchases restored.`;
      if (refused.length > 0) error = refused.join(" ");
    } catch (problem) {
      error = String(problem);
    } finally {
      restoring = false;
    }
  }

  accountsProductsList().then(
    (list) => {
      products = list;
      loaded = true;
    },
    (problem) => (error = String(problem)),
  );
</script>

<main class="page">
  <AppNav />
  <h1>Plans and products</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if loaded && products.length === 0}
    <p class="muted">Nothing is for sale yet.</p>
  {/if}

  <ul class="flex flex-col gap-4">
    {#each products as product (product.id)}
      <li>
        <h2><a href={accountsProductsShowUrl(product.id)} class="link">{product.name}</a></h2>
        {#if product.description}
          <p>{product.description}</p>
        {/if}
        <p class="muted">
          {product.kind === "plan" ? "Plan" : "One-time"} ·
          {product.prices.map((price) => storePriceFormat(price, priceFormat)).join(" or ")}
        </p>
      </li>
    {/each}
  </ul>

  {#if store === "app_store"}
    <div class="mt-6 flex flex-wrap items-center gap-3">
      <button type="button" class="btn btn-sm" disabled={restoring} onclick={restore}>Restore purchases</button>
      {#if restored}
        <span class="muted">{restored}</span>
      {/if}
    </div>
  {/if}
</main>
