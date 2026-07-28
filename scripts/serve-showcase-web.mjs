import { resolve } from "node:path";
import { startServer } from "./server.mjs";

const port = Number.parseInt(process.env.PORT ?? "8080", 10);
const host = process.env.HOST ?? "0.0.0.0";
const server = await startServer(
  resolve(import.meta.dirname, "..", "target", "showcase-web"),
  port,
  { crossOriginIsolation: false, host },
);
console.log(
  `PhysBox Web Showcase listening on ${host}:${server.address().port} ` +
    `(local URL: http://127.0.0.1:${server.address().port}/)`,
);
