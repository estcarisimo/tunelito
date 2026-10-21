import en from "./locales/en.json";
import es from "./locales/es.json";

const LOCALES = { en, es };
const TIMERS = [
  [1800, "timer.option_30m"],
  [3600, "timer.option_1h"],
  [7200, "timer.option_2h"],
  [14400, "timer.option_4h"],
  [28800, "timer.option_8h"],
  [86400, "timer.option_24h"],
];
const SAMPLE_PUB =
  "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH3R0+ef4Nv1ZvLTNm8jM4evFM5YW6JBNbxGC1BtP6K9 helper@laptop";
const SAMPLE_FP = "SHA256:UcC9dBHY9VaM1Pe2xCc0UH/tSRTEtGbGWnIOkEuAKJw";

const params = new URLSearchParams(location.search);
const inTauri = "__TAURI_INTERNALS__" in window;
let lang = params.get("lang") === "es" ? "es" : "en";
let messages = LOCALES[lang];
let view = null;
let lastJson = "";
let draft = "";
let dialog = false;
let copied = "";
let busy = false;

const app = document.querySelector("#app");

function t(key, vars) {
  let node = messages;
  for (const part of key.split(".")) node = node?.[part];
  let text = typeof node === "string" ? node : key;
  if (vars) {
    for (const [name, value] of Object.entries(vars)) {
      text = text.replaceAll(`{${name}}`, value);
    }
  }
  return text;
}

function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value == null || value === false) continue;
    if (key === "class") node.className = value;
    else if (key === "text") node.textContent = String(value);
    else if (key.startsWith("on") && typeof value === "function") {
      node.addEventListener(key.slice(2).toLowerCase(), value);
    } else node.setAttribute(key, String(value));
  }
  for (const child of children) {
    if (child == null) continue;
    node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return node;
}

function formatRemaining(seconds) {
  const total = Math.max(0, Number(seconds) || 0);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (hours > 0) return `${hours}h ${minutes}m`;
  const secs = Math.floor(total % 60);
  return minutes > 0 ? `${minutes}m ${secs}s` : `${secs}s`;
}

