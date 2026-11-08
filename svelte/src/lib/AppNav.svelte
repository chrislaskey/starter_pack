<!-- The links between the pages, who is signed in with Sign out, or Sign
     in and Sign up, the theme menu, and inside the app the sync status
     and the accounts on the device. -->
<script>
  import SyncStatus from "./SyncStatus.svelte";
  import ThemeMenu from "./ThemeMenu.svelte";
  import { accountsSignOut, hasDeviceAccounts } from "./core/index.js";
  import { currentUser, forgetCurrentUser } from "./current_user.js";
  import { navigateToUrl } from "./router.js";
  import { accountsIndexUrl, accountsSignInUrl, accountsSignUpUrl, communitiesIndexUrl, homeUrl, postsIndexUrl } from "./routes.js";

  /** @type {{email: string} | null} */
  let me = $state(null);
  let busy = $state(false);
  let error = $state("");

  currentUser().then(
    (user) => (me = user),
    () => (me = null),
  );

  async function signOut() {
    error = "";
    busy = true;
    try {
      await accountsSignOut();
      forgetCurrentUser();
      await navigateToUrl(hasDeviceAccounts ? accountsIndexUrl() : accountsSignInUrl());
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<nav class="flex flex-wrap items-center gap-4">
  <a href={homeUrl()} class="link">Home</a>
  {#if me}
    <a href={postsIndexUrl()} class="link">Posts</a>
    <a href={communitiesIndexUrl()} class="link">Communities</a>
  {/if}
  <span class="grow"></span>
  {#if me}
    {#if hasDeviceAccounts}
      <a href={accountsIndexUrl()} class="link text-sm">{me.email}</a>
    {:else}
      <span class="muted text-sm">{me.email}</span>
    {/if}
    <button type="button" class="btn btn-ghost btn-sm" onclick={signOut} disabled={busy}>Sign out</button>
  {:else}
    {#if hasDeviceAccounts}
      <a href={accountsIndexUrl()} class="link">Accounts</a>
    {/if}
    <a href={accountsSignInUrl()} class="link">Sign in</a>
    <a href={accountsSignUpUrl()} class="link">Sign up</a>
  {/if}
  <ThemeMenu />
</nav>
{#if error}
  <p class="error">{error}</p>
{/if}
{#if me}
  <SyncStatus />
{/if}
