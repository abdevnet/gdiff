#!/usr/bin/env node

const path = require("path");
const net = require("net");
const { execSync, spawn } = require("child_process");

const repoArg = process.argv[2] || process.cwd();
const repoPath = path.resolve(repoArg);

try {
  execSync("git rev-parse --show-toplevel", { cwd: repoPath, stdio: "pipe" });
} catch {
  console.error(`Error: ${repoPath} is not inside a git repository`);
  process.exit(1);
}

const port = parseInt(process.env.PORT || "3420", 10);
const repoUrl = `http://localhost:${port}?repo=${encodeURIComponent(repoPath)}`;

function isPortInUse(p) {
  return new Promise((resolve) => {
    const tester = net.createServer()
      .once("error", () => resolve(true))
      .once("listening", () => tester.close(() => resolve(false)))
      .listen(p, "127.0.0.1");
  });
}

function openBrowser(url) {
  const fs = require("fs");
  const plat = process.platform;

  const chromePaths = {
    darwin: ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"],
    win32: [
      process.env.LOCALAPPDATA + "\\Google\\Chrome\\Application\\chrome.exe",
      process.env.PROGRAMFILES + "\\Google\\Chrome\\Application\\chrome.exe",
      process.env["PROGRAMFILES(X86)"] + "\\Google\\Chrome\\Application\\chrome.exe",
    ],
    linux: ["google-chrome", "chromium-browser", "chromium"],
  };

  function fallback() {
    if (plat === "darwin") spawn("open", [url]);
    else if (plat === "win32") spawn("cmd", ["/c", "start", "", url], { shell: true, windowsHide: true });
    else spawn("xdg-open", [url]);
  }

  const candidates = (chromePaths[plat] || []).filter((p) => {
    if (plat === "linux") return true; // linux uses PATH lookup
    try { return fs.existsSync(p); } catch { return false; }
  });

  if (candidates.length === 0) { fallback(); return; }

  const s = spawn(candidates[0], [`--app=${url}`], {
    stdio: "ignore",
    detached: true,
    windowsHide: true,
  });
  s.on("error", () => fallback());
  s.unref();
}

async function main() {
  // If a gdiff is already running, just point a new browser window at it.
  if (await isPortInUse(port)) {
    console.log(`gdiff already running on port ${port} — opening ${path.basename(repoPath)}`);
    openBrowser(repoUrl);
    process.exit(0);
  }

  // On Windows, re-spawn detached so the launching terminal isn't held.
  if (process.platform === "win32" && !process.env.GDIFF_DETACHED) {
    console.log(`gdiff serving ${path.basename(repoPath)} at http://localhost:${port}`);
    console.log("(running in background — stop via Task Manager: node.exe)");
    const child = spawn(process.execPath, [__filename, ...process.argv.slice(2)], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
      env: { ...process.env, GDIFF_DETACHED: "1" },
    });
    child.unref();
    process.exit(0);
  }

  process.argv[2] = repoPath;
  const { startServer } = require(path.join(__dirname, "..", "server.js"));
  const { port: actualPort } = await startServer(port);
  if (!process.env.GDIFF_DETACHED) {
    console.log(`gdiff serving ${path.basename(repoPath)} at http://localhost:${actualPort}`);
  }
  openBrowser(`http://localhost:${actualPort}?repo=${encodeURIComponent(repoPath)}`);
}

process.on("SIGINT", () => process.exit(0));
process.on("SIGTERM", () => process.exit(0));

main();
