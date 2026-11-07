// app_core over the web server's JSON API (../../../../axum/src/). An
// error answer, `{"error": "..."}`, is thrown with its message.

/** @param {string} path */
async function get(path) {
  const response = await fetch(path, { headers: { Accept: "application/json" } });
  if (!response.ok) throw new Error(await errorMessage(response));
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

export function listPosts() {
  return get("/api/posts");
}

/** @param {string} id */
export function getPost(id) {
  return get(`/api/posts/${encodeURIComponent(id)}`);
}
