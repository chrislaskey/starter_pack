<!-- The links between the pages, the current community to switch, who
     is signed in with Sign out, or Sign in and Sign up on the web and
     Accounts in the app, the theme menu, and inside the app the sync
     status. -->
<script>
  import CurrentCommunityMenu from "./CurrentCommunityMenu.svelte";
  import SyncStatus from "./SyncStatus.svelte";
  import ThemeMenu from "./ThemeMenu.svelte";
  import { accountsSignOut, hasDeviceAccounts } from "./core/index.js";
  import { currentUserForget, currentUserLoad } from "./current_user.js";
  import { navigateToUrl } from "./router.js";
  import {
    accountsIndexUrl,
    accountsSignInUrl,
    accountsSignUpUrl,
    communitiesIndexUrl,
    filesIndexUrl,
    homeUrl,
    postsIndexUrl,
    profileEditUrl,
  } from "./routes.js";

  /** @type {{email: string} | null} */
  let currentUser = $state(null);
  let busy = $state(false);
  let error = $state("");

  currentUserLoad().then(
    (user) => (currentUser = user),
    () => (currentUser = null),
  );

  async function signOut() {
    error = "";
    busy = true;
    try {
      await accountsSignOut();
      currentUserForget();
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
  {#if currentUser}
    <CurrentCommunityMenu />
    <a href={postsIndexUrl()} class="link">Posts</a>
    <a href={filesIndexUrl()} class="link">Files</a>
    <a href={communitiesIndexUrl()} class="link">Communities</a>
    <a href={profileEditUrl()} class="link">Profile</a>
  {/if}
  <span class="grow"></span>
  {#if currentUser}
    {#if hasDeviceAccounts}
      <a href={accountsIndexUrl()} class="link text-sm">{currentUser.email}</a>
    {:else}
      <span class="muted text-sm">{currentUser.email}</span>
    {/if}
    <button type="button" class="btn btn-ghost btn-sm" onclick={signOut} disabled={busy}>Sign out</button>
  {:else if hasDeviceAccounts}
    <a href={accountsIndexUrl()} class="link">Accounts</a>
  {:else}
    <a href={accountsSignInUrl()} class="link">Sign in</a>
    <a href={accountsSignUpUrl()} class="link">Sign up</a>
  {/if}
  <ThemeMenu />
</nav>
{#if error}
  <p class="error">{error}</p>
{/if}
{#if currentUser}
  <SyncStatus />
{/if}
