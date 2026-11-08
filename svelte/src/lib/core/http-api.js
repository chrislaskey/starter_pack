// app_core over the web server's JSON API (../../../../axum/src/). An
// error answer, `{"error": "..."}`, is thrown with its message.

/**
 * @param {string} method
 * @param {string} path
 * @param {unknown} [body] sent as JSON
 */
async function send(method, path, body) {
  const response = await fetch(path, {
    method,
    headers: { Accept: "application/json", ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) throw new Error(await errorMessage(response));
  if (response.status === 204) return undefined;
  return response.json();
}

/** @param {Response} response */
async function errorMessage(response) {
  try {
    const { error } = await response.json();
    if (typeof error === "string") return error;
  } catch {
    // Not the API's JSON: fall back to the status
  }
  return `${response.status} ${response.statusText}`;
}

/** @param {string} id */
const postPath = (id) => `/api/posts/${encodeURIComponent(id)}`;

export function postsList() {
  return send("GET", "/api/posts");
}

/** @param {string} id */
export function postsGet(id) {
  return send("GET", postPath(id));
}

/** @param {string} body */
export function postsCreate(body) {
  return send("POST", "/api/posts", { body });
}

/**
 * @param {string} id
 * @param {string} body
 */
export function postsUpdate(id, body) {
  return send("PUT", postPath(id), { body });
}

/** @param {string} id */
export function postsDelete(id) {
  return send("DELETE", postPath(id));
}

// The web has nothing to sync: it is the server. The nav shows no sync
// status, and the conflicts page shows none.

export function syncRun() {
  return Promise.resolve(null);
}

export function syncStatus() {
  return Promise.resolve(null);
}

/** @param {boolean} _offline */
export function syncSetOffline(_offline) {
  return Promise.resolve(null);
}

/**
 * @param {(outcome: unknown) => void} _callback
 * @returns {() => void} stops listening
 */
export function onSynced(_callback) {
  return () => {};
}

export function conflictsList() {
  return Promise.resolve([]);
}

/** @param {string} _id */
export function conflictsReapply(_id) {
  return Promise.reject(new Error("nothing to reapply on the web"));
}

/** @param {string} _id */
export function conflictsLetGo(_id) {
  return Promise.reject(new Error("nothing to let go on the web"));
}
