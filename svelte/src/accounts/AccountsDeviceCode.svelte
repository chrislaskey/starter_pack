<!-- The end of a sign-in a device started in this browser: the code the
     person pastes into the app, see docs/design/accounts-sign-in.md. -->
<script>
  /** @type {{code: string}} */
  let { code } = $props();
  let copied = $state(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      copied = true;
    } catch {
      copied = false;
    }
  }
</script>

<div class="flex flex-col gap-3">
  <p>You are signed in. Paste this code into the app to finish there, then you can close this tab.</p>
  <div class="flex flex-wrap items-center gap-3">
    <code class="rounded bg-base-200 px-3 py-2 text-lg break-all select-all" data-device-code>{code}</code>
    <button type="button" class="btn btn-sm" onclick={copy}>{copied ? "Copied" : "Copy"}</button>
  </div>
  <p class="muted text-sm">The code works once and for five minutes.</p>
</div>
