import createBox2dComparator from "./box2d-comparator.mjs";

export function loadBox2dComparator() {
  return createBox2dComparator({
    locateFile(path) {
      return new URL(path, import.meta.url).href;
    },
  });
}
