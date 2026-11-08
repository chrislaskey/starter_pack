<!-- The links between the pages, who is signed in with Sign out, or Sign
     in and Sign up, the theme menu, and inside the app the sync status.
     A sign-out that would lose changes asks first, AccountsSignOutDialog. -->
<script>
  import AccountsSignOutDialog from "../accounts/AccountsSignOutDialog.svelte";
  import SyncStatus from "./SyncStatus.svelte";
  import ThemeMenu from "./ThemeMenu.svelte";
  import { accountsSignOut } from "./core/index.js";
  import { currentUser, forgetCurrentUser } from "./current_user.js";
  import { navigateToUrl } from "./router.js";
  import { accountsSignInUrl, accountsSignUpUrl, communitiesIndexUrl, homeUrl, postsIndexUrl } from "./routes.js";

  /** @type {{email: string} | null} */
  let me = $state(null);
  /** @type {{pending: number, conflicts: number} | null} */
  let loss = $state(null);
  let busy = $state(false);
  let error = $state("");

  currentUser().then(
    (user) => (me = user),
    () => (me = null),
  );

  /** @param {boolean} losingChanges */
  async function signOut(losingChanges) {
    error = "";
    busy = true;
    try {
      const answer = await accountsSignOut(losingChanges);
      if (answer.outcome === "changes_would_be_lost") {
        loss = { pending: answer.pending, conflicts: answer.conflicts };
        return;
      }
      loss = null;
      forgetCurrentUser();
      await navigateToUrl(accountsSignInUrl());
    } catch (problem) {
      loss = null;
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
    <span class="muted text-sm">{me.email}</span>
    <button type="button" class="btn btn-ghost btn-sm" onclick={() => signOut(false)} disabled={busy}>Sign out</button>
  {:else}
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
  <AccountsSignOutDialog {loss} {busy} onsignout={() => signOut(true)} oncancel={() => (loss = null)} />
{/if}
