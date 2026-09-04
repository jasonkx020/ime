const $ = (sel) => document.querySelector(sel);

function token() {
  const t = $("#token").value.trim() || localStorage.getItem("yc_admin_token") || "dev-token";
  $("#token").value = t;
  localStorage.setItem("yc_admin_token", t);
  return t;
}

async function api(path, opts = {}) {
  const headers = Object.assign({ "X-Admin-Token": token() }, opts.headers || {});
  const res = await fetch(path, { ...opts, headers });
  const text = await res.text();
  let data;
  try { data = text ? JSON.parse(text) : {}; } catch { data = { raw: text }; }
  if (!res.ok) throw new Error(data.error || res.statusText);
  return data;
}

async function loadDashboard() {
  const st = await api("/api/v1/dashboard");
  $("#dashboard").innerHTML = [
    ["已发布包", st.published_packs],
    ["草稿包", st.draft_packs],
    ["画像数", st.profiles],
    ["7日活跃设备", st.active_devices_7d],
    ["7日选词事件", st.select_events_7d],
  ].map(([l, n]) => `<div class="card"><div class="n">${n ?? 0}</div><div class="l">${l}</div></div>`).join("");
}

async function loadPacks() {
  const data = await api("/api/v1/langpacks");
  const tb = $("#packs-table tbody");
  tb.innerHTML = (data.items || []).map((p) => `
    <tr>
      <td>${p.id}</td>
      <td>${p.pack_id}<div style="color:#8b9aab;font-size:.8rem">${p.display_name || ""}</div></td>
      <td>${p.lang}</td>
      <td>${p.version}</td>
      <td><span class="status ${p.status}">${p.status}</span></td>
      <td>${p.size_bytes || 0}</td>
      <td class="ops">
        <label class="secondary" style="background:#314255;color:#e8eef4;border-radius:6px;padding:.3rem .55rem;font-size:.8rem;cursor:pointer">
          上传<input type="file" accept=".imepack" hidden data-upload="${p.id}" />
        </label>
        <button type="button" data-publish="${p.id}">发布</button>
        <button type="button" class="secondary" data-archive="${p.id}">归档</button>
      </td>
    </tr>`).join("");
}

$("#create-pack").addEventListener("submit", async (e) => {
  e.preventDefault();
  const fd = new FormData(e.target);
  await api("/api/v1/langpacks", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      pack_id: fd.get("pack_id"),
      lang: fd.get("lang"),
      display_name: fd.get("display_name") || fd.get("pack_id"),
      version: Number(fd.get("version") || 1),
    }),
  });
  e.target.reset();
  await Promise.all([loadPacks(), loadDashboard()]);
});

$("#packs-table").addEventListener("change", async (e) => {
  const input = e.target.closest("input[data-upload]");
  if (!input || !input.files?.[0]) return;
  const id = input.getAttribute("data-upload");
  const body = new FormData();
  body.append("file", input.files[0]);
  await api(`/api/v1/langpacks/${id}/upload`, { method: "POST", body });
  await Promise.all([loadPacks(), loadDashboard()]);
});

$("#packs-table").addEventListener("click", async (e) => {
  const pub = e.target.closest("[data-publish]");
  const arch = e.target.closest("[data-archive]");
  if (pub) {
    await api(`/api/v1/langpacks/${pub.getAttribute("data-publish")}/publish`, { method: "POST" });
    await Promise.all([loadPacks(), loadDashboard()]);
  }
  if (arch) {
    await api(`/api/v1/langpacks/${arch.getAttribute("data-archive")}/archive`, { method: "POST" });
    await Promise.all([loadPacks(), loadDashboard()]);
  }
});

$("#btn-refresh-packs").addEventListener("click", () => loadPacks().catch(alert));

$("#lookup-profile").addEventListener("submit", async (e) => {
  e.preventDefault();
  const id = $("#device-id").value.trim();
  const [profile, perso] = await Promise.all([
    api(`/api/v1/profiles/${encodeURIComponent(id)}`),
    api(`/api/v1/personalization/${encodeURIComponent(id)}`),
  ]);
  $("#profile-out").textContent = JSON.stringify({ profile, personalization: perso }, null, 2);
});

$("#btn-rebuild-one").addEventListener("click", async () => {
  const id = $("#device-id").value.trim();
  const pack = await api(`/api/v1/profiles/${encodeURIComponent(id)}/rebuild`, { method: "POST" });
  $("#profile-out").textContent = JSON.stringify(pack, null, 2);
  await loadDashboard();
});

$("#btn-rebuild-all").addEventListener("click", async () => {
  const r = await api("/api/v1/personalization/rebuild-all", { method: "POST" });
  alert(`已重建 ${r.rebuilt} 个设备`);
  await loadDashboard();
});

$("#btn-demo-habits").addEventListener("click", async () => {
  const device = $("#device-id").value.trim() || "demo-device-1";
  $("#device-id").value = device;
  const now = new Date().toISOString();
  const events = [
    { device_id: device, lang: "zh", pack_id: "zh-pack-v1", event_type: "select", query_key: "ta", selected_word: "他", candidate_pos: 4, privacy_ok: true, occurred_at: now },
    { device_id: device, lang: "zh", pack_id: "zh-pack-v1", event_type: "select", query_key: "ta", selected_word: "他", candidate_pos: 3, privacy_ok: true, occurred_at: now },
    { device_id: device, lang: "zh", pack_id: "zh-pack-v1", event_type: "select", query_key: "ta", selected_word: "他", candidate_pos: 2, privacy_ok: true, occurred_at: now },
    { device_id: device, lang: "zh", pack_id: "zh-pack-v1", event_type: "select", query_key: "nihao", selected_word: "你好", candidate_pos: 0, privacy_ok: true, occurred_at: now },
    { device_id: device, lang: "zh", pack_id: "zh-pack-v1", event_type: "backspace", query_key: "tai", privacy_ok: true, occurred_at: now },
  ];
  const r = await api("/api/v1/habits/events", {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Admin-Token": token() },
    body: JSON.stringify({ events }),
  });
  $("#habit-out").textContent = JSON.stringify(r, null, 2);
  const perso = await api(`/api/v1/personalization/${encodeURIComponent(device)}`);
  $("#profile-out").textContent = JSON.stringify(perso, null, 2);
  await loadDashboard();
});

$("#token").addEventListener("change", token);
token();
Promise.all([loadDashboard(), loadPacks()]).catch((e) => {
  $("#profile-out").textContent = String(e);
});
