<!-- Signing in, at /accounts/sign-in, on the web: the email, then the
     code that was emailed and the password if the account has one, see
     AccountsSignInSteps. Opened by a device with ?challenge=, it ends in
     a code for the device instead, see docs/design/accounts-sign-in.md.
     The email is filled in from ?email=. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsSignIn } from "../lib/core/index.js";
  import { accountsSignUpUrl } from "../lib/routes.js";
  import AccountsForm from "./AccountsForm.svelte";
  import AccountsSignInSteps from "./AccountsSignInSteps.svelte";

  const query = new URLSearchParams(window.location.search);
  const challenge = query.get("challenge") ?? "";

  let email = $state(query.get("email") ?? "");
  let pending = $state("");
  let error = $state("");
  let busy = $state(false);

  async function signIn() {
    busy = true;
    error = "";
    try {
      pending = (await accountsSignIn(email)).pending;
    } catch (problem) {
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<main class="page">
  <AppNav />
  <h1>Sign in</h1>
  {#if challenge}
    <p class="muted">Signing in for the app on your device.</p>
  {/if}
  {#if pending}
    <AccountsSignInSteps {email} {pending} {challenge} onstartover={() => (pending = "")} />
  {:else}
    <AccountsForm bind:email submitText="Sign in" {error} {busy} onsubmit={signIn} />
    <p class="muted">New here? <a href={accountsSignUpUrl(email || undefined)} class="link">Sign up</a></p>
  {/if}
</main>
