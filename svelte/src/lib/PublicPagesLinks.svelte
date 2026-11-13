<!-- Links to the terms and the privacy policy, public pages: a new tab on
     the web, the system browser in the apps, which never show them. -->
<script>
  import { publicPagesOpen } from "./core/index.js";

  /** @param {MouseEvent} event @param {"terms" | "privacy"} page */
  function open(event, page) {
    event.preventDefault();
    publicPagesOpen(page);
  }
</script>

<a href="/terms" class="link" onclick={(event) => open(event, "terms")}>Terms of Service</a> and
<a href="/privacy" class="link" onclick={(event) => open(event, "privacy")}>Privacy Policy</a>
