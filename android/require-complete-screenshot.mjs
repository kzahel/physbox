import { readFile } from "node:fs/promises";
import { PNG } from "pngjs";

const [path] = process.argv.slice(2);
if (!path) throw new Error("usage: require-complete-screenshot.mjs PNG");
const image = PNG.sync.read(await readFile(path));
let black = 0;
for (let offset = 0; offset < image.data.length; offset += 4) {
  if (image.data[offset] <= 3 && image.data[offset + 1] <= 3 && image.data[offset + 2] <= 3) {
    black += 1;
  }
}
const fraction = black / (image.width * image.height);
if (fraction > 0.01) {
  throw new Error(`${path} has ${(fraction * 100).toFixed(2)}% unexpected black pixels`);
}
console.log(`PHYSBOX5_ANDROID_SCREENSHOT_SUCCESS width=${image.width} height=${image.height} black_fraction=${fraction}`);
