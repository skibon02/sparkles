use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse_macro_input};

#[proc_macro]
pub fn instant_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);
    let s = input.value();
    let id = get_hash(&s) as u32;

    let expanded = quote! {
        sparkles::instant_event(#id, #s)
    };

    TokenStream::from(expanded)
}

#[proc_macro]
pub fn range_event_start(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);
    let s = input.value();
    let id = get_hash(&s) as u32;

    let expanded = quote! {
        sparkles::range_event_start(#id, #s)
    };

    TokenStream::from(expanded)
}

fn get_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}