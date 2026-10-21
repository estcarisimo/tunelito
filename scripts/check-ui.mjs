import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { setTimeout as sleep } from "node:timers/promises";

const CHROME = "/usr/local/bin/google-chrome";
const BASE = "http://127.0.0.1:1420/";
const OUT = new URL("../docs/images/", import.meta.url);

const chrome = spawn(
  CHROME,
  [
    "--headless=new",
    "--disable-gpu",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    `--user-data-dir=/tmp/tunelito-chrome-${process.pid}`,
    "--remote-debugging-port=9223",
    "--window-size=1280,900",
    "about:blank",
  ],
  { stdio: "ignore" },
);

let browserWs;
try {
  for (let i = 0; i < 40; i++) {
    try {
      const version = await fetch("http://127.0.0.1:9223/json/version").then((r) => r.json());
      browserWs = version.webSocketDebuggerUrl;
      break;
    } catch {
      await sleep(150);
    }
  }
  if (!browserWs) throw new Error("chrome did not open a debugging port");

  const browser = await connect(browserWs);
  await browser.send("Target.createTarget", { url: "about:blank" });
  const pages = await fetch("http://127.0.0.1:9223/json/list").then((response) => response.json());
  const pageWs = pages.find((item) => item.type === "page" && item.webSocketDebuggerUrl)?.webSocketDebuggerUrl;
  if (!pageWs) throw new Error(`no page target: ${JSON.stringify(pages)}`);
  const page = await connect(pageWs);
  await page.send("Page.enable");
  await page.send("Runtime.enable");

  const failures = [];
  function check(name, ok, detail) {
    if (!ok) failures.push(`${name}: ${detail}`);
    else console.log(`ok ${name}`);
  }

  await go(page, `${BASE}?lang=en`);
  let text = await bodyText(page);
  check("english warning", text.includes("Only do this with someone you know."), text.slice(0, 180));
  check("off switch", text.includes("Off"), text);

  await click(page, "button", "Español");
  await sleep(200);
  text = await bodyText(page);
  check("spanish warning", text.includes("Solo hacé esto con alguien que conocés."), text.slice(0, 240));

  await click(page, "button", "English");
  await sleep(200);

  await setField(page, "[data-testid='key-input']", "-----BEGIN OPENSSH PRIVATE KEY-----\nabc");
  await clickTestId(page, "add-key");
  await sleep(150);
  text = await bodyText(page);
  check("rejects private key", text.includes("That is a private key"), text.slice(0, 400));

  await setField(page, "[data-testid='key-input']", "hello this is not a key");
  await clickTestId(page, "add-key");
  await sleep(150);
  text = await bodyText(page);
  check("rejects junk", text.includes("does not look like a public key"), text.slice(0, 400));

  const sample =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH3R0+ef4Nv1ZvLTNm8jM4evFM5YW6JBNbxGC1BtP6K9 helper@laptop";
  await setField(page, "[data-testid='key-input']", sample);
  await clickTestId(page, "add-key");
  await sleep(150);
  text = await bodyText(page);
  check("adds key", text.includes("SHA256:UcC9dBHY9VaM1Pe2xCc0UH/tSRTEtGbGWnIOkEuAKJw"), text.slice(0, 500));
  check("shows comment", text.includes("helper@laptop"), "missing comment");

  await clickTestId(page, "power");
  await sleep(150);
  text = await bodyText(page);
  check("confirm dialog", text.includes("Turn on access?"), text.slice(0, 500));

  await clickTestId(page, "cancel-on");
  await sleep(100);
  text = await bodyText(page);
  check("cancel closes dialog", !text.includes("Turn on access?"), "dialog still open");

  await clickTestId(page, "power");
  await sleep(100);
  await clickTestId(page, "confirm-on");
  await sleep(200);
  text = await bodyText(page);
  check("hostname after on", text.includes("maple-river-42.trycloudflare.com"), text.slice(0, 700));
  check("someone connected no", text.includes("Someone is connected: No"), text.slice(0, 700));
  check("snippet", text.includes("ProxyCommand cloudflared access ssh --hostname %h"), "missing snippet");
  check("plain ssh user", text.includes("User tunelito"), "missing user");
  check("no hello hook in snippet", !text.includes("LocalCommand"), text);

  await clickTestId(page, "copy-host");
  await sleep(100);
  text = await bodyText(page);
  check("copy feedback", text.includes("Copied"), text.slice(0, 800));

  await go(page, `${BASE}?preview=connected&lang=en`);
  text = await bodyText(page);
  check("connected sample", text.includes("Someone is connected: Yes"), text.slice(0, 400));
  check("unverified label", text.includes("Reported by the client (unverified)"), text);
  check("loopback note", text.includes("127.0.0.1"), text);
  check("green and gray", (await count(page, ".dot.live")) >= 2 && (await count(page, ".dot:not(.live)")) >= 1, "dots");
  check("active and ended", text.includes("Active") && text.includes("Ended"), text);
  check("no client info", text.includes("No client info."), text);
  check("fingerprint without hello", (await count(page, "[data-testid='no-client-info']")) >= 1, "missing no-client-info row");
  await shot(page, new URL("tunelito-en.png", OUT), 1280);

  await go(page, `${BASE}?preview=connected&lang=es`);
  text = await bodyText(page);
  check("spanish connected", text.includes("Alguien está conectado: Sí"), text.slice(0, 400));
  check("spanish unverified", text.includes("Informado por el cliente (sin verificar)"), text);
  check("spanish no client info", text.includes("Sin datos del cliente."), text);
  await shot(page, new URL("tunelito-es.png", OUT), 1280);

  await page.send("Emulation.setDeviceMetricsOverride", {
    width: 390,
    height: 1400,
    deviceScaleFactor: 1,
    mobile: true,
  });
  await go(page, `${BASE}?preview=connected&lang=en`);
  const columns = await evalValue(
    page,
    "getComputedStyle(document.querySelector('.grid')).gridTemplateColumns",
  );
  check("narrow layout is one column", !String(columns).includes(" "), String(columns));

  if (failures.length) {
    console.error(failures.join("\n"));
    process.exitCode = 1;
  } else {
    console.log("ui checks passed");
  }
} finally {
  chrome.kill("SIGKILL");
}

