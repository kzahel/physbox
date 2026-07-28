export function webGpuUnavailableMessage({ isSecureContext, protocol, host }) {
  if (!isSecureContext) {
    const origin = protocol && host ? `${protocol}//${host}` : "This page";
    return (
      `WebGPU requires a secure context. ${origin} was loaded over insecure HTTP, ` +
      "which is the likely reason WebGPU is unavailable. Open the showcase over HTTPS, " +
      "or use http://localhost on the machine running the server."
    );
  }
  return "WebGPU is not available or enabled in this browser.";
}
