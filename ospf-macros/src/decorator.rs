use core::panic;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use regex::Regex;
use syn::{DataStruct, Fields, Ident};

enum BitSize {
    Data(u32),
    Zero(u32),
    Other(syn::Type),
}

fn get_field_tuples(s: &DataStruct) -> impl Iterator<Item = (&Ident, BitSize)> {
    let fields = match &s.fields {
        Fields::Named(ref fields) => &fields.named,
        _ => panic!("Only named fields are supported"),
    };
    fields.iter().map(|f| {
        let ident = f.ident.as_ref().unwrap();
        let ty = f.ty.to_token_stream().to_string();
        let zero_r = Regex::new(r"PhantomData\s*:?:?\s*<\s*[ui](\d+)\s*>").unwrap();
        if let Some(caps) = zero_r.captures(&ty) {
            return (ident, BitSize::Zero(caps[1].parse().expect(&ty)));
        }
        let data_r = Regex::new(r"^[ui](\d+)").unwrap();
        if let Some(caps) = data_r.captures(&ty) {
            return (ident, BitSize::Data(caps[1].parse().expect(&ty)));
        }
        (ident, BitSize::Other(f.ty.clone()))
    })
}

pub fn generate_to_bytes(s: &DataStruct, name: &Ident) -> TokenStream {
    let fields = get_field_tuples(s).map(|(ident, size)| match size {
        BitSize::Data(n) => quote! { buf.put_un(self.#ident, #n); },
        BitSize::Zero(n) => quote! { buf.put_un(0u32, #n); },
        BitSize::Other(_) => quote! { buf.extend_from_slice(&self.#ident.to_bytes()); },
    });
    quote! {
        impl ToBytesMut for #name {
            fn to_bytes_mut(&self) -> ::bytes::BytesMut {
                let mut buf = BitsMut::new();
                #(#fields)*;
                buf.into()
            }
        }
    }
}

pub fn generate_from_buf(s: &DataStruct, name: &Ident) -> TokenStream {
    let fields = get_field_tuples(s).map(|(ident, size)| match size {
        BitSize::Data(n) => quote! { #ident: buf.get_un(#n), },
        BitSize::Zero(_) => quote! { #ident: PhantomData, },
        BitSize::Other(ty) => quote! { #ident: #ty::from_buf(buf.get_buf()), },
    });
    quote! {
        impl FromBuf for #name {
            fn from_buf(buf: &mut impl ::bytes::Buf) -> Self {
                let mut buf = Bits::from(buf);
                Self { #(#fields)* }
            }
        }
    }
}
