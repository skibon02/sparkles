use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse_macro_input};


#[proc_macro]
pub fn id_map(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);
    let s = input.value();
    let id = calculate_id(&s) as u32;

    let expanded = quote! {
        #id
    };

    TokenStream::from(expanded)
}

fn calculate_id(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}