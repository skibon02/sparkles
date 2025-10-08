use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse_macro_input, Expr};
use syn::parse::{Parse, ParseStream};
use syn::token::Comma;

#[cfg(feature = "included-from-sparkles")]
fn repr_name() -> impl quote::ToTokens {
    quote! {
        sparkles::core::StaticNameRepr
    }
}

#[cfg(not(feature="included-from-sparkles"))]
fn repr_name() -> impl quote::ToTokens {
    quote! {
        StaticNameRepr
    }
}

/// Create instant event with given name
/// # Example
/// ```rust
/// sparkles::instant_event!("Packet received");
/// ```
#[proc_macro]
pub fn instant_event(input: TokenStream) -> TokenStream {
    let name = parse_macro_input!(input as LitStr);
    let s = name.value();
    let hash = calculate_hash(&s);

    let repr_name = repr_name();

    let expanded = quote! {
        sparkles::instant_event(unsafe {#repr_name::from_str_and_hash(#name, #hash)})
    };

    TokenStream::from(expanded)
}

/// Create range event with given name
/// 
/// Can be finished with two options:
/// 1. Drop the guard
/// 2. Call `sparkles_macro::range_event_end!(guard, "name")`
/// 
/// # Example
/// ```rust
/// let packet_proc = sparkles::range_event_start!("Packet parsing");
/// // Do some work
/// let Ok(data) = parse_packet(&packet) else {
///    sparkles::range_event_end!(packet_proc, "Failed");
/// };
/// sparkles::range_event_end!(packet_proc, "OK");
/// ```
#[proc_macro]
pub fn range_event_start(input: TokenStream) -> TokenStream {
    let name = parse_macro_input!(input as LitStr);
    let s = name.value();
    let hash = calculate_hash(&s);

    let repr_name = repr_name();

    let expanded = quote! {
        sparkles::range_event_start(unsafe {#repr_name::from_str_and_hash(#name, #hash)})
    };

    TokenStream::from(expanded)
}

struct RangeEventStartInput {
    guard: Expr,
    _comma: Comma,
    name: LitStr,
}

impl Parse for RangeEventStartInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            guard: input.parse()?,
            _comma: input.parse()?,
            name: input.parse()?,
        })
    }
}

/// Finish range event with given name
/// If you don't want to assign name to the event end, simply drop the guard.
///
/// # Example
/// ```rust
/// let packet_proc = sparkles::range_event_start!("Packet parsing");
/// // Do some work
/// let Ok(data) = parse_packet(&packet) else {
///    sparkles::range_event_end!(packet_proc, "Failed");
/// };
/// sparkles::range_event_end!(packet_proc, "OK");
/// ```
#[proc_macro]
pub fn range_event_end(input: TokenStream) -> TokenStream {
    let RangeEventStartInput{guard, name, ..} = parse_macro_input!(input as RangeEventStartInput);
    let s = name.value();
    let hash = calculate_hash(&s);

    let repr_name = repr_name();

    let expanded = quote! {
        #guard.end(unsafe {#repr_name::from_str_and_hash(#name, #hash)})
    };

    TokenStream::from(expanded)
}

fn calculate_hash(s: &str) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish() as u32
}

#[proc_macro]
pub fn static_name(input: TokenStream) -> TokenStream {
    let name = parse_macro_input!(input as LitStr);
    let s = name.value();
    let hash = calculate_hash(&s);

    let repr_name = repr_name();

    let expanded = quote! {
        unsafe {#repr_name::from_str_and_hash(#name, #hash)}
    };

    TokenStream::from(expanded)
}