// The community the person works in, as the pages and the nav show it:
// asked of the application on each page, `currentCommunityGet()`, which
// answers null when they are in none. Joining or leaving says so here,
// `currentCommunityChanged()`, so the nav asks again without a reload.
import { currentCommunityGet } from "./core/index.js";

const changes = new EventTarget();

export function currentCommunityLoad() {
  return currentCommunityGet();
}

export function currentCommunityChanged() {
  changes.dispatchEvent(new Event("changed"));
}

/**
 * @param {() => void} callback
 * @returns {() => void} stops listening
 */
export function onCurrentCommunityChanged(callback) {
  changes.addEventListener("changed", callback);
  return () => changes.removeEventListener("changed", callback);
}
