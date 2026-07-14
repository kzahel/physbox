export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === "/favicon.ico") return secured(new Response(null, { status: 204 }));
    let key = url.pathname.replace(/^\/+/, "");
    if (key === "" || key.endsWith("/")) key = `${key}index.html`;
    const object = await env.BUCKET.get(key);
    if (!object) return secured(new Response("Not Found", { status: 404 }));
    const headers = new Headers();
    object.writeHttpMetadata(headers);
    headers.set("etag", object.httpEtag);
    headers.set("Cache-Control", /\.(html|js|wasm)$/.test(key)
      ? "no-cache, max-age=0, must-revalidate"
      : "public, max-age=3600");
    return secured(new Response(object.body, { headers }));
  },
};

function secured(response) {
  const headers = new Headers(response.headers);
  headers.set("Cross-Origin-Opener-Policy", "same-origin");
  headers.set("Cross-Origin-Embedder-Policy", "require-corp");
  headers.set("Cross-Origin-Resource-Policy", "same-origin");
  headers.set("X-Content-Type-Options", "nosniff");
  return new Response(response.body, { status: response.status, headers });
}
