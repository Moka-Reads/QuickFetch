extern crate proc_macro;

use proc_macro::TokenStream;

use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(QFEntry, attributes(mod_eq, mod_neq, url, name, version))]
pub fn entry_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_entry_derive(&ast)
}

fn impl_entry_derive(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;

    // Extract fields for mod_eq
    let mod_eq_fields = match &ast.data {
        syn::Data::Struct(data_struct) => data_struct.fields.iter().filter_map(|field| {
            if field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("mod_eq"))
            {
                Some(field.ident.as_ref().unwrap().clone())
            } else {
                None
            }
        }),
        _ => {
            eprintln!("QFEntry only supports structs");
            return TokenStream::new();
        }
    };

    // Extract fields for mod_neq
    let mod_neq_fields = match &ast.data {
        syn::Data::Struct(data_struct) => data_struct.fields.iter().filter_map(|field| {
            if field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("mod_neq"))
            {
                Some(field.ident.as_ref().unwrap().clone())
            } else {
                None
            }
        }),
        _ => {
            eprintln!("QFEntry only supports structs");
            return TokenStream::new();
        }
    };

    // Extract the url field
    let url_field = match &ast.data {
        syn::Data::Struct(data_struct) => data_struct.fields.iter().find_map(|field| {
            if field.attrs.iter().any(|attr| attr.path().is_ident("url")) {
                Some(field.ident.as_ref().unwrap().clone())
            } else {
                None
            }
        }),
        _ => {
            eprintln!("QFEntry only supports structs");
            return TokenStream::new();
        }
    };

    let url_field = match url_field {
        Some(field) => field,
        None => {
            eprintln!("No field marked with #[url] attribute");
            return TokenStream::new();
        }
    };

    // Extract the name field
    let name_field = match &ast.data {
        syn::Data::Struct(data_struct) => data_struct.fields.iter().find_map(|field| {
            if field.attrs.iter().any(|attr| attr.path().is_ident("name")) {
                Some(field.ident.as_ref().unwrap().clone())
            } else {
                None
            }
        }),
        _ => {
            eprintln!("QFEntry only supports structs");
            return TokenStream::new();
        }
    };

    let name_field = match name_field {
        Some(field) => field,
        None => {
            eprintln!("No field marked with #[name] attribute");
            return TokenStream::new();
        }
    };

    // Extract the version field
    let version_field = match &ast.data {
        syn::Data::Struct(data_struct) => data_struct.fields.iter().find_map(|field| {
            if field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("version"))
            {
                Some(field.ident.as_ref().unwrap().clone())
            } else {
                None
            }
        }),
        _ => {
            eprintln!("QFEntry only supports structs");
            return TokenStream::new();
        }
    };

    let version_field = match version_field {
        Some(field) => field,
        None => {
            eprintln!("No field marked with #[version] attribute");
            return TokenStream::new();
        }
    };

    let gen = quote! {
        impl Entry for #name {
            fn from_ivec(value: IVec) -> Self {
                bincode::deserialize(&value).expect("Failed to deserialize value")
            }

            fn entry_bytes(&self) -> Vec<u8> {
                bincode::serialize(self).expect("Failed to serialize value")
            }

            fn url(&self) -> String {
                self.#url_field.to_string()
            }

            fn log_cache(&self) {
                info!("{} [{}] (cached)", self.#name_field, self.#version_field)
            }

            fn log_caching(&self) {
                info!("{} [{}] caching", self.#name_field, self.#version_field)
            }

            fn is_modified(
                &self,
                keys_iter: impl DoubleEndedIterator<Item = Result<IVec, sled::Error>>,
            ) -> Option<IVec> {
                for key_result in keys_iter {
            let key = match key_result {
                Ok(key) => key,
                Err(_) => continue, // Skip to next iteration on error
            };

            let pkg = Self::from_ivec(key.clone());

            // Check fields marked with #[mod_eq]
            if !( #( self.#mod_eq_fields == pkg.#mod_eq_fields )&&* ) {
                return Some(key);
            }

            // Check fields marked with #[mod_neq]
            if !( #( self.#mod_neq_fields != pkg.#mod_neq_fields )&&* ) {
                return Some(key);
            }
        }

        None
    }
        }
    };

    gen.into()
}
