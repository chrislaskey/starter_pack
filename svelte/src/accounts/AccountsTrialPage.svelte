<!-- "Try it now", at /accounts/trial, on the web: the app for a day with
     no sign-up, as a trial user, see
     docs/design/accounts-local-and-trial-users.md. The trial starts from
     the button, never from opening the page. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsTrialCreate, accountsTrialGet } from "../lib/core/index.js";
  import { currentUserForget } from "../lib/current_user.js";
  import { navigateToUrl } from "../lib/router.js";
  import { accountsSignInUrl, accountsSignUpUrl, postsIndexUrl } from "../lib/routes.js";

  /** Whether a trial can start; null until the server answered. */
  let available = $state(/** @type {boolean | null} */ (null));
  let error = $state("");
  let busy = $state(false);

  accountsTrialGet().then(
    (answer) => (available = answer),
    (problem) => (error = problem instanceof Error ? problem.message : String(problem)),
  );

  async function start() {
    busy = true;
    error = "";
    try {
      await accountsTrialCreate();
      currentUserForget();
      await navigateToUrl(postsIndexUrl());
    } catch (problem) {
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<main class="page">
  <AppNav />
  <h1>Try it now</h1>
  {#if available === false}
    <p>Trials are not available right now.</p>
    <p><a href={accountsSignUpUrl()} class="btn btn-primary">Sign up</a></p>
  {:else}
    <p>
      Use the app for 24 hours with no account. What you make is deleted after that, unless you sign up, which
      keeps all of it.
    </p>
    <p class="muted text-sm">A trial stays in this browser, and has no files or picture until you sign up.</p>
    {#if error}
      <p class="error">{error}</p>
    {/if}
    <div class="flex flex-wrap items-center gap-3">
      <button type="button" class="btn btn-primary" onclick={start} disabled={busy || available === null} data-trial>
        Start a trial
      </button>
      <a href={accountsSignUpUrl()} class="link text-sm">Sign up</a>
      <a href={accountsSignInUrl()} class="link text-sm">Sign in</a>
    </div>
  {/if}
</main>
