<!-- The signed-in person's password, at /accounts/password: their first,
     or a new one in place of the current one. A password is optional,
     asked for after the emailed code at a sign-in once the account has
     one. The server keeps it, so in the app this needs a connection. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsPasswordUpdate, accountsSecondFactors } from "../lib/core/index.js";
  import { accountsSettingsUrl } from "../lib/routes.js";

  /** Whether the account has a password; null until the server answered. */
  let hasPassword = $state(/** @type {boolean | null} */ (null));
  let current = $state("");
  let password = $state("");
  let error = $state("");
  let saved = $state(false);
  let busy = $state(false);

  accountsSecondFactors().then(
    (factors) => (hasPassword = factors.includes("password")),
    (problem) => (error = problem instanceof Error ? problem.message : String(problem)),
  );

  /** @param {SubmitEvent} event */
  async function save(event) {
    event.preventDefault();
    busy = true;
    error = "";
    saved = false;
    try {
      await accountsPasswordUpdate(hasPassword ? current : null, password);
      hasPassword = true;
      saved = true;
      current = "";
      password = "";
    } catch (problem) {
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<main class="page">
  <AppNav />
  <h1>{hasPassword ? "Change password" : "Create a password"}</h1>
  {#if hasPassword !== null}
    <form onsubmit={save} class="flex flex-col gap-3">
      {#if hasPassword}
        <input bind:value={current} type="password" autocomplete="current-password" class="input w-full" aria-label="Current password" placeholder="Current password" required />
        <p class="muted text-sm">Forgot it? Sign out and sign in again; the password step offers a reset when this app allows one.</p>
      {:else}
        <p>A password is optional. With one, signing in asks for it after the emailed code.</p>
      {/if}
      <input
        bind:value={password}
        type="password"
        autocomplete="new-password"
        minlength="12"
        class="input w-full"
        aria-label="New password"
        placeholder="New password, 12 characters or more"
        required
      />
      {#if saved}
        <p class="muted" data-saved>Your password is saved.</p>
      {/if}
      {#if error}
        <p class="error">{error}</p>
      {/if}
      <div class="flex flex-wrap items-center gap-3">
        <button type="submit" class="btn btn-primary" disabled={busy}>{hasPassword ? "Change password" : "Create password"}</button>
        <a href={accountsSettingsUrl()} class="link text-sm">Back to account</a>
      </div>
    </form>
  {:else if error}
    <p class="error">{error}</p>
  {/if}
</main>
