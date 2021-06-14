extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_attribute]
pub fn serializable(_attr: TokenStream, mut item: TokenStream) -> TokenStream {
    let temp = item.clone();
    let input = parse_macro_input!(temp as DeriveInput);

    let name = input.ident;

    match input.data {
        Data::Struct(datastruct) => match datastruct.fields {
            Fields::Named(fields) => {
                let field_names = fields.named.iter().map(|f| &f.ident);
                item.extend(TokenStream::from(quote! {
                    impl Serializable for #name {
                        fn to_writer<W: ::std::io::Write>(&self, output: &mut W) -> ::std::io::Result<()> {
                            #(Serializable::to_writer(&self.#field_names, &mut *output)?;)*
                            Ok(())
                        }
                    }
                }));
            }
            Fields::Unnamed(fields) => {
                let field_indices = fields.unnamed.iter().enumerate().map(|f| f.0);
                item.extend(TokenStream::from(quote! {
                    impl Serializable for #name {
                        fn to_writer<W: ::std::io::Write>(&self, output: &mut W) -> ::std::io::Result<()> {
                            #(Serializable::to_writer(&self.#field_indices, &mut *output)?;)*
                            Ok(())
                        }
                    }
                }));
            }
            Fields::Unit => {
                item.extend(TokenStream::from(quote! {
                    impl Serializable for #name {
                        fn to_writer<W: ::std::io::Write>(&self, output: &mut W) -> ::std::io::Result<()> {
                            Ok(())
                        }
                    }
                }));
            }
        },
        Data::Enum(dataenum) => {
            let mut variants: Vec<_> = dataenum
                .variants
                .iter()
                .map(|v| {
                    let id = &v.ident;
                    quote! {#id}
                })
                .collect();
            let mut implementations = Vec::new();
            let mut discriminant = 0;
            for (i, variant) in dataenum.variants.iter().enumerate() {
                if let Some((
                    _,
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(d),
                        ..
                    }),
                )) = &variant.discriminant
                {
                    discriminant = d.base10_parse::<i32>().unwrap();
                }
                match &variant.fields {
                    Fields::Named(fields) => {
                        let field_names = fields.named.iter().map(|f| &f.ident);
                        variants[i].extend(quote! {
                            {
                                #(#field_names,)*
                            }
                        });
                        let field_names = fields.named.iter().map(|f| &f.ident);
                        implementations.push(quote! {
                            Serializable::to_writer(&VarInt(#discriminant), &mut *output)?;
                            #(Serializable::to_writer(#field_names, &mut *output)?;)*
                        });
                    }
                    Fields::Unnamed(fields) => {
                        let field_names = fields.unnamed.iter().enumerate().map(|(i, _)| {
                            syn::Ident::new(
                                &format!("__field{}", i),
                                quote::__private::Span::call_site(),
                            )
                        });
                        variants[i].extend(quote! {
                            (
                                #(#field_names,)*
                            )
                        });
                        let field_names = fields.unnamed.iter().enumerate().map(|(i, _)| {
                            syn::Ident::new(
                                &format!("__field{}", i),
                                quote::__private::Span::call_site(),
                            )
                        });
                        implementations.push(quote! {
                            Serializable::to_writer(&VarInt(#discriminant), &mut *output)?;
                            #(Serializable::to_writer(#field_names, &mut *output)?;)*
                        });
                    }
                    Fields::Unit => {
                        implementations.push(quote! {
                            Serializable::to_writer(&VarInt(#discriminant), &mut *output)?;
                        });
                    }
                }

                discriminant += 1;
            }
            item.extend(TokenStream::from(quote! {
                impl Serializable for #name {
                    fn to_writer<W: ::std::io::Write>(&self, output: &mut W) -> ::std::io::Result<()> {
                        match self {
                            #(Self::#variants => {
                                #implementations
                            },)*
                        }
                        Ok(())
                    }
                }
            }));
        }
        Data::Union(_) => {
            panic!("Serializable does not support unions");
        }
    }

    item
}

#[proc_macro_attribute]
pub fn deserializable(_attr: TokenStream, mut item: TokenStream) -> TokenStream {
    let temp = item.clone();
    let input = parse_macro_input!(temp as DeriveInput);

    let name = input.ident;

    match input.data {
        Data::Struct(datastruct) => match datastruct.fields {
            Fields::Named(fields) => {
                let field_names = fields.named.iter().map(|f| &f.ident);
                item.extend(TokenStream::from(quote! {
                    impl Deserializable for #name {
                        fn from_reader<R: ::std::io::Read>(input: &mut R) -> ::std::io::Result<Self> {
                            Ok(Self {
                                #(#field_names: Deserializable::from_reader(&mut *input)?,)*
                            })
                        }
                    }
                }));
            }
            Fields::Unnamed(fields) => {
                let field_types = fields.unnamed.iter().map(|f| &f.ty);
                item.extend(TokenStream::from(quote! {
                    impl Deserializable for #name {
                        fn from_reader<R: ::std::io::Read>(input: &mut R) -> ::std::io::Result<Self> {
                            Ok(Self (
                                #({
                                    let temp: #field_types = Deserializable::from_reader(&mut *input)?;
                                    temp
                                },)*
                            ))
                        }
                    }
                }));
            }
            Fields::Unit => {
                item.extend(TokenStream::from(quote! {
                    impl Deserializable for #name {
                        fn from_reader<R: ::std::io::Read>(input: &mut R) -> ::std::io::Result<Self> {
                            Ok(Self{})
                        }
                    }
                }));
            }
        },
        Data::Enum(dataenum) => {
            let mut discriminants = Vec::new();
            let mut implementations = Vec::new();
            let mut discriminant = 0;
            for variant in &dataenum.variants {
                if let Some((
                    _,
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(d),
                        ..
                    }),
                )) = &variant.discriminant
                {
                    discriminant = d.base10_parse::<i32>().unwrap();
                }

                discriminants.push(discriminant);

                let variant_name = &variant.ident;

                match &variant.fields {
                    Fields::Named(fields) => {
                        let field_names = fields.named.iter().map(|f| &f.ident);
                        implementations.push(quote! {
                            Self::#variant_name {
                                #(#field_names: Deserializable::from_reader(&mut *input)?,)*
                            }
                        });
                    }
                    Fields::Unnamed(fields) => {
                        let field_types = fields.unnamed.iter().map(|f| &f.ty);
                        implementations.push(quote! {
                            Self::#variant_name (
                                #({
                                    let temp: #field_types = Deserializable::from_reader(&mut *input)?;
                                    temp
                                },)*
                            )
                        });
                    }
                    Fields::Unit => {
                        implementations.push(quote! {
                            Self::#variant_name
                        });
                    }
                }

                discriminant += 1;
            }
            item.extend(TokenStream::from(quote! {
                impl Deserializable for #name {
                    fn from_reader<R: ::std::io::Read>(input: &mut R) -> ::std::io::Result<Self> {
                        let discriminant: VarInt = Deserializable::from_reader(&mut *input)?;
                        match discriminant.0 {
                            #(#discriminants => Ok(
                                #implementations
                            ),)*
                            _ => Err(::std::io::Error::new(::std::io::ErrorKind::Other, "Invalid enum discriminant")),
                        }
                    }
                }
            }));
        }
        Data::Union(_) => {
            panic!("Deserializable does not support unions");
        }
    }

    item
}
