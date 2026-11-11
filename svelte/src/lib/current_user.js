// The user signed in, as the pages and the nav show them: asked of the
// application once, `currentUserGet()`, and kept until a sign-out or a
// sign-in says otherwise. Null when nobody is signed in, which is what
// sends a visitor to the sign-in page (router.js); an error only when the
// application could not tell, which the page shows.
//
// Whether someone is signed in is also kept in localStorage, under
// `app:signed-in`, for the public website's header (src/public/partials),
// which has no way to ask: it shows "Open the app" in place of Sign in
// and Sign up. Only a hint, so a stale one costs a click.
import { currentUserGet } from "./core/index.js";

/** @typedef {{id: string, email: string, avatar_url?: string | null}} User */

/** @type {Promise<User | null> | null} */
let current = null;

const SIGNED_IN_KEY = "app:signed-in";

/** @param {boolean} signedIn */
function rememberSignedIn(signedIn) {
  if (signedIn) localStorage.setItem(SIGNED_IN_KEY, "true");
  else localStorage.removeItem(SIGNED_IN_KEY);
}

/** @returns {Promise<User | null>} */
export function currentUserLoad() {
  current ??= currentUserGet().then(
    (user) => {
      if (!user) current = null;
      rememberSignedIn(Boolean(user));
      return user;
    },
    (problem) => {
      current = null;
      throw problem;
    },
  );
  return current;
}

export function currentUserForget() {
  current = null;
  rememberSignedIn(false);
}
