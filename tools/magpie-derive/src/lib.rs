extern crate proc_macro;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, Generics};

#[proc_macro_derive(RavenExtern)]
pub fn my_macro_here_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    // Used in the quasi-quotation below as `#name`.
    let name = input.ident;
    let raven_name = Ident::new(&*(name.to_string() + "_RavenType"), Span::call_site());

    // Add a bound `T: RavenExtern` to every type parameter T.
    let generics = add_trait_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let translated = translate(&input.data);
    let translated_extern = translate_extern(&input.data);

    let expanded = quote! {
        #[repr(C, align(8))]
        #[derive(Debug)]
        pub struct #raven_name {
            type_id: core::ffi::c_int,
            #translated_extern
        }

        // The generated impl.
        impl #impl_generics data::RavenExtern for #name #ty_generics #where_clause {
            type Input = #raven_name;
            unsafe fn translate(raven_type: *mut #raven_name) -> Self {
                let raven_type = std::ptr::read(raven_type);
                return Self {
                    #translated
                }
            }
        }
    };

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}

// Add a bound `T: RavenExtern` to every type parameter T.
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(magpie - derive::RavenExtern));
        }
    }
    generics
}

// Generate an expression to translate field.
fn translate(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span()=>
                        #name: data::RavenExtern::translate(std::mem::transmute(raven_type.#name.load(std::sync::atomic::Ordering::Relaxed)))
                    }
                });
                quote! {
                    #(#recurse),*
                }
            }
            _ => unimplemented!(),
        },
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}

fn translate_extern(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span()=>
                        #name: std::sync::atomic::AtomicPtr<()>
                    }
                });
                quote! {
                    #(#recurse),*
                }
            }
            _ => unimplemented!(),
        },
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}
