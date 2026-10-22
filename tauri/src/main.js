// The status window. It mirrors the connection state the Rust side emits
// and sends the user's actions back as commands.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const LABELS = {
  disconnected: "Disconnected",
  connecting: "Connecting…",
  connected: "Connected, joining the api channel…",
  joined: "Connected",
};

const el = (id) => document.getElementById(id);

function log(message, data) {
  const item = document.createElement("li");
  const time = new Date().toLocaleTimeString();
  item.textContent = `${time}  ${message}`;
  if (data !== undefined) {
    const pre = document.createElement("pre");
    pre.textContent = JSON.stringify(data, null, 2);
    item.appendChild(pre);
  }
  const list = el("log");
  list.prepend(item);
  while (list.children.length > 30) list.lastChild.remove();
}

let lastState;

function render(status) {
  el("server-url").textContent = status.server_url;
  el("status").dataset.state = status.state;
  el("status-label").textContent = LABELS[status.state] ?? status.state;
  el("status-detail").textContent = status.detail ?? "";

  el("log-in").hidden = status.has_token;
  el("log-out").hidden = !status.has_token;
  el("ping").hidden = status.state !== "joined";

  if (status.state !== lastState) {
    log(`${LABELS[status.state] ?? status.state}${status.detail ? ` — ${status.detail}` : ""}`);
    lastState = status.state;
  }
}

async function ping() {
  const started = performance.now();
  try {
    const reply = await invoke("api_request", { event: "auth.token.get", payload: {} });
    log(`auth.token.get replied in ${Math.round(performance.now() - started)}ms`, reply);
  } catch (error) {
    log("auth.token.get failed", error);
  }
}

async function run(action, command, args) {
  try {
    await invoke(command, args);
  } catch (error) {
    log(`${action} failed: ${error}`);
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  el("log-in").addEventListener("click", () =>
    run("Log in", "open_phoenix", { path: "/client/new" }),
  );
  el("open-phoenix").addEventListener("click", () => run("Open Phoenix", "open_phoenix", { path: "/" }));
  el("log-out").addEventListener("click", () => run("Log out", "log_out"));
  el("ping").addEventListener("click", ping);

  await listen("connection-status", (event) => render(event.payload));
  await listen("api-message", (event) => log(`Server pushed ${event.payload.event}`, event.payload.payload));

  render(await invoke("get_connection_status"));
});
