<!-- Signing up, at /accounts/sign-up, on the web: the name and the
     email, then the code that was emailed, which proves the email, and a
     password to make or skip, see AccountsSignInSteps. Opened by a
     device with ?challenge=, it ends in a code for the device instead;
     the email is filled in from ?email=. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsSignUp } from "../lib/core/index.js";
  import { accountsSignInUrl } from "../lib/routes.js";
  import AccountsForm from "./AccountsForm.svelte";
  import AccountsSignInSteps from "./AccountsSignInSteps.svelte";

  const query = new URLSearchParams(window.location.search);
  const challenge = query.get("challenge") ?? "";

  let email = $state(query.get("email") ?? "");
  let name = $state("");
  let pending = $state("");
  let error = $state("");
  let busy = $state(false);

  async function signUp() {
    busy = true;
    error = "";
    try {
      pending = (await accountsSignUp(email, name)).pending;
    } catch (problem) {
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<main class="page">
  <AppNav />
  <h1>Sign up</h1>
  {#if challenge}
    <p class="muted">Signing up for the app on your device.</p>
  {/if}
  {#if pending}
    <AccountsSignInSteps {email} {pending} {challenge} onstartover={() => (pending = "")} />
  {:else}
    <AccountsForm bind:email bind:name withName submitText="Sign up" {error} {busy} onsubmit={signUp} />
    <p class="muted">Already have an account? <a href={accountsSignInUrl(email || undefined)} class="link">Sign in</a></p>
  {/if}
</main>
