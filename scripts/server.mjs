import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize, resolve, sep } from "node:path";

const contentTypes = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".wasm", "application/wasm"],
]);

export async function startServer(root, port = 0, options = {}) {
  const resolvedRoot = resolve(root);
  const crossOriginIsolation = options.crossOriginIsolation ?? true;
  const host = options.host ?? "127.0.0.1";
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      const relative = normalize(decodeURIComponent(url.pathname)).replace(/^[/\\]+/, "") || "index.html";
      let path = resolve(join(resolvedRoot, relative));
      if (path !== resolvedRoot && !path.startsWith(`${resolvedRoot}${sep}`)) throw new Error("path escapes root");
      let info = await stat(path);
      if (info.isDirectory()) {
        path = resolve(join(path, "index.html"));
        if (!path.startsWith(`${resolvedRoot}${sep}`)) throw new Error("path escapes root");
        info = await stat(path);
      }
      if (!info.isFile()) throw new Error("not a file");
      const headers = {
        "Content-Type": contentTypes.get(extname(path)) ?? "application/octet-stream",
        "Cache-Control": "no-store",
      };
      if (crossOriginIsolation) {
        headers["Cross-Origin-Opener-Policy"] = "same-origin";
        headers["Cross-Origin-Embedder-Policy"] = "require-corp";
      }
      response.writeHead(200, headers);
      response.end(await readFile(path));
    } catch {
      response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
      response.end("Not found");
    }
  });
  await new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(port, host, resolveListen);
  });
  return server;
}
