// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

//! Derives for `Canon` trait for rust types

#![deny(missing_docs)]

use proc_macro2::{Ident, Literal};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam,
    Generics,
};

const FIELD_NAMES: [&str; 16] = [
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o",
    "p",
];

fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(canonical::Canon));
        }
    }
    generics
}

#[proc_macro_derive(Canon)]
/// Derive macro that implements the serialization method for a type
pub fn canon_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident.clone();

    let generics = add_trait_bounds(input.generics.clone());

    let (_, ty_generics, where_clause) = generics.split_for_impl();

    let (decode, encode, length) = match input.data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => {
                let decode = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    let ty = &f.ty;
                    quote_spanned! { f.span() =>
                                     #name : <#ty>::decode(source)?,
                    }
                });

                let encode = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! { f.span() =>
                                     canonical::Canon::encode(&self . #name, sink);
                    }
                });

                let length = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! { f.span() =>
                                     + canonical::Canon::encoded_len(& self.#name)
                    }
                });

                (
                    quote! { Ok(#name { #( #decode )* } )},
                    quote! { #( #encode )* },
                    quote! { #( #length )* },
                )
            }
            Fields::Unnamed(ref fields) => {
                let decode = fields.unnamed.iter().map(|f| {
                    let ty = &f.ty;
                    quote_spanned! { f.span() =>
                                     <#ty>::decode(source)?,
                    }
                });

                let encode = fields.unnamed.iter().enumerate().map(|(i, f)| {
                    let i = Literal::usize_unsuffixed(i);
                    quote_spanned! { f.span() =>
                                     canonical::Canon::encode(&self . #i, sink);
                    }
                });

                let length = fields.unnamed.iter().enumerate().map(|(i, f)| {
                    let i = Literal::usize_unsuffixed(i);
                    quote_spanned! { f.span() =>
                                     + canonical::Canon::encoded_len(& self.#i)
                    }
                });

                (
                    quote! { Ok(#name ( #( #decode )* ) )},
                    quote! { #( #encode )* },
                    quote! { #( #length )* },
                )
            }
            Fields::Unit => {
                (quote! { Ok(Self) }, quote! { () }, quote! { + 0 })
            }
        },
        Data::Enum(ref data) => {
            if data.variants.len() > 256 {
                unimplemented!(
                    "More than 256 enum variants is not supported at the time."
                )
            }

            let mut decodes = vec![];
            let mut encodes = vec![];
            let mut lengths = vec![];

            for (i, v) in data.variants.iter().enumerate() {
                let tag = Literal::u8_suffixed(i as u8);
                let ident = &v.ident;

                match v.fields {
                    Fields::Unit => {
                        decodes.push(quote! { #tag => Ok( #name :: #ident ), });
                        encodes.push(
                            quote! { #name :: #ident => Canon::encode(& #tag, sink), },
                        );
                        lengths.push(quote! { #name :: #ident => 1, });
                    }
                    Fields::Unnamed(ref fields) => {
                        let fields_decode = fields.unnamed.iter().map(|f| {
                            let ty = &f.ty;
                            quote_spanned! { f.span() =>
                                             <#ty>::decode(source)?
                            }
                        });
                        let fields_bind =
                            fields.unnamed.iter().enumerate().map(|(i, f)| {
                                let ident =
                                    Ident::new(FIELD_NAMES[i], f.span());
                                quote_spanned! { f.span() => #ident }
                            });

                        let fields_assign = fields.unnamed.iter().enumerate().map(|(i, f)| {
                            let ident = Ident::new(FIELD_NAMES[i], f.span());
                            quote_spanned! { f.span() => Canon::encode(#ident, sink); }
                        });

                        let fields_lengths = fields.unnamed.iter().enumerate().map(|(i, f)| {
                            let ident = Ident::new(FIELD_NAMES[i], f.span());
                            quote_spanned! { f.span() => + Canon::encoded_len(#ident)}
                        });

                        let fields_bind2 = fields_bind.clone();

                        decodes.push(
                            quote! { #tag => Ok( #name :: #ident ( #( #fields_decode ),* ) ) , },
                        );

                        encodes.push(quote! { #name :: #ident ( #( #fields_bind ),* ) =>
                                              { Canon::encode(& #tag, sink); #( #fields_assign )* } });

                        lengths.push(quote! { #name :: #ident ( #( #fields_bind2 ),* ) => {
                            1 #( #fields_lengths )*
                        },
                        });
                    }
                    Fields::Named(ref fields) => {
                        let fields_decode = fields.named.iter().map(|f| {
                            let ty = &f.ty;
                            let ident = &f.ident;
                            quote_spanned! { f.span() =>
                                             #ident : <#ty>::decode(source)?
                            }
                        });
                        let fields_bind = fields.named.iter().map(|f| {
                            let ident = &f.ident;
                            quote_spanned! { f.span() => #ident }
                        });

                        let fields_assign = fields.named.iter().map(|f| {
                            let ident = &f.ident;
                            quote_spanned! { f.span() => Canon::encode(#ident, sink); }
                        });

                        let fields_lengths = fields.named.iter().map(|f| {
                            let ident = &f.ident;
                            quote_spanned! { f.span() => + Canon::encoded_len(#ident) }
                        });

                        let fields_bind2 = fields_bind.clone();

                        decodes.push(
                            quote! { #tag => Ok( #name :: #ident { #( #fields_decode ),* } ) , },
                        );

                        encodes.push(quote! { #name :: #ident { #( #fields_bind ),* } =>
                                              { Canon::encode(& #tag, sink); #( #fields_assign )* } });

                        lengths.push(quote! { #name :: #ident { #( #fields_bind2 ),* } => {
                            1 #( #fields_lengths )*
                        },
                        });
                    }
                }
            }

            (
                quote! {
                    let tag = u8::decode(source)?;
                    match & tag {
                        #( #decodes )*
                        _ => Err(canonical::CanonError::InvalidEncoding)
                    }
                },
                quote! {
                    match self {
                        #( #encodes )*
                    }
                },
                quote! {
                    + match & self {
                        #( #lengths )*
                    }
                },
            )
        }
        Data::Union(_) => unimplemented!("Union types are not derivable"),
    };

    let output = quote! {
        impl #generics canonical::Canon for #name #ty_generics #where_clause {
            fn encode(&self, sink: &mut canonical::Sink) {
                #encode
                ;
            }

            fn decode(source: &mut canonical::Source)
                    -> Result<Self, canonical::CanonError> {
                #decode
            }

            fn encoded_len(&self) -> usize {
                0 #length
            }
        }
    };

    proc_macro::TokenStream::from(output)
}
