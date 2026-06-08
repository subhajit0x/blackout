// BLACKOUT desktop — frontend logic. Talks to the Rust backend via Tauri.
const { invoke } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;
const event = window.__TAURI__.event;

const base = (p) => (p || "").split("/").pop();
const el = (id) => document.getElementById(id);

// ---------- native-app feel ----------
// No browser right-click menu, no image/text drag, no accidental pinch-zoom.
document.addEventListener("contextmenu", (e) => e.preventDefault());
document.addEventListener("dragstart", (e) => e.preventDefault());
document.addEventListener("gesturestart", (e) => e.preventDefault());

// ---------- view switching ----------
const VIEWS = ["clean", "opsec", "lockdown", "panic"];
function showView(name) {
  document.querySelectorAll(".nav-item").forEach((b) => b.classList.toggle("active", b.dataset.view === name));
  document.querySelectorAll(".view").forEach((v) => v.classList.remove("active"));
  el("view-" + name).classList.add("active");
}
document.querySelectorAll(".nav-item").forEach((btn) =>
  btn.addEventListener("click", () => showView(btn.dataset.view)));

// Desktop keyboard shortcuts: Cmd/Ctrl + 1..4 switch tabs.
document.addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key >= "1" && e.key <= "4") {
    e.preventDefault();
    showView(VIEWS[parseInt(e.key, 10) - 1]);
  }
});

// ================= PLATFORM =================
// Every screen shows only what THIS OS can actually do. We learn the platform
// from the backend's capabilities() once, then tailor copy + which controls show.
let CAP = { platform: "", wifi: false, bluetooth: false, firewall: false, settings_deeplink: false };
let PLAT = "macos"; // normalized: macos | windows | linux | android | ios
const PLAT_NAME = { macos: "macOS", windows: "Windows", linux: "Linux", android: "Android", ios: "iOS" };
const platName = () => PLAT_NAME[PLAT] || CAP.platform || "this device";

async function detectPlatform() {
  try { CAP = (await invoke("capabilities")) || CAP; } catch (_) {}
  const p = (CAP.platform || "").toLowerCase();
  PLAT = p.includes("mac") ? "macos"
    : p.includes("win") ? "windows"
    : p.includes("ios") ? "ios"
    : p.includes("droid") ? "android"
    : p.includes("linux") ? "linux"
    : (p || "macos");
  document.body.dataset.platform = PLAT;
  applyPlatformCopy();
  renderLockdownGeneric();
}

function applyPlatformCopy() {
  const lockSub = {
    macos: "Reduce device exposure. Actions macOS blocks are clearly marked.",
    android: "One-tap exposure controls for Android — opens the right system panel and clears your clipboard.",
    ios: "iOS limits what apps may change. Your OPSEC guide has the exact steps for iPhone/iPad.",
    windows: "Live hardening for Windows is on the way — your OPSEC guide has the steps to do now.",
    linux: "Live hardening for Linux is on the way — your OPSEC guide has the steps to do now.",
  };
  const panicSub = {
    macos: "One tap, complete isolation: wipe clipboard, kill Wi-Fi/Bluetooth, turn AirDrop off, open Lockdown Mode, lock the screen.",
    android: "One tap: clear the clipboard and jump to Airplane mode so you can cut every radio fast.",
    ios: "iOS won't let an app cut radios or lock the device. Use the steps in your OPSEC guide.",
    windows: "Live panic actions for Windows are coming — your OPSEC guide has the steps for now.",
    linux: "Live panic actions for Linux are coming — your OPSEC guide has the steps for now.",
  };
  const setTxt = (id, txt) => { const e = el(id); if (e && txt) e.textContent = txt; };
  setTxt("lockdownSub", lockSub[PLAT] || lockSub.macos);
  setTxt("panicSub", panicSub[PLAT] || panicSub.macos);
  if (PLAT !== "macos" && PLAT !== "windows" && PLAT !== "linux") {
    setTxt("dzTitle", "Choose files to clean");
  }
}

