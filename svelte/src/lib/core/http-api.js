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
