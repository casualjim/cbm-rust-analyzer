macro_rules! define_macro_generated_receiver {
    () => {
        pub struct MacroGeneratedReceiver;

        impl MacroGeneratedReceiver {
            pub fn macro_generated_cross_file_target(&self) -> i32 {
                43
            }
        }
    };
}

define_macro_generated_receiver!();
