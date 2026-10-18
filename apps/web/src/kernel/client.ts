import { createStubKernel } from "./stub";
import type { PixelKernel } from "./types";

/**
 * Kernel entry used by the shell.
 *
 * Pixels on screen come from `snapshot()`. Paint and erase are the only
 * writes. Today this returns an in-memory stand-in whose checks match
 * `pixelforge-kernel`. The planned replacement is a `pixelforge-kernel-wasm`
 * crate (wasm-bindgen) that wraps the same Rust `Document`. Swap the body
 * of this function when that crate exists; leave the view on `snapshot()`.
 */
export function createKernel(): PixelKernel {
  return createStubKernel();
}
