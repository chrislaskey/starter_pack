// The user signed in, as the pages and the nav show them: asked of the
// application once, `currentUserGet()`, and kept until a sign-out or a
// sign-in says otherwise. Null when nobody is signed in, which is what
// sends a visitor to the sign-in page (router.js); an error only when the
// application could not tell, which the page shows.
import { currentUserGet } from "./core/index.js";

/** @typedef {{id: string, email: string, avatar_url?: string | null}} User */

/** @type {Promise<User | null> | null} */
let current = null;

/** @returns {Promise<User | null>} */
export function currentUserLoad() {
  current ??= currentUserGet().then(
    (user) => {
      if (!user) current = null;
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
}
