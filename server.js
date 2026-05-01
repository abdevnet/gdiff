const { execSync, spawn } = require("child_process");
const path = require("path");
const fs = require("fs");
const os = require("os");
const http = require("http");
const zlib = require("zlib");

// ── User config (~/.gdiff-viewer.json) ──

const configPath = path.join(os.homedir(), ".gdiff-viewer.json");

const DEFAULT_EDITOR_COMMANDS = {
  darwin: "open -a Zed {file}",
  win32: "notepad {file}",
  linux: "xdg-open {file}",
};

function getConfig() {
  try {
    return JSON.parse(fs.readFileSync(configPath, "utf-8"));
  } catch {
    return {};
  }
}

function setConfig(partial) {
  const merged = { ...getConfig(), ...partial };
  fs.writeFileSync(configPath, JSON.stringify(merged, null, 2));
  return merged;
}

function getEditorCommand() {
  const c = getConfig();
  return c.editorCommand
    || DEFAULT_EDITOR_COMMANDS[process.platform]
    || "xdg-open {file}";
}

function openInEditor(absPath) {
  const cmd = getEditorCommand();
  const parts = cmd.split(/\s+/).filter(Boolean).map((p) =>
    p === "{file}" ? absPath : p,
  );
  if (!parts.some((p) => p === absPath)) parts.push(absPath);
  const exe = parts.shift();
  spawn(exe, parts, { detached: true, stdio: "ignore", windowsHide: true }).unref();
}

// ── Bundled JetBrains themes ──

const themesData = (() => {
  try {
    return JSON.parse(
      fs.readFileSync(path.join(__dirname, "themes.json"), "utf-8"),
    );
  } catch {
    return {};
  }
})();

function getThemes() { return themesData; }

let defaultRepoPath = process.argv[2] || process.cwd();
defaultRepoPath = path.resolve(defaultRepoPath);
try {
  defaultRepoPath = execSync("git rev-parse --show-toplevel", {
    cwd: defaultRepoPath, encoding: "utf-8",
  }).trim();
} catch {}


// ── Git helpers ──

function git(args, repo) {
  repo = repo || defaultRepoPath;
  try {
    return execSync(`git ${args}`, {
      cwd: repo,
      encoding: "utf-8",
      maxBuffer: 10 * 1024 * 1024,
      stdio: ["pipe", "pipe", "pipe"],
    }).trim();
  } catch (e) {
    return e.stdout?.trim() || "";
  }
}

function listFilesRecursive(dirPath, prefix) {
  const results = [];
  const items = fs.readdirSync(dirPath, { withFileTypes: true });
  for (const item of items) {
    const rel = prefix + item.name;
    if (item.isDirectory()) {
      results.push(...listFilesRecursive(path.join(dirPath, item.name), rel + "/"));
    } else {
      results.push(rel);
    }
  }
  return results;
}

function getChangedFiles(repo) {
  repo = repo || defaultRepoPath;
  const raw = execSync("git status --porcelain", {
    cwd: repo, encoding: "utf-8", maxBuffer: 10 * 1024 * 1024,
  }).trimEnd();
  if (!raw) return [];

  const results = [];
  for (const line of raw.split("\n")) {
    const status = line.substring(0, 2);
    const filePath = line.substring(3).trim();
    const actualPath = filePath.includes(" -> ")
      ? filePath.split(" -> ")[1]
      : filePath;

    const indexStatus = status[0];
    const workStatus = status[1];

    function label(s) {
      if (s === "A" || s === "?") return "added";
      if (s === "D") return "deleted";
      if (s === "R") return "renamed";
      return "modified";
    }

    // Untracked directories: git shows "?? dir/" — expand into individual files
    if (status === "??" && actualPath.endsWith("/")) {
      const dirFull = path.join(repo, actualPath);
      try {
        const files = listFilesRecursive(dirFull, actualPath);
        for (const f of files) {
          results.push({ path: f, status: "added", staged: false });
        }
      } catch {
        results.push({ path: actualPath, status: "added", staged: false });
      }
      continue;
    }

    if (indexStatus !== " " && indexStatus !== "?") {
      results.push({ path: actualPath, status: label(indexStatus), staged: true });
    }
    if (workStatus !== " " && workStatus !== undefined) {
      results.push({ path: actualPath, status: label(workStatus), staged: false });
    }
  }
  return results;
}

