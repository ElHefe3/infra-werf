// The view message API v1 lives next to this page, at ../infrawerf/v1/.
// Every call is checked against what the owner allowed this plugin.
const api = path => new URL(`../infrawerf/v1/${path}`, location.href);
const show = (id, text) => { document.getElementById(id).textContent = text; };

async function call(path, options) {
  const response = await fetch(api(path), options);
  const body = await response.json();
  if (!response.ok) throw new Error(body.message || body.error);
  return body;
}

async function load() {
  const context = await call("context");
  show("server", context.server ? `Host dashboard · ${context.server.name}` : "Host dashboard");
  try {
    const readings = await call("observations");
    const list = document.getElementById("readings");
    list.replaceChildren(...readings.map(reading => {
      const item = document.createElement("li");
      const result = reading.observation?.result;
      item.textContent = `${reading.title}: ${result?.value ?? (result ? result.outcome : "not read yet")}`;
      return item;
    }));
  } catch (error) { show("readings", `Not available: ${error.message}`); }
  try {
    const { inventory } = await call("inventory");
    show("inventory", inventory ? `${inventory.hostname}: ${inventory.cpus} CPUs, collected ${inventory.collected_at}` : "No saved inventory yet.");
  } catch (error) { show("inventory", `Not available: ${error.message}`); }
}

document.getElementById("read-kernel").addEventListener("click", async () => {
  try {
    const asked = await call("actions", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ contribution_id: "kernel" }) });
    show("action", "Waiting for you in InfraWerf…");
    for (;;) {
      await new Promise(resolve => setTimeout(resolve, 700));
      const state = await call(`actions/${asked.id}`);
      if (state.state === "waiting" || state.state === "running") continue;
      show("action", state.state === "done" ? `Kernel ${state.observation?.result?.value ?? "unknown"}` : `Not run: ${state.state}`);
      await load();
      break;
    }
  } catch (error) { show("action", `Not run: ${error.message}`); }
});

load().catch(error => show("readings", `Not available: ${error.message}`));