function formatWhen(iso) {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return new Intl.DateTimeFormat(lang === "es" ? "es" : "en", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function readError(err) {
  if (err && typeof err === "object" && typeof err.code === "string") return err;
  if (typeof err === "string") {
    try {
      const parsed = JSON.parse(err);
      if (parsed && parsed.code) return parsed;
    } catch {
      return { code: "generic", detail: err };
    }
  }
  return { code: "generic", detail: err ? String(err) : null };
}

function emptyView() {
  return {
    phase: "off",
    hostname: null,
    ssh_snippet: null,
    started_at: null,
    expires_at: null,
    seconds_remaining: null,
    someone_connected: false,
    active_session_count: 0,
    keys: [],
    sessions: [],
    unmatched_hellos: [],
    settings: {
      auto_off_secs: 7200,
      revoke_keys_on_off: false,
      language: lang,
      stop_sshd_if_started_by_us: true,
    },
    error_code: null,
    error_detail: null,
    activity: null,
    dedicated_user: "tunelito",
    version: "0.1.0",
  };
}

function connectedSample() {
  const now = Date.now();
  const started = new Date(now - 8 * 60 * 1000).toISOString();
  const ended = new Date(now - 3 * 60 * 60 * 1000).toISOString();
  const host = "maple-river-42.trycloudflare.com";
  const vm = emptyView();
  vm.phase = "on";
  vm.hostname = host;
  vm.ssh_snippet = [
    `# BEGIN tunelito ${host}`,
    `Host ${host}`,
    `HostName ${host}`,
    "User tunelito",
    "IdentityFile ~/.ssh/id_ed25519",
    "IdentitiesOnly yes",
    "StrictHostKeyChecking accept-new",
    "ProxyCommand cloudflared access ssh --hostname %h",
    `# END tunelito ${host}`,
    "",
  ].join("\n");
  vm.started_at = started;
  vm.expires_at = new Date(now + 2 * 60 * 60 * 1000).toISOString();
  vm.seconds_remaining = 2 * 60 * 60;
  vm.someone_connected = true;
  vm.active_session_count = 1;
  vm.activity = "ready";
  vm.keys = [
    {
      fingerprint: SAMPLE_FP,
      comment: "helper@laptop",
      key_type: "ssh-ed25519",
      added_at: new Date(now - 86400000).toISOString(),
    },
  ];
  vm.sessions = [
    {
      id: "active",
      active: true,
      user: "tunelito",
      fingerprint: SAMPLE_FP,
      observed_peer: "127.0.0.1",
      loopback: true,
      started_at: started,
      ended_at: null,
      hello: {
        public_ip: "203.0.113.10",
        asn: "AS64500",
        rdns: null,
        os: "linux",
        os_detail: "Debian",
        reported_at: started,
        verification: "unverified",
        label: "reported by client (unverified)",
      },
    },
    {
      id: "ended",
      active: false,
      user: "tunelito",
      fingerprint: SAMPLE_FP,
      observed_peer: "127.0.0.1",
      loopback: true,
      started_at: ended,
      ended_at: new Date(now - 2 * 60 * 60 * 1000).toISOString(),
      hello: null,
    },
  ];
  return vm;
}

function previewCall(command, args = {}) {
  const vm = view ? structuredClone(view) : emptyView();
  vm.error_code = null;
  vm.error_detail = null;
  if (command === "add_key") {
    const text = String(args.text || "");
    if (/PRIVATE KEY/.test(text)) {
      throw { code: "private_key_rejected" };
    }
    const line = text
      .split(/\r?\n/)
      .map((item) => item.trim())
      .find((item) => /^(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp256)\s+\S+/.test(item));
    if (!line) throw { code: "invalid_public_key" };
    const comment = line.split(/\s+/)[2] || "";
    if (vm.keys.some((key) => key.comment === comment && comment)) {
      throw { code: "duplicate_key" };
    }
    vm.keys.push({
      fingerprint: SAMPLE_FP,
      comment,
      key_type: line.split(/\s+/)[0],
      added_at: new Date().toISOString(),
    });
    draft = "";
  } else if (command === "remove_key") {
    const before = vm.keys.length;
    vm.keys = vm.keys.filter((key) => key.fingerprint !== args.fingerprint);
    if (vm.keys.length === before) throw { code: "key_not_found" };
    if (vm.phase !== "off") {
      vm.phase = "off";
      vm.hostname = null;
      vm.ssh_snippet = null;
      vm.someone_connected = false;
    }
  } else if (command === "turn_on") {
    if (vm.keys.length === 0) throw { code: "key_required" };
    const sample = connectedSample();
    sample.keys = vm.keys;
    sample.settings = vm.settings;
    sample.sessions = [];
    sample.someone_connected = false;
    sample.active_session_count = 0;
    return sample;
  } else if (command === "turn_off") {
    vm.phase = "off";
    vm.hostname = null;
    vm.ssh_snippet = null;
    vm.someone_connected = false;
    vm.sessions = vm.sessions.map((session) => ({
      ...session,
      active: false,
      ended_at: session.ended_at || new Date().toISOString(),
    }));
    vm.active_session_count = 0;
    if (args.revoke_keys || vm.settings.revoke_keys_on_off) vm.keys = [];
  } else if (command === "update_settings") {
    vm.settings = { ...vm.settings, ...args.settings };
    if (vm.settings.language === "es" || vm.settings.language === "en") {
      lang = vm.settings.language;
      messages = LOCALES[lang];
    }
  }
  vm.settings.language = lang;
  return vm;
}

async function call(command, args) {
  if (!inTauri) return previewCall(command, args);
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke(command, args);
}

function setView(next, force = false) {
  const json = JSON.stringify(next);
  view = next;
  if (next.settings?.language === "es" || next.settings?.language === "en") {
    if (lang !== next.settings.language) {
      lang = next.settings.language;
      messages = LOCALES[lang];
      force = true;
    }
  }
  if (!force && json === lastJson) return;
  lastJson = json;
  render();
}

async function run(command, args) {
  busy = true;
  render();
  try {
    const next = await call(command, args);
    busy = false;
    dialog = false;
    setView(next, true);
  } catch (err) {
    busy = false;
    const parsed = readError(err);
    const next = view ? { ...view } : emptyView();
    next.error_code = parsed.code || "generic";
    next.error_detail = parsed.detail || null;
    setView(next, true);
  }
}

function switchLabel() {
  if (view.phase === "on") return t("switch.on");
  if (view.phase === "starting") return t("switch.starting");
  return t("switch.off");
}

function renderHello(hello) {
  if (!hello) {
    return el("p", { class: "meta", "data-testid": "no-client-info", text: t("sessions.no_hello") });
  }
  const rows = [
    ["sessions.ip", hello.public_ip],
    ["sessions.asn", hello.asn],
    ["sessions.rdns", hello.rdns],
    ["sessions.os", [hello.os, hello.os_detail].filter(Boolean).join(" · ")],
  ];
  return el("div", { class: "hello" }, [
    el("strong", { text: t("sessions.reported_by_client") }),
    ...rows.map(([key, value]) =>
      el("p", { class: "meta", text: `${t(key)}: ${value || "—"}` }),
    ),
  ]);
}

function render() {
  document.documentElement.lang = lang;
  document.title = t("app.title");
  const phase = view?.phase || "off";
  const on = phase === "on" || phase === "starting";
  app.replaceChildren(
    el("header", { class: "top" }, [
      el("div", {}, [
        el("h1", { text: t("app.title") }),
        el("p", { class: "subtitle", text: t("app.subtitle") }),
      ]),
      el("div", { class: "langs", role: "group" }, [
        el("button", {
          type: "button",
          text: t("lang.en"),
          "aria-pressed": lang === "en",
          onclick: () => chooseLang("en"),
        }),
        el("button", {
          type: "button",
          text: t("lang.es"),
          "aria-pressed": lang === "es",
          onclick: () => chooseLang("es"),
        }),
      ]),
    ]),
    el("p", { class: "warning line", role: "note", text: t("warning.title") }),
    view?.error_code
      ? el("section", { class: "error", role: "alert" }, [
          el("strong", { text: t(`errors.${view.error_code}`) }),
          view.error_detail
            ? el("p", { text: view.error_detail })
            : null,
        ])
      : null,
    el("div", { class: "grid" }, [
      el("section", { class: "card main" }, [phase === "off" ? renderPaste() : renderLive(phase)]),
    ]),
    el("details", { class: "more", "data-testid": "more" }, [
      el("summary", { text: t("more.summary") }),
      renderTimer(),
      renderPresence(),
      renderKeyList(),
      renderSessions(),
      phase === "off" ? null : renderSnippet(),
    ]),
    dialog ? renderDialog() : null,
  );
  const area = app.querySelector("[data-testid='key-input']");
  if (area) area.value = draft;
  if (copied) {
    const button = app.querySelector(`[data-copy='${copied}']`);
    if (button) button.textContent = t("tunnel.copied");
  }
}

function renderPaste() {
  return el("div", { class: "paste" }, [
    el("label", { class: "paste-label", for: "key-input", text: t("keys.label") }),
    el("textarea", {
      id: "key-input",
      "data-testid": "key-input",
      placeholder: t("keys.placeholder"),
      oninput: (event) => {
        draft = event.target.value;
      },
    }),
    el("button", {
      type: "button",
      class: "switch off",
      "data-testid": "power",
      disabled: busy,
      text: t("switch.turn_on"),
      onclick: onPower,
    }),
  ]);
}

function renderLive(phase) {
  const waiting = !view.hostname;
  return el("div", { class: "live" }, [
    phase === "starting" || (view.activity && waiting)
      ? el("p", { class: "meta", text: t(`activity.${view.activity || "starting_tunnel"}`) })
      : null,
    el("p", { class: "url-label", text: t("tunnel.hostname") }),
    el("code", {
      class: "url",
      "data-testid": "hostname",
      text: waiting ? t("tunnel.waiting") : view.hostname,
    }),
    waiting
      ? null
      : el("div", { class: "share-row" }, [
          el("button", {
            type: "button",
            class: "share",
            "data-copy": "host",
            "data-testid": "copy-host",
            text: t("tunnel.copy"),
            onclick: () => copyText(view.hostname, "host"),
          }),
          el("button", {
            type: "button",
            class: "share",
            "data-copy": "share",
            "data-testid": "share-host",
            text: t("tunnel.share"),
            onclick: shareUrl,
          }),
        ]),
    el("button", {
      type: "button",
      class: "ghost off-button",
      "data-testid": "turn-off",
      disabled: busy || phase === "starting",
      text: t("switch.turn_off"),
      onclick: () => run("turn_off", { revoke_keys: Boolean(view.settings.revoke_keys_on_off) }),
    }),
  ]);
}

function renderTimer() {
  return el("label", { class: "field" }, [
    t("timer.label"),
    (() => {
      const select = el("select", {
        "data-testid": "timer",
        onchange: (event) =>
          run("update_settings", {
            settings: { ...view.settings, auto_off_secs: Number(event.target.value) },
          }),
      });
      for (const [secs, key] of TIMERS) {
        const option = el("option", { value: secs, text: t(key) });
        if (Number(view.settings.auto_off_secs) === secs) option.selected = true;
        select.append(option);
      }
      return select;
    })(),
    view?.seconds_remaining != null && view.phase !== "off"
      ? el("span", {
          class: "meta",
          text: t("timer.remaining", { time: formatRemaining(view.seconds_remaining) }),
        })
      : null,
  ]);
}

function renderPresence() {
  return el("div", { class: "presence", "data-testid": "presence" }, [
    el("span", { class: view?.someone_connected ? "dot live" : "dot" }),
    el("span", {
      text: `${t("connected.label")}: ${view?.someone_connected ? t("connected.yes") : t("connected.no")}`,
    }),
  ]);
}

function renderKeyList() {
  return el("section", {}, [
    el("h2", { text: t("keys.title") }),
    view.keys.length
      ? el(
          "div",
          {},
          view.keys.map((key) =>
            el("article", { class: "keys" }, [
              el("div", { class: "key-top" }, [
                el("strong", { text: key.comment || key.key_type }),
                el("button", {
                  type: "button",
                  class: "ghost",
                  text: t("keys.remove"),
                  onclick: () => run("remove_key", { fingerprint: key.fingerprint }),
                }),
              ]),
              el("p", { class: "meta", text: `${t("keys.fingerprint")}: ${key.fingerprint}` }),
              el("p", { class: "meta", text: `${t("keys.added")}: ${formatWhen(key.added_at)}` }),
            ]),
          ),
        )
      : el("p", { class: "help", text: t("keys.empty") }),
    el("label", { class: "check" }, [
      (() => {
        const box = el("input", {
          type: "checkbox",
          "data-testid": "revoke",
          onchange: (event) =>
            run("update_settings", {
              settings: { ...view.settings, revoke_keys_on_off: event.target.checked },
            }),
        });
        box.checked = Boolean(view.settings.revoke_keys_on_off);
        return box;
      })(),
      el("span", { text: t("keys.revoke_on_off") }),
    ]),
  ]);
}

function renderSessions() {
  return el("section", { class: "sessions" }, [
    el("h2", { text: t("sessions.title") }),
    view.sessions.length
      ? el(
          "div",
          {},
          view.sessions.map((session) =>
            el("article", { class: "session", "data-testid": "session" }, [
              el("div", { class: "session-top" }, [
                el("strong", { class: "session-title" }, [
                  el("span", { class: session.active ? "dot live" : "dot" }),
                  session.active ? t("sessions.active") : t("sessions.ended"),
                ]),
                el("span", { class: "meta", text: session.user }),
              ]),
              el("p", { class: "meta", text: `${t("keys.fingerprint")}: ${session.fingerprint || "—"}` }),
              el("p", { class: "meta", text: `${t("sessions.started")}: ${formatWhen(session.started_at)}` }),
              session.ended_at
                ? el("p", {
                    class: "meta",
                    text: `${t("sessions.ended_at")}: ${formatWhen(session.ended_at)}`,
                  })
                : null,
              el("p", {
                class: "meta",
                text: `${t("sessions.observed_peer")}: ${session.observed_peer}`,
              }),
              session.loopback ? el("p", { class: "meta", text: t("sessions.loopback_note") }) : null,
              renderHello(session.hello),
            ]),
          ),
        )
      : el("p", { class: "help", text: t("sessions.empty") }),
    view.unmatched_hellos?.length
      ? el("p", { class: "meta", text: t("sessions.unmatched_hello") })
      : null,
  ]);
}

function renderSnippet() {
  if (!view.ssh_snippet) return null;
  return el("div", { class: "snippet" }, [
    el("strong", { text: t("tunnel.snippet") }),
    el("p", { class: "help", text: t("tunnel.snippet_help") }),
    el("pre", { text: view.ssh_snippet }),
    el("button", {
      type: "button",
      class: "copy",
      "data-copy": "snippet",
      "data-testid": "copy-snippet",
      text: t("tunnel.copy_snippet"),
      onclick: () => copyText(view.ssh_snippet, "snippet"),
    }),
  ]);
}

function renderDialog() {
  return el("div", { class: "modal" }, [
    el("div", { class: "dialog", role: "dialog", "aria-modal": "true" }, [
      el("h3", { text: t("switch.confirm_on") }),
      el("p", { text: t("switch.confirm_on_body") }),
      el("div", { class: "actions" }, [
        el("button", {
          type: "button",
          class: "ghost",
          "data-testid": "cancel-on",
          text: t("switch.cancel"),
          onclick: () => {
            dialog = false;
            render();
          },
        }),
        el("button", {
          type: "button",
          class: "primary",
          "data-testid": "confirm-on",
          text: t("switch.confirm"),
          onclick: () => confirmOn(),
        }),
      ]),
    ]),
  ]);
}

function looksLikeKey(text) {
  return text
    .split(/\r?\n/)
    .map((item) => item.trim())
    .some((item) => /^(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp256)\s+\S+/.test(item));
}

function onPower() {
  const text = draft.trim();
  if (/PRIVATE KEY/.test(text)) {
    setView({ ...view, error_code: "private_key_rejected", error_detail: null }, true);
    return;
  }
  if (text && !looksLikeKey(text)) {
    setView({ ...view, error_code: "invalid_public_key", error_detail: null }, true);
    return;
  }
  if (!text && view.keys.length === 0) {
    setView({ ...view, error_code: "key_required", error_detail: null }, true);
    return;
  }
  dialog = true;
  render();
}

async function confirmOn() {
  dialog = false;
  const text = draft.trim();
  if (text) {
    try {
      view = await call("add_key", { text });
      draft = "";
    } catch (err) {
      const parsed = readError(err);
      if (parsed.code !== "duplicate_key") {
        setView(
          { ...(view || emptyView()), error_code: parsed.code, error_detail: parsed.detail || null },
          true,
        );
        return;
      }
    }
  }
  await run("turn_on");
}

async function shareUrl() {
  const text = view.hostname || "";
  if (navigator.share) {
    try {
      await navigator.share({ title: t("app.title"), text });
      return;
    } catch {
      /* fall through to copy */
    }
  }
  await copyText(text, "share");
}

function chooseLang(next) {
  lang = next;
  messages = LOCALES[next];
  lastJson = "";
  run("update_settings", { settings: { ...view.settings, language: next } });
}

async function copyText(text, which) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    document.body.append(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }
  copied = which;
  render();
  setTimeout(() => {
    if (copied === which) {
      copied = "";
      render();
    }
  }, 1500);
}

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && dialog) {
    dialog = false;
    render();
  }
});

view = params.get("preview") === "connected" && !inTauri ? connectedSample() : emptyView();
view.settings.language = lang;
setView(view, true);

async function refreshQuiet() {
  try {
    const next = await call("refresh");
    setView(next);
  } catch (err) {
    const parsed = readError(err);
    setView(
      { ...(view || emptyView()), error_code: parsed.code, error_detail: parsed.detail || null },
      true,
    );
  }
}

if (inTauri) {
  refreshQuiet().then(() => setInterval(refreshQuiet, 2000));
}
