// Phoenix's generated Svelte components turn `phx-click`, `phx-submit` and
// `phx-change` into calls to the `app:emit` function in context (see
// ../phoenix/priv/svelte/README.md). What an event does here isn't decided
// yet, so for now it's logged.

import { setContext } from "svelte";
import { log } from "./log.svelte.js";

/** Provides `app:emit` to the calling component and its children. */
export function setAppEmitContext() {
  setContext("app:emit", (name, payload) => {
    const data = payload instanceof FormData ? Object.fromEntries(payload) : payload;
    log(`Component emitted ${name}`, data);
  });
}
