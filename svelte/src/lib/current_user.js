// The user signed in, as the pages and the nav show them: asked of the
// application once, `usersMe()`, and kept until a sign-out or a sign-in
// says otherwise. Throws when nobody is signed in, which is what sends a
// visitor to the sign-in page (router.js).
import { usersMe } from "./core/index.js";

/** @type {Promise<{id: string, email: string}> | null} */
let current = null;

export function currentUser() {
  current ??= usersMe().catch((problem) => {
    current = null;
    throw problem;
  });
  return current;
}

export function forgetCurrentUser() {
  current = null;
}