function getFileContent(filePath, version, repo) {
  repo = repo || defaultRepoPath;
  try {
    if (version === "head") {
      return git(`show HEAD:"${filePath}"`, repo);
    } else if (version === "staged") {
      return git(`show :"${filePath}"`, repo);
    } else {
      const fullPath = path.join(repo, filePath);
      if (fs.existsSync(fullPath)) {
        return fs.readFileSync(fullPath, "utf-8");
      }
      return "";
    }
  } catch {
    return "";
  }
}

function getFileDiff(filePath, staged, repo) {
  repo = repo || defaultRepoPath;
  let original, modified;
  if (staged) {
    original = getFileContent(filePath, "head", repo);
    modified = getFileContent(filePath, "staged", repo);
  } else {
    original = git(`show :"${filePath}"`, repo) || git(`show HEAD:"${filePath}"`, repo);
    modified = getFileContent(filePath, "working", repo);
  }
  return { original, modified };
}

function detectLanguage(filePath) {
  const ext = path.extname(filePath).toLowerCase();
  const map = {
    ".js": "javascript", ".jsx": "javascript",
    ".ts": "typescript", ".tsx": "typescript",
    ".cs": "csharp", ".py": "python", ".rb": "ruby",
    ".go": "go", ".rs": "rust", ".java": "java",
    ".cpp": "cpp", ".c": "c", ".h": "c",
    ".css": "css", ".scss": "scss",
    ".html": "html", ".xml": "xml", ".json": "json",
    ".yaml": "yaml", ".yml": "yaml",
    ".md": "markdown", ".sql": "sql",
    ".sh": "shell", ".bash": "shell",
    ".ps1": "powershell", ".psm1": "powershell",
    ".csproj": "xml", ".sln": "plaintext",
    ".toml": "plaintext", ".dockerfile": "dockerfile",
  };
  return map[ext] || "plaintext";
}

function stageFiles(filePaths, repo) {
  repo = repo || defaultRepoPath;
  const escaped = filePaths.map((f) => `"${f}"`).join(" ");
  git(`add -- ${escaped}`, repo);
  return getChangedFiles(repo);
}

function unstageFiles(filePaths, repo) {
  repo = repo || defaultRepoPath;
  const escaped = filePaths.map((f) => `"${f}"`).join(" ");
  const hasCommits = git("rev-parse HEAD", repo) !== "";
  if (hasCommits) {
    git(`reset HEAD -- ${escaped}`, repo);
  } else {
    git(`rm --cached -- ${escaped}`, repo);
  }
  return getChangedFiles(repo);
}

function discardFiles(filePaths, repo) {
  repo = repo || defaultRepoPath;
  const escaped = filePaths.map((f) => `"${f}"`).join(" ");
  git(`checkout -- ${escaped}`, repo);
  return getChangedFiles(repo);
}

