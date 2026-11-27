// Installed by the physical tower runner only in profile mode, after scene startup.
// Use pass-boundary timestamps on the same device, encoder and release WASM as a timing run.
export function installTowerGpuProfile() {
  const profile = { frames: [], errors: [], pending: [] };
  window.towerGpuProfile = profile;
  const commands = new WeakMap();
  const createEncoder = GPUDevice.prototype.createCommandEncoder;
  const submit = GPUQueue.prototype.submit;
  GPUDevice.prototype.createCommandEncoder = function (descriptor) {
    const encoder = createEncoder.call(this, descriptor);
    const step = window.__physboxShowcase.fixedSteps + 1;
    if (
      descriptor?.label !== "physbox_showcase_frame" ||
      step < 80 ||
      step >= 150 ||
      step % 5 !== 0
    )
      return encoder;
    if (!this.features.has("timestamp-query"))
      throw Error("Device lacks timestamp-query");
    const labels = [];
    const count = 128;
    const size = count * 8;
    const querySet = this.createQuerySet({ type: "timestamp", count });
    const resolve = this.createBuffer({
      size,
      usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC,
    });
    const read = this.createBuffer({
      size,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    for (const method of ["beginComputePass", "beginRenderPass"]) {
      const begin = encoder[method].bind(encoder);
      encoder[method] = (pass) => {
        const label = pass?.label ?? "";
        if (
          pass?.timestampWrites ||
          !(label.startsWith("particle_") || method === "beginRenderPass")
        )
          return begin(pass);
        if (labels.length * 2 >= count)
          throw Error("Tower query set exhausted");
        const index = 2 * labels.length;
        labels.push(label);
        return begin({
          ...pass,
          timestampWrites: {
            querySet,
            beginningOfPassWriteIndex: index,
            endOfPassWriteIndex: index + 1,
          },
        });
      };
    }
    const finish = encoder.finish.bind(encoder);
    encoder.finish = (descriptor) => {
      if (labels.length) {
        encoder.resolveQuerySet(querySet, 0, 2 * labels.length, resolve, 0);
        encoder.copyBufferToBuffer(resolve, 0, read, 0, 16 * labels.length);
      }
      const buffer = finish(descriptor);
      commands.set(buffer, async () => {
        try {
          if (!labels.length) throw Error("No labeled passes");
          await read.mapAsync(GPUMapMode.READ);
          const ns = new BigUint64Array(read.getMappedRange());
          const passes = labels.map((label, i) => {
            const start = ns[i * 2],
              end = ns[i * 2 + 1];
            if (start === 0n || end < start)
              throw Error("Invalid timestamps for " + label);
            // Tile GPUs can begin several passes before earlier work finishes.
            // Keep both boundaries so adjacent completion times can be compared.
            return {
              label,
              startMs: Number(start - ns[0]) / 1e6,
              endMs: Number(end - ns[0]) / 1e6,
            };
          });
          profile.frames.push({ step, passes });
          read.unmap();
        } catch (error) {
          profile.errors.push(String(error));
        } finally {
          querySet.destroy();
          resolve.destroy();
          read.destroy();
        }
      });
      return buffer;
    };
    return encoder;
  };
  GPUQueue.prototype.submit = function (buffers) {
    submit.call(this, buffers);
    for (const buffer of buffers) {
      const collect = commands.get(buffer);
      if (collect) profile.pending.push(collect());
    }
  };
}