// Android (and any non-macOS) Lockdown surface: real deep-links + one-tap.
const GENERIC_PANES = {
  android: [
    ["wifi", "Wi-Fi"], ["bluetooth", "Bluetooth"], ["airplane", "Airplane mode"],
    ["location", "Location"], ["permissions", "App permissions"],
  ],
};
function renderLockdownGeneric() {
  const host = el("lockdownGeneric");
  if (!host || PLAT === "macos") { if (host) host.innerHTML = ""; return; }
  const panes = GENERIC_PANES[PLAT];
  if (panes) {
    const links = panes
      .map(([pane, label]) => `<button class="btn btn-ghost gen-pane" data-pane="${pane}">${label} ↗</button>`)
      .join("");
    host.innerHTML = `<div class="hardening">
      <h2 class="sub">${platName()} controls</h2>
      <p class="sub-note">Jump straight to the system panel, or lock down in one tap.</p>
      <div class="deeplinks">${links}</div>
      <button class="btn btn-primary harden gen-lockdown">🛡 Lock down now — clear clipboard + open Airplane mode</button>
    </div>`;
    host.querySelectorAll(".gen-pane").forEach((b) =>
      b.addEventListener("click", () => invoke("open_settings", { pane: b.dataset.pane })));
    host.querySelector(".gen-lockdown").addEventListener("click", async (e) => {
      const results = await invoke("apply_level", { level: 4 });
      renderActions("lockdownResults", results, "Lockdown applied");
    });
  } else {
    // iOS / Windows / Linux: no live control — point to the device guide.
    host.innerHTML = `<div class="hardening">
      <p class="sub-note">Live one-tap control isn't available on ${platName()} yet. Your OPSEC guide has the exact, accurate steps for this device.</p>
      <button class="btn btn-primary gen-guide">Open the ${platName()} guide →</button>
    </div>`;
    host.querySelector(".gen-guide").addEventListener("click", () => showView("opsec"));
  }
}

// ================= CLEAN =================
let selected = [];

const CLEAN_EXTS = ["jpg","jpeg","png","webp","tif","tiff","heic","heif",
  "mp3","wav","m4a","pdf","docx","xlsx","pptx","txt","mp4","mov","avi","mkv"];

function setFiles(paths) {
  const seen = new Set(selected);
  for (const p of paths) if (!seen.has(p)) { selected.push(p); seen.add(p); }
  el("fileCount").textContent = `${selected.length} file${selected.length === 1 ? "" : "s"} selected`;
  el("fileBar").classList.toggle("hidden", selected.length === 0);
}

el("browseBtn").addEventListener("click", async () => {
  const picked = await dialog.open({
    multiple: true,
    filters: [{ name: "Cleanable files", extensions: CLEAN_EXTS }],
  });
  if (picked) setFiles(Array.isArray(picked) ? picked : [picked]);
});

el("clearBtn").addEventListener("click", () => {
  selected = [];
  el("fileBar").classList.add("hidden");
  el("cleanResults").innerHTML = "";
});

el("inspectBtn").addEventListener("click", async () => {
  if (!selected.length) return;
  busy("inspectBtn", true);
  const reports = await invoke("inspect_files", { paths: selected });
  renderInspect(reports);
  busy("inspectBtn", false);
});

el("cleanBtn").addEventListener("click", async () => {
  if (!selected.length) return;
  busy("cleanBtn", true);
  const res = await invoke("clean_files", { paths: selected });
  renderClean(res);
  busy("cleanBtn", false);
});

function busy(id, on) {
  const b = el(id);
  b.disabled = on;
  if (on) { b.dataset.t = b.textContent; b.textContent = "Working…"; }
  else if (b.dataset.t) b.textContent = b.dataset.t;
}

