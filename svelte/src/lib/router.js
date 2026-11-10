// The router: the page at each path, swapped in place as the address
// changes, so switching pages never shows a blank window. The paths are
// routes.js's, the one module that spells them out. A page is loaded the
// first time it is visited, so each page's code stays a chunk of its own.
// Before a page that needs someone signed in, the router asks who is,
// and sends a visitor to the sign-in page instead when nobody is, or in
// the app to the accounts on the device, where sign-in is reached from;
// the app never shows the sign-in pages themselves.
import { createRouter } from "sv-router";
import RemountPageOnNavigation from "./RemountPageOnNavigation.svelte";
import { hasDeviceAccounts } from "./core/index.js";
import { currentUser } from "./current_user.js";
import {
  ACCOUNTS_INDEX_PATH,
  ACCOUNTS_SIGN_IN_PATH,
  ACCOUNTS_SIGN_UP_PATH,
  COMMUNITIES_EDIT_PATH,
  COMMUNITIES_INDEX_PATH,
  COMMUNITIES_NEW_PATH,
  COMMUNITIES_SHOW_PATH,
  CONFLICTS_INDEX_PATH,
  FILES_EDIT_PATH,
  FILES_INDEX_PATH,
  FILES_NEW_PATH,
  FILES_SHOW_PATH,
  HOME_PATH,
  OPEN_PATHS,
  POSTS_EDIT_PATH,
  POSTS_INDEX_PATH,
  POSTS_NEW_PATH,
  POSTS_SHOW_PATH,
  PROFILE_EDIT_PATH,
  accountsIndexUrl,
  accountsSignInUrl,
} from "./routes.js";

const { navigate, route } = createRouter({
  layout: RemountPageOnNavigation,
  hooks: {
    async beforeLoad({ pathname }) {
      // In the app a sign-in happens in the browser, from the accounts page
      if (hasDeviceAccounts && (pathname === ACCOUNTS_SIGN_IN_PATH || pathname === ACCOUNTS_SIGN_UP_PATH)) {
        throw navigateToUrl(accountsIndexUrl());
      }
      if (OPEN_PATHS.includes(pathname)) return;
      try {
        await currentUser();
      } catch {
        throw navigateToUrl(hasDeviceAccounts ? accountsIndexUrl() : accountsSignInUrl());
      }
    },
  },
  [HOME_PATH]: () => import("../HomePage.svelte"),
  [ACCOUNTS_INDEX_PATH]: () => import("../accounts/AccountsIndexPage.svelte"),
  [ACCOUNTS_SIGN_UP_PATH]: () => import("../accounts/AccountsSignUpPage.svelte"),
  [ACCOUNTS_SIGN_IN_PATH]: () => import("../accounts/AccountsSignInPage.svelte"),
  [POSTS_INDEX_PATH]: () => import("../posts/PostsIndexPage.svelte"),
  [POSTS_NEW_PATH]: () => import("../posts/PostsNewPage.svelte"),
  [POSTS_SHOW_PATH]: () => import("../posts/PostsShowPage.svelte"),
  [POSTS_EDIT_PATH]: () => import("../posts/PostsEditPage.svelte"),
  [FILES_INDEX_PATH]: () => import("../files/FilesIndexPage.svelte"),
  [FILES_NEW_PATH]: () => import("../files/FilesNewPage.svelte"),
  [FILES_SHOW_PATH]: () => import("../files/FilesShowPage.svelte"),
  [FILES_EDIT_PATH]: () => import("../files/FilesEditPage.svelte"),
  [COMMUNITIES_INDEX_PATH]: () => import("../communities/CommunitiesIndexPage.svelte"),
  [COMMUNITIES_NEW_PATH]: () => import("../communities/CommunitiesNewPage.svelte"),
  [COMMUNITIES_SHOW_PATH]: () => import("../communities/CommunitiesShowPage.svelte"),
  [COMMUNITIES_EDIT_PATH]: () => import("../communities/CommunitiesEditPage.svelte"),
  [CONFLICTS_INDEX_PATH]: () => import("../conflicts/ConflictsIndexPage.svelte"),
  [PROFILE_EDIT_PATH]: () => import("../profile/ProfileEditPage.svelte"),
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