function getFileTree(dirPath, repo, prefix) {
  repo = repo || defaultRepoPath;
  dirPath = dirPath || repo;
  prefix = prefix || "";
  const entries = [];
  try {
    const items = fs.readdirSync(dirPath, { withFileTypes: true });
    const skip = new Set([".git", "node_modules", ".DS_Store", "bin", "obj", ".vs"]);
    items.sort((a, b) => {
      if (a.isDirectory() !== b.isDirectory()) return a.isDirectory() ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    for (const item of items) {
      if (skip.has(item.name)) continue;
      const rel = prefix ? `${prefix}/${item.name}` : item.name;
      if (item.isDirectory()) {
        entries.push({ name: item.name, path: rel, type: "dir", children: getFileTree(path.join(dirPath, item.name), repo, rel) });
      } else {
        entries.push({ name: item.name, path: rel, type: "file" });
      }
    }
  } catch {}
  return entries;
}

function getRepoInfo(repo) {
  repo = repo || defaultRepoPath;
  const branch = git("branch --show-current", repo) || git("rev-parse --short HEAD", repo);
  const repoName = path.basename(repo);
  return { repoName, branch, repoPath: repo };
}

// ── Exports for main.js ──

module.exports = {
  git, getChangedFiles, getFileDiff, getFileContent,
  detectLanguage, stageFiles, unstageFiles, discardFiles, getFileTree, getRepoInfo,
  getThemes,
  getConfig, setConfig, openInEditor,
  startServer, repoPath: defaultRepoPath,
};

// ── HTTP Server (only when run directly) ──

// Serve bundled Monaco from /vs/* with in-memory gzip cache. Compressing once
// per server lifetime keeps repeat requests fast; cold start is one-shot.
const _vsCache = new Map();
const VS_TYPES = {
  ".js": "application/javascript",
  ".css": "text/css",
  ".ttf": "font/ttf",
  ".svg": "image/svg+xml",
  ".json": "application/json",
};
function serveVs(req, res, pathname) {
  const rel = pathname.replace(/^\//, "");
  const file = path.join(__dirname, rel);
  const root = path.join(__dirname, "vs") + path.sep;
  if (!file.startsWith(root) || !fs.existsSync(file)) {
    res.writeHead(404); res.end(); return;
  }
  let cached = _vsCache.get(file);
  if (!cached) {
    const raw = fs.readFileSync(file);
    const ext = path.extname(file);
    const compressible = ext === ".js" || ext === ".css" || ext === ".svg";
    cached = {
      raw,
      gz: compressible ? zlib.gzipSync(raw) : null,
      type: VS_TYPES[ext] || "application/octet-stream",
    };
    _vsCache.set(file, cached);
  }
  const acceptsGzip = (req.headers["accept-encoding"] || "").includes("gzip");
  const headers = {
    "Content-Type": cached.type,
    "Cache-Control": "public, max-age=31536000, immutable",
  };
  if (acceptsGzip && cached.gz) {
    headers["Content-Encoding"] = "gzip";
    headers["Content-Length"] = cached.gz.length;
    res.writeHead(200, headers);
    res.end(cached.gz);
  } else {
    headers["Content-Length"] = cached.raw.length;
    res.writeHead(200, headers);
    res.end(cached.raw);
  }
}

function startServer(port) {
  if (port == null) port = parseInt(process.env.PORT || "3420", 10);
  const sseClients = new Map(); // repo -> Set<res>

  function json(res, data) {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify(data));
  }

  function readBody(req) {
    return new Promise((resolve) => {
      let body = "";
      req.on("data", (chunk) => (body += chunk));
      req.on("end", () => resolve(body ? JSON.parse(body) : {}));
    });
  }

  function resolveRepo(url) {
    const r = url.searchParams.get("repo");
    return r ? path.resolve(r) : defaultRepoPath;
  }

  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, `http://localhost:${port}`);

    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type");
    if (req.method === "OPTIONS") { res.writeHead(204); res.end(); return; }

    if (url.pathname === "/" || url.pathname === "/index.html") {
      let html = fs.readFileSync(path.join(__dirname, "index.html"), "utf-8");
      const cfgTheme = (getConfig().theme || "default").replace(/[^a-zA-Z0-9._-]/g, "");
      // Inject the saved theme so the very first paint is correct, even when
      // localStorage is empty (e.g. fresh ephemeral-port origin on Windows).
      html = html.replace(
        /<html lang="en">/,
        `<html lang="en" data-theme-init="${cfgTheme}">`,
      );
      res.writeHead(200, { "Content-Type": "text/html" });
      res.end(html);
    } else if (url.pathname.startsWith("/vs/")) {
      serveVs(req, res, url.pathname);
    } else if (url.pathname === "/favicon.png") {
      const icon = fs.readFileSync(path.join(__dirname, "favicon.png"));
      res.writeHead(200, { "Content-Type": "image/png" });
      res.end(icon);
    } else if (url.pathname === "/api/repo-info") {
      json(res, getRepoInfo(resolveRepo(url)));
    } else if (url.pathname === "/api/changed-files") {
      json(res, getChangedFiles(resolveRepo(url)));
    } else if (url.pathname === "/api/file-diff") {
      const repo = resolveRepo(url);
      const filePath = url.searchParams.get("path");
      const staged = url.searchParams.get("staged") === "true";
      const { original, modified } = getFileDiff(filePath, staged, repo);
      const language = detectLanguage(filePath);
      json(res, { original, modified, language, filePath });
    } else if (url.pathname === "/api/stage" && req.method === "POST") {
      const { paths, repo: bodyRepo } = await readBody(req);
      const repo = bodyRepo ? path.resolve(bodyRepo) : resolveRepo(url);
      json(res, stageFiles(paths, repo));
    } else if (url.pathname === "/api/unstage" && req.method === "POST") {
      const { paths, repo: bodyRepo } = await readBody(req);
      const repo = bodyRepo ? path.resolve(bodyRepo) : resolveRepo(url);
      json(res, unstageFiles(paths, repo));
    } else if (url.pathname === "/api/discard" && req.method === "POST") {
      const { paths, repo: bodyRepo } = await readBody(req);
      const repo = bodyRepo ? path.resolve(bodyRepo) : resolveRepo(url);
      json(res, discardFiles(paths, repo));
    } else if (url.pathname === "/api/events") {
      const repo = resolveRepo(url);
      res.writeHead(200, {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        Connection: "keep-alive",
      });
      res.write("data: connected\n\n");
      if (!sseClients.has(repo)) sseClients.set(repo, new Set());
      sseClients.get(repo).add(res);
      ensureWatcher(repo);
      req.on("close", () => {
        const clients = sseClients.get(repo);
        if (clients) { clients.delete(res); if (!clients.size) sseClients.delete(repo); }
      });
    } else if (url.pathname === "/api/file-tree") {
      const repo = resolveRepo(url);
      const sub = url.searchParams.get("path") || "";
      const dir = sub ? path.join(repo, sub) : repo;
      json(res, getFileTree(dir, repo, sub));
    } else if (url.pathname === "/api/themes") {
      json(res, getThemes());
    } else if (url.pathname === "/api/open-in-editor") {
      const repo = resolveRepo(url);
      const filePath = url.searchParams.get("path");
      openInEditor(path.resolve(repo, filePath));
      json(res, { ok: true });
    } else if (url.pathname === "/api/config" && req.method === "GET") {
      json(res, getConfig());
    } else if (url.pathname === "/api/config" && req.method === "POST") {
      const body = await readBody(req);
      json(res, setConfig(body));
    } else {
      res.writeHead(404);
      res.end("Not found");
    }
  });

  // File watcher → push SSE events per repo
  const watchers = new Map();

  function ensureWatcher(repo) {
    if (watchers.has(repo)) return;
    let debounce = null;
    import("chokidar").then((chokidar) => {
      const trackedFiles = git("ls-files", repo).split("\n").filter(Boolean)
        .map(f => path.join(repo, f));

      const workTreeWatcher = chokidar.watch(trackedFiles, {
        ignoreInitial: true, persistent: true, depth: 0,
      });
      const gitWatcher = chokidar.watch([
        path.join(repo, ".git", "index"),
        path.join(repo, ".git", "HEAD"),
        path.join(repo, ".git", "refs"),
      ], { ignoreInitial: true, persistent: true });

      function notify() {
        clearTimeout(debounce);
        debounce = setTimeout(() => {
          const clients = sseClients.get(repo);
          if (clients) for (const c of clients) c.write("data: files-changed\n\n");
        }, 500);
      }

      workTreeWatcher.on("all", notify);
      workTreeWatcher.on("error", () => {});
      gitWatcher.on("all", notify);
      gitWatcher.on("error", () => {});
      watchers.set(repo, { workTreeWatcher, gitWatcher });
    });
  }

  // Watch default repo on startup
  ensureWatcher(defaultRepoPath);

  return new Promise((resolve) => {
    server.listen(port, () => {
      const actualPort = server.address().port;
      resolve({ server, port: actualPort });
    });
  });
}

if (require.main === module) {
  startServer().then(({ port }) => {
    console.log(`Git Diff Viewer serving ${path.basename(defaultRepoPath)} at http://localhost:${port}`);
    console.log(`Open other repos: http://localhost:${port}?repo=/path/to/repo`);
  });
}