const ICO = (p) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${p}</svg>`;
const F_ICON = {
  location: ICO('<path d="M20 10c0 6-8 12-8 12s-8-6-8-12a8 8 0 0 1 16 0z"/><circle cx="12" cy="10" r="2.6"/>'),
  device: ICO('<path d="M14.5 5h-5L7.5 7.5H4.5a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h15a2 2 0 0 0 2-2v-8a2 2 0 0 0-2-2h-3z"/><circle cx="12" cy="13" r="3"/>'),
  date: ICO('<circle cx="12" cy="12" r="9"/><path d="M12 7.5V12l3 2"/>'),
  identity: ICO('<circle cx="12" cy="8" r="3.6"/><path d="M5 20a7 7 0 0 1 14 0"/>'),
  software: ICO('<rect x="7" y="7" width="10" height="10" rx="2"/><path d="M10 3v2M14 3v2M10 19v2M14 19v2M3 10h2M3 14h2M19 10h2M19 14h2"/>'),
  place: ICO('<path d="M9 20H5a2 2 0 0 1-2-2V8l9-5 9 5v10a2 2 0 0 1-2 2h-4"/><path d="M9 20v-6h6v6"/>'),
};

function reportCard(r, statusLabel, pillClass, items) {
  const findings = (r.findings || []).map((f) => {
    const icon = F_ICON[f.kind] || ICO('<circle cx="12" cy="12" r="2"/>');
    const map = f.kind === "location"
      ? ` <a class="reveal maplink" data-path="https://www.google.com/maps?q=${encodeURIComponent(f.value)}">show on map ↗</a>`
      : "";
    return `<div class="finding">
      <span class="f-ico ${f.kind}">${icon}</span>
      <span class="f-label">${esc(f.label)}</span>
      <span class="f-val">${esc(f.value)}</span>${map}</div>`;
  }).join("");
  const removed = (items || []).map((x) => `<div class="removed-item">${esc(x)}</div>`).join("");
  const notes = (r.notes || []).map((n) => `<div class="note-item">${esc(n)}</div>`).join("");
  return `<div class="card">
    <div class="card-head">
      <span class="card-name">${esc(base(r.source))}</span>
      <span class="card-cat">${esc(r.category)}</span>
      <span class="pill ${pillClass}">${statusLabel}</span>
    </div>${findings}${removed}${notes}</div>`;
}

function renderClean(res) {
  const out = el("cleanResults");
  const parts = [];
  parts.push(`<div class="banner ok">
    ✓ ${res.cleaned} cleaned · ${res.copied} copied · ${res.skipped} skipped · ${res.errored} errored
    <a class="reveal" data-path="${esc(res.out_dir)}">Reveal output folder ↗</a>
  </div>`);
  if (PLAT === "macos" && !res.ffmpeg && res.skipped > 0) {
    parts.push(`<div class="banner info">ℹ Install ffmpeg (brew install ffmpeg) to clean video / HEIC / M4A.</div>`);
  }
  for (const r of res.reports) {
    const pill = r.status === "cleaned" ? "cleaned" : r.status === "copied" ? "copied"
      : r.status === "unsupported" ? "unsupported" : "error";
    parts.push(reportCard(r, r.status, pill, r.removed));
  }
  out.innerHTML = parts.join("");
  wireReveal(out);
}

function renderInspect(reports) {
  const out = el("cleanResults");
  const parts = [`<div class="banner info">🔍 Inspection only — nothing was written. Press Clean to remove.</div>`];
  for (const r of reports) {
    const exposed = (r.removed || []).length > 0;
    parts.push(reportCard(r, exposed ? "exposed" : "clean", exposed ? "exposed" : "clean", r.removed));
  }
  out.innerHTML = parts.join("");
  wireReveal(out);
}

function wireReveal(scope) {
  scope.querySelectorAll(".reveal").forEach((a) =>
    a.addEventListener("click", () => invoke("reveal_path", { path: a.dataset.path })));
}

// drag & drop (native Tauri events)
const dz = el("dropzone");
event.listen("tauri://drag-enter", () => dz.classList.add("drag"));
event.listen("tauri://drag-leave", () => dz.classList.remove("drag"));
event.listen("tauri://drag-drop", (e) => {
  dz.classList.remove("drag");
  const paths = (e.payload?.paths || []).filter((p) =>
    CLEAN_EXTS.includes((p.split(".").pop() || "").toLowerCase()));
  if (paths.length) setFiles(paths);
});

// ================= WATCH FOLDER =================
let watching = false;

el("watchBtn").addEventListener("click", async () => {
  const dir = await dialog.open({ directory: true, multiple: false });
  if (!dir) return;
  try {
    const out = await invoke("start_watch", { path: dir });
    watching = true;
    el("watchStatus").textContent = `Watching ${dir.split("/").pop()} → cleaned copies go to BLACKOUT-clean`;
    el("watchBtn").classList.add("hidden");
    el("watchStop").classList.remove("hidden");
    el("watchLog").innerHTML = `<div class="watch-line dim">Watching… drop or save a file into that folder.</div>`;
  } catch (e) {
    el("watchStatus").textContent = "Could not watch: " + e;
  }
});

el("watchStop").addEventListener("click", async () => {
  await invoke("stop_watch");
  watching = false;
  el("watchStatus").textContent = "Off — pick a folder to auto-strip every new file.";
  el("watchStop").classList.add("hidden");
  el("watchBtn").classList.remove("hidden");
});

event.listen("watch-cleaned", (e) => {
  const p = e.payload || {};
  const tag = p.status === "cleaned" ? "✓ cleaned" : p.status === "copied" ? "· no metadata" : p.status;
  const detail = (p.removed || []).length ? ` — ${p.removed[0]}` : "";
  const line = `<div class="watch-line"><b>${esc(p.name)}</b> <span class="dim">${tag}${esc(detail)}</span></div>`;
  el("watchLog").insertAdjacentHTML("afterbegin", line);
});

// ================= OPSEC =================
const SEV = { bad: 0, warn: 1, unknown: 2, good: 3 };
const CATS = ["Device", "Network", "Sharing", "Privacy", "Other"];

el("scanBtn").addEventListener("click", async () => {
  busy("scanBtn", true);
  renderOpsec(await invoke("opsec_score"));
  busy("scanBtn", false);
});

const DEV_ICO = ICO('<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 20h8M12 17v3"/>');

function renderOpsec(rep) {
  const color = rep.score >= 80 ? "var(--good)" : rep.score >= 50 ? "var(--warn)" : "var(--bad)";
  el("scoreNum").textContent = rep.score;
  document.querySelector(".gauge").style.background =
    `conic-gradient(${color} ${rep.score * 3.6}deg, var(--panel-2) 0deg)`;

  let html = "";

  // Device line — so the guidance is clearly "for your device".
  if (rep.device) {
    const parts = [rep.device.platform, rep.device.os_version, rep.device.model].filter(Boolean).join(" · ");
    if (parts) html += `<div class="device-row">${DEV_ICO}<span>${esc(parts)}</span></div>`;
  }

  // Tailored guide — prioritized hardening steps.
  if (rep.guide && rep.guide.length) {
    html += `<div class="section-title">Recommended for your device</div>`;
    for (const g of rep.guide) {
      const fix = g.fix ? `<button class="btn btn-ghost fix-btn" data-fix="${esc(g.fix)}">Fix</button>` : "";
      html += `<div class="guide-step sev-${esc(g.severity)}">
        <div class="gs-head"><span class="gs-sev"></span>
          <span class="gs-title">${esc(g.title)}</span>${fix}</div>
        <div class="gs-why">${esc(g.why)}</div>
        <div class="gs-how">${esc(g.how)}</div>
      </div>`;
    }
  }

  // Detailed grouped checks — only when there are real probes (desktop), not the mobile stub.
  if (rep.checks.length > 1) {
    html += `<div class="section-title">Detailed checks</div>`;
    const groups = {};
    for (const c of rep.checks) (groups[c.category] || (groups[c.category] = [])).push(c);
    for (const cat of CATS) {
      const list = groups[cat];
      if (!list) continue;
      list.sort((a, b) => (SEV[a.status] ?? 9) - (SEV[b.status] ?? 9));
      html += `<div class="check-cat">${esc(cat)}</div>`;
      for (const c of list) {
        const fix = c.fix ? `<button class="btn btn-ghost fix-btn" data-fix="${esc(c.fix)}">Fix</button>` : "";
        html += `<div class="check">
          <span class="check-dot ${c.status}"></span>
          <div class="check-body">
            <div class="check-label">${esc(c.label)}</div>
            <div class="check-detail">${esc(c.detail)}</div>
          </div>${fix}
        </div>`;
      }
    }
  }

  el("opsecChecks").innerHTML = html;

  el("opsecChecks").querySelectorAll(".fix-btn").forEach((b) =>
    b.addEventListener("click", async () => {
      b.disabled = true;
      b.textContent = "Fixing…";
      await invoke("apply_fix", { id: b.dataset.fix });
      renderOpsec(await invoke("opsec_score")); // re-scan to reflect the change
    }));
}

// ================= LOCKDOWN =================
document.querySelectorAll(".level").forEach((btn) => {
  btn.addEventListener("click", async () => {
    const lvl = parseInt(btn.dataset.level, 10);
    const results = await invoke("apply_level", { level: lvl });
    renderActions("lockdownResults", results, `Level ${lvl} applied`);
  });
});

// deep-links to System Settings panes
document.querySelectorAll("[data-settings]").forEach((b) =>
  b.addEventListener("click", () => invoke("open_settings", { pane: b.dataset.settings })));

// admin-authenticated hardening
el("hardenBtn").addEventListener("click", async () => {
  busy("hardenBtn", true);
  const results = await invoke("harden_now");
  renderActions("lockdownResults", results, "Hardening applied");
  busy("hardenBtn", false);
});

// ================= PANIC =================
el("panicBtn").addEventListener("click", async () => {
  const results = await invoke("panic_now");
  renderActions("panicResults", results, "Panic executed");
});

function renderActions(targetId, results, title) {
  const icon = (s) => (s === "done" ? "✓" : s === "unavailable" ? "⊘" : "✕");
  const parts = [`<div class="banner ok">⚡ ${title}</div>`];
  for (const a of results) {
    const labelHtml = a.status === "unavailable"
      ? `<span class="strike">${esc(a.label)}</span> <span style="color:var(--muted)">— not available on ${esc(platName())}</span>`
      : esc(a.label);
    parts.push(`<div class="action ${a.status}">
      <span class="action-ico">${icon(a.status)}</span>
      <div><div class="action-label">${labelHtml}</div>
      <div class="action-detail">${esc(a.detail)}</div></div>
    </div>`);
  }
  el(targetId).innerHTML = parts.join("");
}

// ---------- util ----------
function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

// Learn the platform and tailor the UI. Runs once at startup.
detectPlatform();
