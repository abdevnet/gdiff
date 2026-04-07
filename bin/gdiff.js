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

// Override argv so server.js picks up the repo path
process.argv[2] = repoPath;

const { startServer } = require(path.join(__dirname, "..", "server.js"));

function openBrowser(url) {
  const plat = process.platform;
  // Try Chrome/Chromium in app mode (own window, no tabs)
  const chromePaths = {
    darwin: ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"],
    win32: [
      process.env.LOCALAPPDATA + "\\Google\\Chrome\\Application\\chrome.exe",
      process.env.PROGRAMFILES + "\\Google\\Chrome\\Application\\chrome.exe",
    ],
    linux: ["google-chrome", "chromium-browser", "chromium"],
  };
  const candidates = chromePaths[plat] || [];
  for (const bin of candidates) {
    try {
      const s = spawn(bin, [`--new-window`, `--app=${url}`], { stdio: "ignore", detached: true });
      s.unref();
      return;
    } catch {}
  }
  // Fallback: default browser
  if (plat === "darwin") spawn("open", [url]);
  else if (plat === "win32") spawn("cmd", ["/c", "start", "", url], { shell: true });
  else spawn("xdg-open", [url]);
}

startServer().then(({ port }) => {
  const url = `http://localhost:${port}`;
  console.log(`gdiff serving ${path.basename(repoPath)} at ${url}`);
  openBrowser(url);
});

process.on("SIGINT", () => process.exit(0));
process.on("SIGTERM", () => process.exit(0));
