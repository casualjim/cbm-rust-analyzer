use fixture_cross_crate::cross_crate_target;
use fixture_derive::ProcMacroTarget;
use std::ops::Deref;

pub mod macro_generated_method_mod;
pub mod macro_target_mod;

pub fn direct_target(input: i32) -> i32 {
    input + 1
}

pub struct Calculator;

impl Calculator {
    pub fn method_target(&self, input: i32) -> i32 {
        input * 2
    }
}

pub struct AssociatedTarget;

impl AssociatedTarget {
    pub fn associated_target(input: i32) -> i32 {
        input + 3
    }
}

pub trait Greeter {
    fn trait_target(&self) -> &'static str;
}

pub struct FriendlyGreeter;

impl Greeter for FriendlyGreeter {
    fn trait_target(&self) -> &'static str {
        "hello"
    }
}

pub fn generic_target<T: Clone>(value: T) -> T {
    value.clone()
}

pub struct DerefInner;

impl DerefInner {
    pub fn deref_target(&self, input: i32) -> i32 {
        input + 4
    }
}

pub struct DerefWrapper(DerefInner);

impl Deref for DerefWrapper {
    type Target = DerefInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn async_target(input: i32) -> i32 {
    input + 5
}

#[cfg(feature = "cfg-fixture")]
pub fn cfg_target(input: i32) -> i32 {
    input + 6
}

macro_rules! define_macro_target {
    () => {
        pub fn macro_target(input: i32) -> i32 {
            input - 1
        }
    };
}

define_macro_target!();

macro_rules! call_cross_file_method {
    () => {{
        let receiver = crate::macro_target_mod::MacroReceiver;
        receiver.macro_cross_file_target()
    }};
}

pub mod out_dir_generated {
    include!(concat!(env!("OUT_DIR"), "/generated_include.rs"));
}

pub fn out_dir_workspace_target(input: i32) -> i32 {
    input + 31
}

#[derive(ProcMacroTarget)]
pub struct GeneratedByDerive;

pub fn exercise_all_cases() -> (
    i32,
    i32,
    &'static str,
    u32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
    i32,
) {
    let calculator = Calculator;
    let greeter = FriendlyGreeter;
    let wrapper = DerefWrapper(DerefInner);
    let generated = GeneratedByDerive;
    let macro_generated_receiver = crate::macro_generated_method_mod::MacroGeneratedReceiver;

    let direct = direct_target(10);
    let method = calculator.method_target(11);
    let trait_value = greeter.trait_target();
    let generic = generic_target::<u32>(12);
    let macro_value = macro_target(13);
    let macro_expanded_call = call_cross_file_method!();
    let macro_generated_method = macro_generated_receiver.macro_generated_cross_file_target();
    let associated = AssociatedTarget::associated_target(14);
    let deref_value = wrapper.deref_target(15);
    let cfg_value = cfg_target(16);
    let cross_crate = cross_crate_target(17);
    let proc_macro = generated.proc_macro_target();

    (
        direct,
        method,
        trait_value,
        generic,
        macro_value,
        macro_expanded_call,
        macro_generated_method,
        associated,
        deref_value,
        cfg_value,
        cross_crate,
        proc_macro,
    )
}

pub async fn exercise_async_case() -> i32 {
    async_target(18).await
}

pub fn exercise_out_dir_include_case() -> (i32, i32) {
    let generated = out_dir_generated::generated_out_dir_target(30);
    let workspace = out_dir_generated::generated_calls_workspace(31);
    (generated, workspace)
}
