const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

const hex = (rgb) => "#" + rgb.map((v) => v.toString(16).padStart(2, "0")).join("");
const rgb = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
const lines = (s) => s.split("\n").map((l) => l.trim()).filter(Boolean);

function fill(cfg) {
  $("busy_colour").value = hex(cfg.busy_colour);
  $("free_colour").value = hex(cfg.free_colour);
  $("paused_colour").value = hex(cfg.paused_colour);
  $("locked_colour").value = hex(cfg.locked_colour);
  $("lock_detection").checked = cfg.lock_detection;
  $("brightness").value = cfg.brightness;
  $("brightness_out").value = cfg.brightness + "%";
  $("use_camera").checked = cfg.use_camera;
  $("process_level_detection").checked = cfg.process_level_detection;
  $("poll_interval_s").value = cfg.poll_interval_ms / 1000;
  $("busy_hold_s").value = cfg.busy_hold_ms / 1000;
  $("fade_speed").value = cfg.fade_speed;
  $("force_busy_default_minutes").value = cfg.force_busy_default_minutes;
  $("test_duration_s").value = cfg.test_duration_s;
  $("ignore_apps").value = cfg.ignore_apps.join("\n");
  $("ignore_devices").value = cfg.ignore_devices.join("\n");
}

function read() {
  return {
    busy_colour: rgb($("busy_colour").value),
    free_colour: rgb($("free_colour").value),
    paused_colour: rgb($("paused_colour").value),
    locked_colour: rgb($("locked_colour").value),
    lock_detection: $("lock_detection").checked,
    brightness: Number($("brightness").value),
    use_camera: $("use_camera").checked,
    process_level_detection: $("process_level_detection").checked,
    poll_interval_ms: Math.round(Number($("poll_interval_s").value) * 1000),
    busy_hold_ms: Math.round(Number($("busy_hold_s").value) * 1000),
    fade_speed: Number($("fade_speed").value),
    force_busy_default_minutes: Number($("force_busy_default_minutes").value),
    test_duration_s: Number($("test_duration_s").value),
    ignore_apps: lines($("ignore_apps").value),
    ignore_devices: lines($("ignore_devices").value),
  };
}

function showStatus(st) {
  const dot = $("dot");
  dot.className = "dot " + st.state + (st.light_connected ? "" : " disconnected");
  const who = [...st.mic, ...st.cam];
  let text = { free: "Free", busy: "Busy", forced_busy: "Busy (forced)", locked: "Away (screen locked)", paused: "Paused" }[st.state] || st.state;
  if (st.state === "busy" && who.length) text += ": " + who.join(", ");
  if (!st.light_connected) text += " · no Luxafor connected";
  $("headline").textContent = text;
}

async function refreshControls() {
  const { paused, forced } = await invoke("controls");
  $("paused").checked = paused;
  const sel = $("forced");
  const left = sel.querySelector('option[value="left"]');
  if (forced > 0) {
    left.hidden = false;
    left.textContent = forced + " min left";
    sel.value = "left";
  } else {
    left.hidden = true;
    sel.value = String(forced);
  }
}

async function init() {
  fill(await invoke("get_config"));
  showStatus(await invoke("get_status"));
  await refreshControls();
  $("config_path").textContent = await invoke("config_path");
  $("log_path").textContent = await invoke("log_path");
  if (!navigator.userAgent.includes("Mac")) $("process_level_row").hidden = true;

  await listen("status", (e) => { showStatus(e.payload); refreshControls(); });
  await listen("config", (e) => fill(e.payload));

  $("brightness").addEventListener("input", (e) => ($("brightness_out").value = e.target.value + "%"));
  $("paused").addEventListener("change", (e) => invoke("set_paused", { paused: e.target.checked }));
  $("forced").addEventListener("change", (e) => {
    const v = e.target.value;
    if (v === "left") return;
    invoke("set_forced", { minutes: v === "-1" ? null : Number(v) });
  });

  $("save").addEventListener("click", async () => {
    try {
      fill(await invoke("save_config", { cfg: read() }));
      $("saved").textContent = "Saved";
      setTimeout(() => ($("saved").textContent = ""), 1500);
    } catch (err) {
      $("saved").textContent = "Error: " + err;
    }
  });

  document.querySelectorAll("button[data-test]").forEach((b) =>
    b.addEventListener("click", () => {
      const cfg = read();
      const c = cfg[b.dataset.test].map((v) => Math.round((v * cfg.brightness) / 100));
      invoke("test_light", { kind: "colour", colour: c });
    })
  );
  $("test_blink").addEventListener("click", () => invoke("test_light", { kind: "blink", colour: [255, 255, 0] }));
  $("test_off").addEventListener("click", () => invoke("test_light", { kind: "off", colour: null }));
  const reveal = async (cmd) => {
    const p = await invoke(cmd);
    try { await window.__TAURI__.opener.revealItemInDir(p); } catch (e) { console.warn(e); }
  };
  $("open_config").addEventListener("click", () => reveal("config_path"));
  $("open_log").addEventListener("click", () => reveal("log_path"));
}

init().catch((e) => ($("headline").textContent = "Error: " + e));
