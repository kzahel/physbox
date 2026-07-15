import { readFile } from "node:fs/promises";
import { PNG } from "pngjs";

const [path, targetXText, targetYText] = process.argv.slice(2);
if (!path || !targetXText || !targetYText) {
  throw new Error("usage: require-grab-tether.mjs PNG TARGET_X TARGET_Y");
}
const image = PNG.sync.read(await readFile(path));
const targetX = Number(targetXText);
const targetY = Number(targetYText);
const endpoint = (red, green, blue) =>
  red > 120 && green > 180 && green > blue + 5 && green > red + 20;
const line = (red, green, blue) =>
  blue > 170 && green > 150 && blue > green + 5 && green > red + 40;

let endpointPixels = 0;
for (let y = Math.max(0, targetY - 16); y <= Math.min(image.height - 1, targetY + 16); y += 1) {
  for (let x = Math.max(0, targetX - 16); x <= Math.min(image.width - 1, targetX + 16); x += 1) {
    const offset = (y * image.width + x) * 4;
    if (endpoint(image.data[offset], image.data[offset + 1], image.data[offset + 2])) {
      endpointPixels += 1;
    }
  }
}

let linePixels = 0;
for (let y = Math.floor(image.height * 0.5); y <= targetY; y += 1) {
  for (let x = Math.floor(image.width * 0.55); x <= targetX; x += 1) {
    const offset = (y * image.width + x) * 4;
    if (line(image.data[offset], image.data[offset + 1], image.data[offset + 2])) {
      linePixels += 1;
    }
  }
}

if (endpointPixels < 20 || linePixels < 100) {
  throw new Error(
    `${path} is missing the active grab tether: endpoint_pixels=${endpointPixels} line_pixels=${linePixels}`,
  );
}
console.log(
  `PHYSBOX5_ANDROID_GRAB_TETHER_SUCCESS endpoint_pixels=${endpointPixels} line_pixels=${linePixels}`,
);
