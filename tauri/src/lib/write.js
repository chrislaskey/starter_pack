// A page's writes: the command, then the page read again. Each returns the
// error message for the page to show, or "" when the write worked.

import { invoke } from "@tauri-apps/api/core";

/**
 * @param {string} command
 * @param {Record<string, unknown>} args
 * @param {() => Promise<void>} reload the page's read
 */
export async function write(command, args, reload) {
  try {
    await invoke(command, args);
    await reload();
    return "";
  } catch (problem) {
    return String(problem);
  }
}

/**
 * The submit handler of a small form: its fields by name go to the
 * command, and the form is cleared once the write worked.
 * @param {string} command
 * @param {(fields: Record<string, string>) => Record<string, unknown>} args
 * @param {() => Promise<void>} reload
 * @param {(error: string) => void} showError
 */
export function submitTo(command, args, reload, showError) {
  return async (/** @type {SubmitEvent} */ event) => {
    event.preventDefault();
    const form = /** @type {HTMLFormElement} */ (event.currentTarget);
    const fields = /** @type {Record<string, string>} */ (Object.fromEntries(new FormData(form)));
    const error = await write(command, args(fields), reload);
    showError(error);
    if (!error) form.reset();
  };
}
