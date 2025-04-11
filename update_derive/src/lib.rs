use proc_macro::TokenStream;
use quote::quote;
use syn::Data;

#[proc_macro_derive(Update)]
pub fn update_macro_derive(item: TokenStream) -> TokenStream {
    let ast = syn::parse(item).unwrap();
    impl_update_macro(&ast)
}

fn impl_update_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    assert!(matches!(ast.data, Data::Struct(_)));
    let Data::Struct(s) = &ast.data else {
        unreachable!()
    };
    let update_fields = s.fields.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        quote! {
            if let Some(other) = other.#name {
                let _ = self.#name.replace(other);
            }
        }
    });
    let remove_fields = s.fields.iter().map(|field| {
        let name = field.ident.as_ref().unwrap();
        let name_string = name.to_string();
        quote! {
            #name_string => { let _ = self.#name.take(); }
        }
    });
    quote! {
        impl update::Update for #name {
            fn update(&mut self, other: Self) {
                #( #update_fields )*
            }
            fn remove<T: AsRef<str>>(&mut self, properties_name: &[T]) {
                for name in properties_name {
                    match name.as_ref() {
                        #( #remove_fields )*
                        _ => (),
                    }
                }
            }
        }
    }
    .into()
}
