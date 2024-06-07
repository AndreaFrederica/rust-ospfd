use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, ToTokens};
use std::collections::HashMap;
use syn::{parse::ParseStream, Result};

pub struct DefineAttr {
    pub map: HashMap<String, TokenStream>,
}

impl DefineAttr {
    pub fn parse(input: ParseStream) -> Result<Self> {
        let mut map = HashMap::new();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=>]>()?;
            let value: syn::Expr = input.parse()?;
            map.insert(key.to_string(), value.to_token_stream());
            let d = input.parse::<syn::Token![;]>();
            if d.is_err() && !input.is_empty() {
                return Err(d.err().unwrap());
            }
        }
        Ok(DefineAttr { map })
    }

    pub fn process_define(&self, input: TokenStream) -> TokenStream {
        input
            .into_iter()
            .map(|tt| match tt {
                TokenTree::Ident(ident) => {
                    if let Some(replace) = self.map.get(&ident.to_string()) {
                        replace.clone()
                    } else {
                        quote!(#ident)
                    }
                }
                TokenTree::Group(group) => {
                    let delim = group.delimiter();
                    let stream = self.process_define(group.stream());
                    let group = proc_macro2::Group::new(delim, stream);
                    quote!(#group)
                }
                tt => quote!(#tt),
            })
            .collect()
    }
}
