use core::panic;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use regex::Regex;
use syn::{DataStruct, Fields, Ident, Meta};

enum BitSize {
    Data(u32),
    Zero(u32),
    Vec(String, String),
    Ipv4,
}

fn get_field_tuples(s: &DataStruct) -> impl Iterator<Item = (&Ident, BitSize)> {
    let fields = match &s.fields {
        Fields::Named(ref fields) => &fields.named,
        _ => panic!("Only named fields are supported"),
    };
    fields.iter().map(|f| {
        let ident = f.ident.as_ref().unwrap();
        let ty = f.ty.to_token_stream().to_string();
        if f.vis.to_token_stream().to_string() == "" {
            panic!("All fields must be public")
        }
        let attr = f
            .attrs
            .iter()
            .find_map(|attr| {
                if attr.meta.path().is_ident("size") {
                    match &attr.meta {
                        Meta::List(list) => Some(
                            list.parse_args::<syn::Expr>()
                                .unwrap()
                                .to_token_stream()
                                .to_string(),
                        ),
                        _ => panic!(
                            "size attribute should be a list, no {:?}",
                            attr.meta.to_token_stream()
                        ),
                    }
                } else {
                    None
                }
            })
            .unwrap_or(String::new());

        let zero_r = Regex::new(r"PhantomData\s*:?:?\s*<\s*[ui](\d+)\s*>").unwrap();
        if let Some(caps) = zero_r.captures(&ty) {
            return (ident, BitSize::Zero(caps[1].parse().expect(&ty)));
        }
        let data_r = Regex::new(r"^[ui](\d+)").unwrap();
        if let Some(caps) = data_r.captures(&ty) {
            return (ident, BitSize::Data(caps[1].parse().expect(&ty)));
        }
        let vec_r = Regex::new(r"Vec\s*:?:?\s*<\s*(\w+)\s*>").unwrap();
        if let Some(caps) = vec_r.captures(&ty) {
            return (ident, BitSize::Vec(caps[1].to_string(), attr));
        }
        if ty.contains("Ipv4Addr") {
            return (ident, BitSize::Ipv4);
        }
        panic!("Unsupported type {}", ty);
    })
}

pub fn generate_to_bytes(s: &DataStruct, name: &Ident) -> TokenStream {
    let fields = get_field_tuples(s).map(|(ident, size)| match size {
        BitSize::Data(n) => quote! { buf.put_un(self.#ident, #n); },
        BitSize::Zero(n) => quote! { buf.put_un(0u32, #n); },
        BitSize::Vec(..) => quote! { buf.extend_from_slice(&self.#ident.to_bytes()); },
        BitSize::Ipv4 => quote! { buf.extend_from_slice(self.#ident.octets().as_slice()); },
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
    let raw_fields: Vec<_> = get_field_tuples(s).collect();
    let decls = raw_fields.iter().map(|(ident, size)| match size {
        BitSize::Data(n) => quote! { #ident: buf.get_un(#n) },
        BitSize::Zero(n) => quote! { #ident: { let _ = buf.get_un::<u32>(#n); PhantomData } },
        BitSize::Vec(..) => quote! { #ident: vec![] },
        BitSize::Ipv4 => quote! { #ident: ::std::net::Ipv4Addr::from_buf(buf.get_buf()) },
    });
    let assigns = raw_fields.iter().map(|(ident, size)| {
        if let BitSize::Vec(ty, size) = size {
            let ty = Ident::new(&ty, ident.span());
            if size.is_empty() {
                quote! {{s.#ident = {
                    let mut vec = vec![];
                    let buf = buf.get_buf();
                    while buf.has_remaining() {
                        vec.push(#ty::from_buf(buf));
                    }
                    vec
                }}}
            } else {
                let sz = Ident::new(&size, ident.span());
                quote! {{s.#ident = {
                    let mut vec = vec![];
                    let buf = buf.get_buf();
                    for _ in 0..s.#sz {
                        vec.push(#ty::from_buf(buf));
                    }
                    vec
                }}}
            }
        } else {
            quote! {}
        }
    });
    quote! {
        impl FromBuf for #name {
            fn from_buf(buf: &mut impl ::bytes::Buf) -> Self {
                let mut buf = Bits::from(buf);
                let mut s = Self { #(#decls,)* };
                #(#assigns)*
                s
            }
        }
    }
}

pub fn generate_default(s: &DataStruct, name: &Ident) -> TokenStream {
    let fields = get_field_tuples(s).map(|(ident, size)| match size {
        BitSize::Data(_) => quote! { #ident: 0 },
        BitSize::Zero(_) => quote! { #ident: PhantomData },
        BitSize::Vec(..) => quote! { #ident: Vec::new() },
        BitSize::Ipv4 => quote! { #ident: Ipv4Addr::UNSPECIFIED },
    });
    quote! {
        impl Default for #name {
            fn default() -> Self {
                Self { #(#fields,)* }
            }
        }
    }
}
