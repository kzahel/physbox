import assert from "node:assert/strict";
import test from "node:test";
import { webGpuUnavailableMessage } from "../showcase-web/startup-diagnostics.js";

test("diagnoses insecure HTTP before browser WebGPU support", () => {
  const message = webGpuUnavailableMessage({
    isSecureContext: false,
    protocol: "http:",
    host: "100.76.216.59:8080",
  });

  assert.match(message, /WebGPU requires a secure context/);
  assert.match(message, /http:\/\/100\.76\.216\.59:8080/);
  assert.match(message, /likely reason WebGPU is unavailable/);
  assert.match(message, /HTTPS/);
});

test("retains a browser-support diagnosis for secure origins", () => {
  assert.equal(
    webGpuUnavailableMessage({
      isSecureContext: true,
      protocol: "https:",
      host: "example.test",
    }),
    "WebGPU is not available or enabled in this browser.",
  );
});
