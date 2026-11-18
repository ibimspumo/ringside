/** Builds stored spots (library/format.rs) for tests: a real header and a raw deflate body. */

export interface StoredOptions {
  catalog?: number;
  exploitability?: number;
  /** Bytes after the header and exploitability; by default a small deflated body. */
  tail?: Uint8Array;
}

export async function deflateRaw(bytes: Uint8Array<ArrayBuffer>): Promise<Uint8Array> {
  const stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream("deflate-raw"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

export async function storedSpot(scenario: string, flop: string, o: StoredOptions = {}): Promise<string> {
  const head = [..."RSLS"].map((c) => c.charCodeAt(0));
  const catalog = o.catalog ?? 1;
  head.push(1, catalog & 0xff, catalog >> 8, scenario.length, ...[...scenario + flop].map((c) => c.charCodeAt(0)));
  const nums = new DataView(new ArrayBuffer(8));
  nums.setUint32(0, 300, true);
  nums.setFloat32(4, o.exploitability ?? 0.5, true);
  const tail = o.tail ?? await deflateRaw(new Uint8Array([1, 0, 0, 0, 0, 0, 0, 0]));
  const bytes = new Uint8Array([...head, ...new Uint8Array(nums.buffer), ...tail]);
  return btoa(String.fromCharCode(...bytes));
}
