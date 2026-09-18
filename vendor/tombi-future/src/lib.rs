#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
mod on_native;
#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
pub use on_native::*;

#[cfg(any(target_arch = "wasm32", not(feature = "native")))]
mod on_wasm;
#[cfg(any(target_arch = "wasm32", not(feature = "native")))]
pub use on_wasm::*;
