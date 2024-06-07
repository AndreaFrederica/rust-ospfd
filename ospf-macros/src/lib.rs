mod decorator;
mod define;

use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(ToBytesMut)]
pub fn derive_to_bytes(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let s = match &ast.data {
        syn::Data::Struct(ref s) => decorator::generate_to_bytes(s, name),
        _ => panic!("Only structs are supported"),
    };
    s.into()
}

#[proc_macro_derive(FromBuf, attributes(size))]
pub fn derive_from_bytes(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let s = match &ast.data {
        syn::Data::Struct(ref s) => decorator::generate_from_buf(s, name),
        _ => panic!("Only structs are supported"),
    };
    s.into()
}

#[proc_macro_attribute]
pub fn raw_packet(_attrs: TokenStream, code: TokenStream) -> TokenStream {
    let input = parse_macro_input!(code as DeriveInput);
    // enhancement: if input already has Clone and/or Debug, do not add them
    let s = quote! {
        #[derive(::ospf_macros::ToBytesMut, ::ospf_macros::FromBuf, Clone, Debug)]
        #input
    };
    s.into()
}

#[proc_macro_attribute]
pub fn define(attrs: TokenStream, code: TokenStream) -> TokenStream {
    let attrs = parse_macro_input!(attrs with define::DefineAttr::parse);
    attrs.process_define(code.into()).into()
}
