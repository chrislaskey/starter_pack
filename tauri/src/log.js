// The log list both pages show at the bottom: newest first, the last 30.

export function log(message, data) {
  const item = document.createElement("li");
  const time = new Date().toLocaleTimeString();
  item.textContent = `${time}  ${message}`;
  if (data !== undefined) {
    const pre = document.createElement("pre");
    pre.textContent = JSON.stringify(data, null, 2);
    item.appendChild(pre);
  }
  const list = document.getElementById("log");
  list.prepend(item);
  while (list.children.length > 30) list.lastChild.remove();
}
