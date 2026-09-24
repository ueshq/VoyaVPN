import { createServer } from "node:http";
import { createServer as createTcpServer } from "node:net";
import { networkInterfaces } from "node:os";
import { spawn } from "node:child_process";

export function lanAddress(interfaces = networkInterfaces()) {
  const address = Object.values(interfaces).flat().find((entry) =>
    entry && entry.family === "IPv4" && !entry.internal &&
    /^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.)/.test(entry.address));
  if (!address) throw new Error("A private LAN IPv4 address is required for the subscription fixture (the app rejects loopback subscriptions).");
  return address.address;
}

export async function unusedPort() {
  const server = createTcpServer();
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}

function clipboard(device, text) {
  return new Promise((resolve, reject) => {
    const child = spawn("xcrun", ["simctl", text === undefined ? "pbpaste" : "pbcopy", device]);
    let output = "";
    child.stdout.on("data", (data) => { output += data; });
    child.once("error", reject);
    child.once("close", (code) => code === 0 ? resolve(output) : reject(new Error(`simctl clipboard: ${code}`)));
    child.stdin.end(text);
  });
}

export async function startFixtures({ device, address, record }) {
  let delay = 0;
  let downloads = 0;
  const timers = new Set();
  const server = createServer(async (request, response) => {
    try {
      // Only the subscription endpoint needs LAN access. Clipboard/control
      // requests belong to the local XCTest runner, never another LAN client.
      if (["/clipboard", "/delay"].includes(request.url) && request.socket.remoteAddress !== "127.0.0.1") {
        response.writeHead(403); response.end(); return;
      }
      const chunks = [];
      for await (const chunk of request) chunks.push(chunk);
      const body = Buffer.concat(chunks).toString();
      record(`${request.method} ${request.url}`);
      switch (request.url) {
        case "/clipboard":
          response.end(await clipboard(device(), request.method === "POST" ? body : undefined));
          break;
        case "/delay":
          delay = Math.max(0, Math.min(10000, Number(body) || 0));
          response.end("ok");
          break;
        case "/subscription": {
          downloads += 1;
          const title = downloads === 1 ? "A" : downloads === 2 ? "Updated" : "Refreshed";
          response.setHeader("profile-title", "QA Subscription");
          response.end([
            `vless://55555555-5555-5555-5555-555555555555@qa-sub.example.test:443?security=tls#QA%20Subscription%20${title}`,
            "vless://66666666-6666-6666-6666-666666666666@qa-sub-b.example.test:443?security=tls#QA%20Subscription%20B",
          ].join("\n"));
          break;
        }
        case "/ping": {
          const timer = setTimeout(() => { timers.delete(timer); response.writeHead(204); response.end(); }, delay);
          timers.add(timer);
          break;
        }
        case "/ip": response.end('{"ip":"192.0.2.1","country":"QA"}'); break;
        case "/health": response.end("ok"); break;
        default: response.writeHead(404); response.end();
      }
    } catch (error) { response.writeHead(500); response.end(String(error)); }
  });
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "0.0.0.0", resolve); });
  const port = server.address().port;
  return {
    controlUrl: `http://127.0.0.1:${port}`,
    subscriptionUrl: `http://${address}:${port}/subscription`,
    close: async () => {
      for (const timer of timers) clearTimeout(timer);
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}
