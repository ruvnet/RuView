//! RUSTSEC-2026-0569: a re-exported host function has no calling Wasm module.
#![cfg(feature = "wasmtime")]

#[test]
fn reexported_host_function_has_no_calling_module() {
    use wasmtime::{Caller, Engine, Linker, Module, Store};
    let engine = Engine::default();
    let module = Module::new(&engine, "(module (import \"host\" \"inspect\" (func)) (memory (export \"memory\") 1) (export \"run\" (func 0)))").unwrap();
    let memory = module.get_export_index("memory").unwrap();
    let mut linker = Linker::new(&engine);
    linker
        .func_wrap("host", "inspect", move |mut caller: Caller<'_, ()>| {
            assert!(caller.get_module_export(&memory).is_none());
        })
        .unwrap();
    let mut store = Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &module).unwrap();
    instance
        .get_typed_func::<(), ()>(&mut store, "run")
        .unwrap()
        .call(&mut store, ())
        .unwrap();
}
