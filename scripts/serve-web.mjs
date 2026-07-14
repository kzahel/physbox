import { resolve } from "node:path";
import { startServer } from "./server.mjs";

const port = Number.parseInt(process.env.PORT ?? "8080", 10);
const server = await startServer(resolve(import.meta.dirname, "..", "target", "web"), port);
console.log(`PhysBox 5 web build served at http://127.0.0.1:${server.address().port}`);
