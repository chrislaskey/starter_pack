// The main window's router: the page at each path, swapped in place as the
// address changes, so switching pages never shows a blank window. The paths
// are routes.js's, the one module that spells them out. A page is loaded the
// first time it is visited, so each page's code stays a chunk of its own.
// The quick search popup is a window of its own with its own HTML entry
// (src/quick-search.html), not a page here.
import { createRouter } from "sv-router";
import RemountPageOnNavigation from "./RemountPageOnNavigation.svelte";
import {
  COMMUNITIES_INDEX_PATH,
  COMMUNITY_SHOW_PATH,
  HOME_PATH,
  POSTS_INDEX_PATH,
  PROFILE_PATH,
  TAGS_INDEX_PATH,
  TMUX_EXAMPLE_PATH,
} from "./routes.js";

const { navigate, route } = createRouter({
  layout: RemountPageOnNavigation,
  [HOME_PATH]: () => import("../StatusPage.svelte"),
  [COMMUNITIES_INDEX_PATH]: () => import("../communities/CommunitiesPage.svelte"),
  [COMMUNITY_SHOW_PATH]: () => import("../communities/CommunityPage.svelte"),
  [POSTS_INDEX_PATH]: () => import("../posts/PostsPage.svelte"),
  [TAGS_INDEX_PATH]: () => import("../tags/TagsPage.svelte"),
  [PROFILE_PATH]: () => import("../profile/ProfilePage.svelte"),
  [TMUX_EXAMPLE_PATH]: () => import("../tmux-example/TmuxExamplePage.svelte"),
});

/**
 * The current page's path parameters, such as `id` in `/communities/:id`.
 * @returns {Record<string, string>}
 */
export function pathParams() {
  return /** @type {Record<string, string>} */ (route.params);
}

// The router types its paths; ours come from routes.js, so any path goes.
const navigateToPath = /** @type {(pathname: string, options: { search: string }) => Promise<unknown>} */ (
  /** @type {unknown} */ (navigate)
);

/**
 * Shows the page at a URL from routes.js in place of the current one, and
 * adds it to the history as a link would.
 * @param {string} url
 */
export function navigateToUrl(url) {
  const { pathname, search } = new URL(url, window.location.origin);
  return navigateToPath(pathname, { search });
}
