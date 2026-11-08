// The router: the page at each path, swapped in place as the address
// changes, so switching pages never shows a blank window. The paths are
// routes.js's, the one module that spells them out. A page is loaded the
// first time it is visited, so each page's code stays a chunk of its own.
import { createRouter } from "sv-router";
import RemountPageOnNavigation from "./RemountPageOnNavigation.svelte";
import {
  COMMUNITIES_INDEX_PATH,
  CONFLICTS_INDEX_PATH,
  HOME_PATH,
  POSTS_EDIT_PATH,
  POSTS_INDEX_PATH,
  POSTS_NEW_PATH,
  POSTS_SHOW_PATH,
} from "./routes.js";

const { navigate, route } = createRouter({
  layout: RemountPageOnNavigation,
  [HOME_PATH]: () => import("../HomePage.svelte"),
  [POSTS_INDEX_PATH]: () => import("../posts/PostsIndexPage.svelte"),
  [POSTS_NEW_PATH]: () => import("../posts/PostsNewPage.svelte"),
  [POSTS_SHOW_PATH]: () => import("../posts/PostsShowPage.svelte"),
  [POSTS_EDIT_PATH]: () => import("../posts/PostsEditPage.svelte"),
  [COMMUNITIES_INDEX_PATH]: () => import("../communities/CommunitiesIndexPage.svelte"),
  [CONFLICTS_INDEX_PATH]: () => import("../conflicts/ConflictsIndexPage.svelte"),
});

/**
 * The current page's path parameters, such as `id` in `/posts/:id`.
 * @returns {Record<string, string>}
 */
export function pathParams() {
  return /** @type {Record<string, string>} */ (route.params);
}

// The router types its paths; ours come from routes.js, so any path goes.
const navigateToPath = /** @type {(pathname: string) => Promise<unknown>} */ (/** @type {unknown} */ (navigate));

/**
 * Shows the page at a URL from routes.js in place of the current one, and
 * adds it to the history as a link would.
 * @param {string} url
 */
export function navigateToUrl(url) {
  return navigateToPath(url);
}
