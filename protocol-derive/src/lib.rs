extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, ExprLit, Fields, Index, Lit, Path,
};

#[proc_macro_derive(ToBytes, attributes(discriminant_as))]
pub fn derive_tobytes(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let implementation = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let field_names = fields.named.iter().map(|f| &f.ident);

                quote! {
                    let mut written = 0;
                    #(written += ToBytes::write_to(&self.#field_names, write)?; )*
                    Ok(written)
                }
            }
            Fields::Unnamed(fields) => {
                let field_indices = (0..fields.unnamed.len()).map(|i| Index::from(i));

                quote! {
                    let mut written = 0;
                    #(written += ToBytes::write_to(&self.#field_indices, write)?; )*
                    Ok(written)
                }
            }
            Fields::Unit => quote! {
                Ok(0)
            },
        },
        Data::Enum(data) => {
            let discriminant_type = get_discriminant(input.attrs);

            let mut match_arms = Vec::new();

            let mut next_discriminant = 0;

            for variant in data.variants {
                let variant_name = variant.ident;

                if let Some((
                    _,
                    Expr::Lit(ExprLit {
                        lit: Lit::Int(d), ..
                    }),
                )) = variant.discriminant
                {
                    next_discriminant = d.base10_digits().parse().unwrap();
                }

                let discriminant = quote! {
                    written += ToBytes::write_to(
                        &(::std::convert::Into::< #discriminant_type >::into(#next_discriminant) ),
                        write
                    )?;
                };

                match_arms.push(match variant.fields {
                    Fields::Named(fields) => {
                        let field_names = fields.named.iter().map(|f| &f.ident);
                        let field_names2 = field_names.clone();

                        quote! {
                            Self::#variant_name { #( #field_names ),*} => {
                               #discriminant
                               #(written += ToBytes::write_to( #field_names2, write)?; )*
                            }
                        }
                    }
                    Fields::Unnamed(fields) => {
                        let field_names = fields.unnamed.iter().enumerate().map(|(i, _)| {
                            syn::Ident::new(&format!("__field{}", i), Span::call_site())
                        });
                        let field_names2 = field_names.clone();

                        quote! {
                            Self::#variant_name ( #( #field_names ),*) => {
                               #discriminant
                               #(written += ToBytes::write_to( #field_names2, write)?; )*
                            }
                        }
                    }
                    Fields::Unit => {
                        quote! {
                            Self::#variant_name => {
                               #discriminant
                            }
                        }
                    }
                });

                next_discriminant += 1;
            }

            quote! {
                let mut written = 0;

                match self {
                    #(#match_arms,)*
                }

                Ok(written)
            }
        }
        Data::Union(_) => {
            return syn::Error::new(Span::call_site(), "ToBytes can't be derived for Unions")
                .to_compile_error()
                .into()
        }
    };

    let expanded = quote! {
        impl #impl_generics ToBytes for #name #ty_generics #where_clause {
            fn write_to<__W: ::std::io::Write>(&self, write: &mut __W) -> ::std::io::Result<usize> {
                #implementation
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(FromBytes, attributes(discriminant_as))]
pub fn derive_from_bytes(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let implementation = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let field_names = fields.named.iter().map(|f| &f.ident);

                quote! {
                    Ok(Self{
                        #( #field_names: FromBytes::read_from(read)?, )*
                    })
                }
            }
            Fields::Unnamed(fields) => {
                let field_types = fields.unnamed.iter().map(|f| &f.ty);
                quote! {
                    Ok(Self (
                        #({
                            let temp: #field_types = FromBytes::read_from(read)?;
                            temp
                        },)*
                    ))
                }
            }
            Fields::Unit => quote! {
                Ok(Self)
            },
        },
        Data::Enum(data) => {
            let discriminant_type = get_discriminant(input.attrs);

            let mut match_arms = Vec::new();
            let mut next_discriminant = 0;

            for variant in data.variants {
                let variant_name = variant.ident;

                if let Some((
                    _,
                    Expr::Lit(ExprLit {
                        lit: Lit::Int(d), ..
                    }),
                )) = variant.discriminant
                {
                    next_discriminant = d.base10_digits().parse().unwrap();
                }

                match variant.fields {
                    Fields::Named(fields) => {
                        let field_names = fields.named.iter().map(|f| &f.ident);

                        match_arms.push(quote! {
                            #next_discriminant => {
                                Ok(Self::#variant_name{ #(
                                    #field_names: FromBytes::read_from(read)?,
                                )* })
                            }
                        });
                    }
                    Fields::Unnamed(fields) => {
                        let field_types = fields.unnamed.iter().map(|f| &f.ty);

                        match_arms.push(quote! {
                            #next_discriminant => {
                                Ok(Self::#variant_name ( #(
                                    {
                                        let temp: #field_types = FromBytes::read_from(read)?;
                                        temp
                                    }
                                )* ))
                            }
                        });
                    }
                    Fields::Unit => {
                        match_arms.push(quote! {
                            #next_discriminant => {
                                Ok(Self::#variant_name)
                            }
                        });
                    }
                }

                next_discriminant += 1;
            }

            quote! {
                let discriminant: #discriminant_type = FromBytes::read_from(read)?;
                let discriminant: i32 = ::std::convert::Into::into(discriminant);

                match discriminant {
                    #(#match_arms,)*
                    _ => Err(::std::io::Error::new(::std::io::ErrorKind::InvalidData, "Invalid enum discriminant")),
                }
            }
        }
        Data::Union(_) => {
            return syn::Error::new(Span::call_site(), "FromBytes can't be derived for Unions")
                .to_compile_error()
                .into()
        }
    };

    let expanded = quote! {
        impl #impl_generics FromBytes for #name #ty_generics #where_clause {
            fn read_from<__R: ::std::io::Read>(read: &mut __R) -> ::std::io::Result<Self> {
                #implementation
            }
        }
    };

    TokenStream::from(expanded)
}

fn get_discriminant(attrs: Vec<Attribute>) -> TokenStream2 {
    for attribute in attrs {
        if let Some(ident) = attribute.path.get_ident() {
            if ident.to_string() == "discriminant_as" {
                match attribute.parse_args::<Path>() {
                    Ok(arg) => {
                        return quote! { #arg };
                    }
                    Err(_) => {
                        return syn::Error::new(attribute.span(), "invalid path")
                            .to_compile_error()
                            .into();
                    }
                }
            }
        }
    }

    quote! { protocol::VarInt }
}
