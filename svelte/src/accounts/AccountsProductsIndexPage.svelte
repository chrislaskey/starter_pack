<!-- What is for sale, at /accounts/products: plans, then one-time
     products, each with its prices and a link to its page, where it is
     bought. See docs/design/purchasing.md, P1. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsProductsList } from "../lib/core/index.js";
  import { accountsProductsShowUrl } from "../lib/routes.js";
  import { priceFormat } from "./money.js";

  let products = $state([]);
  let loaded = $state(false);
  let error = $state("");

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
          {product.prices.map(priceFormat).join(" or ")}
        </p>
      </li>
    {/each}
  </ul>
</main>
