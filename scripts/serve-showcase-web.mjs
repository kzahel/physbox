import { resolve } from "node:path";
import { startServer } from "./server.mjs";

const port = Number.parseInt(process.env.PORT ?? "8080", 10);
const server = await startServer(
  resolve(import.meta.dirname, "..", "target", "showcase-web"),
  port,
  { crossOriginIsolation: false },
);
console.log(`PhysBox Web Showcase served at http://127.0.0.1:${server.address().port}`);
