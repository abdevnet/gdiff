const { execSync } = require("child_process");
const path = require("path");
const fs = require("fs");
const http = require("http");

let repoPath = process.argv[2] || process.cwd();
repoPath = path.resolve(repoPath);

// ── Git helpers ──

function git(args) {
  try {
    return execSync(`git ${args}`, {
      cwd: repoPath,
      encoding: "utf-8",
      maxBuffer: 10 * 1024 * 1024,
      stdio: ["pipe", "pipe", "pipe"],
    }).trim();
  } catch (e) {
    return e.stdout?.trim() || "";
  }
}

function getChangedFiles() {
  const raw = execSync("git status --porcelain", {
    cwd: repoPath, encoding: "utf-8", maxBuffer: 10 * 1024 * 1024,
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

    if (indexStatus !== " " && indexStatus !== "?") {
      results.push({ path: actualPath, status: label(indexStatus), staged: true });
    }
    if (workStatus !== " " && workStatus !== undefined) {
      results.push({ path: actualPath, status: label(workStatus), staged: false });
    }
  }
  return results;
}

function getFileContent(filePath, version) {
  try {
    if (version === "head") {
      return git(`show HEAD:"${filePath}"`);
    } else if (version === "staged") {
      return git(`show :"${filePath}"`);
    } else {
      const fullPath = path.join(repoPath, filePath);
      if (fs.existsSync(fullPath)) {
        return fs.readFileSync(fullPath, "utf-8");
      }
      return "";
    }
  } catch {
    return "";
  }
}

function getFileDiff(filePath, staged) {
  let original, modified;
  if (staged) {
    original = getFileContent(filePath, "head");
    modified = getFileContent(filePath, "staged");
  } else {
    try {
      original = git(`show :"${filePath}"`);
    } catch {
      original = getFileContent(filePath, "head");
    }
    modified = getFileContent(filePath, "working");
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

function stageFiles(filePaths) {
  const escaped = filePaths.map((f) => `"${f}"`).join(" ");
  git(`add -- ${escaped}`);
  return getChangedFiles();
}

function unstageFiles(filePaths) {
  const escaped = filePaths.map((f) => `"${f}"`).join(" ");
  const hasCommits = git("rev-parse HEAD") !== "";
  if (hasCommits) {
    git(`reset HEAD -- ${escaped}`);
  } else {
    git(`rm --cached -- ${escaped}`);
  }
  return getChangedFiles();
}

function discardFiles(filePaths) {
  const escaped = filePaths.map((f) => `"${f}"`).join(" ");
  git(`checkout -- ${escaped}`);
  return getChangedFiles();
}

function getRepoInfo() {
  const branch = git("branch --show-current") || git("rev-parse --short HEAD");
  const repoName = path.basename(repoPath);
  return { repoName, branch, repoPath };
}

// ── Exports for main.js ──

module.exports = {
  git, getChangedFiles, getFileDiff, getFileContent,
  detectLanguage, stageFiles, unstageFiles, discardFiles, getRepoInfo,
  repoPath,
};

// ── HTTP Server (only when run directly) ──

if (require.main === module) {
  const PORT = parseInt(process.env.PORT || "3420", 10);
  const sseClients = new Set();

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

  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, `http://localhost:${PORT}`);

    // CORS for local dev
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type");
    if (req.method === "OPTIONS") { res.writeHead(204); res.end(); return; }

    if (url.pathname === "/" || url.pathname === "/index.html") {
      const html = fs.readFileSync(path.join(__dirname, "index.html"), "utf-8");
      res.writeHead(200, { "Content-Type": "text/html" });
      res.end(html);
    } else if (url.pathname === "/api/repo-info") {
      json(res, getRepoInfo());
    } else if (url.pathname === "/api/changed-files") {
      json(res, getChangedFiles());
    } else if (url.pathname === "/api/file-diff") {
      const filePath = url.searchParams.get("path");
      const staged = url.searchParams.get("staged") === "true";
      const { original, modified } = getFileDiff(filePath, staged);
      const language = detectLanguage(filePath);
      json(res, { original, modified, language, filePath });
    } else if (url.pathname === "/api/stage" && req.method === "POST") {
      const { paths } = await readBody(req);
      json(res, stageFiles(paths));
    } else if (url.pathname === "/api/unstage" && req.method === "POST") {
      const { paths } = await readBody(req);
      json(res, unstageFiles(paths));
    } else if (url.pathname === "/api/discard" && req.method === "POST") {
      const { paths } = await readBody(req);
      json(res, discardFiles(paths));
    } else if (url.pathname === "/api/events") {
      res.writeHead(200, {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        Connection: "keep-alive",
      });
      res.write("data: connected\n\n");
      sseClients.add(res);
      req.on("close", () => sseClients.delete(res));
    } else {
      res.writeHead(404);
      res.end("Not found");
    }
  });

  // File watcher → push SSE events
  let watchDebounce = null;
  function notifyClients() {
    clearTimeout(watchDebounce);
    watchDebounce = setTimeout(() => {
      for (const client of sseClients) {
        client.write("data: files-changed\n\n");
      }
    }, 500);
  }

  (async () => {
    const chokidar = await import("chokidar");

    // Get tracked files from git to watch (instead of entire directory)
    const trackedFiles = git("ls-files").split("\n").filter(Boolean)
      .map(f => path.join(repoPath, f));

    const workTreeWatcher = chokidar.watch(trackedFiles, {
      ignoreInitial: true,
      persistent: true,
      depth: 0,
    });
    workTreeWatcher.on("all", notifyClients);
    workTreeWatcher.on("error", () => {});

    // Watch specific git state files for stage/commit/branch changes
    const gitWatcher = chokidar.watch([
      path.join(repoPath, ".git", "index"),
      path.join(repoPath, ".git", "HEAD"),
      path.join(repoPath, ".git", "refs"),
    ], {
      ignoreInitial: true,
      persistent: true,
    });
    gitWatcher.on("all", notifyClients);
    gitWatcher.on("error", () => {});
  })();

  server.listen(PORT, () => {
    console.log(`Git Diff Viewer serving ${path.basename(repoPath)} at http://localhost:${PORT}`);
  });
}