async function connect(url) {
  const ws = new WebSocket(url);
  await new Promise((resolve, reject) => {
    ws.addEventListener("open", resolve);
    ws.addEventListener("error", reject);
  });
  let next = 0;
  const pending = new Map();
  ws.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      pending.get(message.id)(message);
      pending.delete(message.id);
    }
  });
  return {
    send(method, params = {}) {
      const id = ++next;
      ws.send(JSON.stringify({ id, method, params }));
      return new Promise((resolve, reject) => {
        pending.set(id, (message) => {
          if (message.error) reject(new Error(`${method}: ${JSON.stringify(message.error)}`));
          else resolve(message);
        });
      });
    },
  };
}

async function go(page, url) {
  await page.send("Page.navigate", { url });
  await sleep(400);
}

async function bodyText(page) {
  return evalValue(page, "document.body.innerText");
}

async function evalValue(page, expression) {
  const message = await page.send("Runtime.evaluate", {
    expression,
    returnByValue: true,
  });
  return message.result.result.value;
}

async function click(page, tag, label) {
  const expression = `(() => { const node = [...document.querySelectorAll(${JSON.stringify(tag)})].find((el) => el.textContent.trim() === ${JSON.stringify(label)}); if (!node) return false; node.click(); return true; })()`;
  const ok = await evalValue(page, expression);
  if (!ok) throw new Error(`no ${tag} labeled ${label}`);
}

async function clickTestId(page, id) {
  const ok = await evalValue(
    page,
    `(() => { const node = document.querySelector("[data-testid='${id}']"); if (!node) return false; node.click(); return true; })()`,
  );
  if (!ok) throw new Error(`no test id ${id}`);
}

async function setField(page, selector, value) {
  const expression = `(() => { const node = document.querySelector(${JSON.stringify(selector)}); if (!node) return false; node.value = ${JSON.stringify(value)}; node.dispatchEvent(new Event("input", { bubbles: true })); return true; })()`;
  const ok = await evalValue(page, expression);
  if (!ok) throw new Error(`no field ${selector}`);
}

async function count(page, selector) {
  return evalValue(page, `document.querySelectorAll(${JSON.stringify(selector)}).length`);
}

async function shot(page, fileUrl, width) {
  const height = await evalValue(page, "Math.ceil(document.documentElement.scrollHeight)");
  await page.send("Emulation.setDeviceMetricsOverride", {
    width,
    height: Math.min(height + 20, 2200),
    deviceScaleFactor: 1,
    mobile: false,
  });
  await sleep(150);
  const message = await page.send("Page.captureScreenshot", { format: "png" });
  await mkdir(new URL(".", fileUrl), { recursive: true });
  await writeFile(fileUrl, Buffer.from(message.result.data, "base64"));
  console.log(`wrote ${fileUrl.pathname}`);
}
