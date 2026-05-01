#!/usr/bin/env node

const path = require("path");
const { execSync, spawn } = require("child_process");

const repoArg = process.argv[2] || process.cwd();
const repoPath = path.resolve(repoArg);

try {
  execSync("git rev-parse --show-toplevel", { cwd: repoPath, stdio: "pipe" });
} catch {
  console.error(`Error: ${repoPath} is not inside a git repository`);
  process.exit(1);
}

// On Windows, re-spawn ourselves detached so the launching terminal isn't held.
if (process.platform === "win32" && !process.env.GDIFF_DETACHED) {
  const port = process.env.PORT || "3420";
  console.log(`gdiff serving ${path.basename(repoPath)} at http://localhost:${port}`);
  console.log("(running in background — stop via Task Manager: node.exe)");
  const child = spawn(process.execPath, [__filename, ...process.argv.slice(2)], {
    detached: true,
    stdio: "ignore",
    env: { ...process.env, GDIFF_DETACHED: "1" },
  });
  child.unref();
  process.exit(0);
}

// Override argv so server.js picks up the repo path
process.argv[2] = repoPath;

const { startServer } = require(path.join(__dirname, "..", "server.js"));

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
    else if (plat === "win32") spawn("cmd", ["/c", "start", "", url], { shell: true });
    else spawn("xdg-open", [url]);
  }

  const candidates = (chromePaths[plat] || []).filter((p) => {
    if (plat === "linux") return true; // linux uses PATH lookup
    try { return fs.existsSync(p); } catch { return false; }
  });

  if (candidates.length === 0) { fallback(); return; }

  const s = spawn(candidates[0], ["--new-window", `--app=${url}`], { stdio: "ignore", detached: true });
  s.on("error", () => fallback());
  s.unref();
}

startServer().then(({ port }) => {
  const url = `http://localhost:${port}`;
  console.log(`gdiff serving ${path.basename(repoPath)} at ${url}`);
  openBrowser(url);
});

process.on("SIGINT", () => process.exit(0));
process.on("SIGTERM", () => process.exit(0));
