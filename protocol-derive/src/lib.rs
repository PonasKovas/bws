extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, parse_quote, Attribute, Data, DeriveInput, Fields, LitInt, Path};

/// Derives the `Serializable` trait for structs and enums.
///
/// Requires that all members of the given type already implement `Serializable`
///
/// # `#[discriminant_as]`
///
/// The `#[discriminant_as(TYPE)]` attribute can be used on enums to make them use the given `TYPE`
/// when serializing their discriminants. `TYPE` must implement `TryInto<i32>` (for deserialization)
/// and `TryFrom<i32>` (for serialization).
///
/// # `#[discriminant]`
///
/// `#[discriminant(DISCRIMINANT)]` can be used on individual variants of an enum to set specific
/// discriminants when serializing. All variants that follow get increased discriminants by 1.
///
/// For example:
///
/// ```
/// # use protocol_derive::Serializable;
/// #[derive(Serializable)]
/// enum Foo {
///     FirstVariant,  // discriminant = 0
///     SecondVariant, // discriminant = 1
///     #[discriminant(6)]
///     SpecialCase,   // discriminant = 6
///     FourthVariant, // discriminant = 7
/// }
/// ```
///
/// # `#[inline_enum]`
///
/// `#[inline_enum]` can be used on a sinle variant of an enum that is a tuple with a single
/// field which is another enum, and the inner enum will be "inlined".
///
/// It will behave as if the outer enum "extended" the inner one, they will use a shared discriminant,
/// so it's recommended to use `#[inline_enum]` together with `#[discriminant]` on the variant that
/// follows the inlined one, so discriminants don't overlap.
///
/// Example:
///
/// ```rust
/// # use protocol_derive::Serializable;
/// #[derive(Serializable)]
/// enum Base {
///     First,
///     Second,
/// }
///
/// #[derive(Serializable)]
/// enum Child {
///     // If this one get serialized, it will use the discriminant of Base instead of Child
///     #[inline_enum]
///     Base(Base),
///     // So set the discriminant of the following variant to not overlap
///     #[discriminant(2)]
///     Third,
/// }
/// ```
///
#[proc_macro_derive(Serializable, attributes(discriminant_as, discriminant, inline_enum))]
pub fn derive_serializable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let field_names = fields.named.iter().map(|f| &f.ident);

                TokenStream::from(quote! {
                    impl#generics protocol::Serializable for #name#generics {
                        fn to_writer<__W: ::std::io::Write>(&self, __output: &mut __W) -> ::std::io::Result<usize> {
                            let mut sum = 0;
                            #(sum += protocol::Serializable::to_writer(&self.#field_names, __output)?;)*
                            Ok(sum)
                        }
                    }
                })
            }
            Fields::Unnamed(fields) => {
                let field_indices = fields.unnamed.iter().enumerate().map(|f| syn::Index {
                    index: f.0 as u32,
                    span: Span::call_site(),
                });
                TokenStream::from(quote! {
                    impl#generics protocol::Serializable for #name#generics {
                        fn to_writer<__W: ::std::io::Write>(&self, __output: &mut __W) -> ::std::io::Result<usize> {
                            let mut sum = 0;
                            #(sum += protocol::Serializable::to_writer(&self.#field_indices, __output)?;)*
                            Ok(sum)
                        }
                    }
                })
            }
            Fields::Unit => TokenStream::from(quote! {
                impl#generics protocol::Serializable for #name#generics {
                    fn to_writer<__W: ::std::io::Write>(&self, __output: &mut __W) -> ::std::io::Result<usize> {
                        Ok(0)
                    }
                }
            }),
        },
        Data::Enum(data) => {
            let discriminant_as = parse_attrs(input.attrs);

            let mut match_arms = Vec::new();

            let mut next_discriminant = 0;

            for variant in data.variants {
                let variant_name = variant.ident;

                // check the attributes to see if it needs to be inlined or a specific discriminant
                let mut should_inline = false;

                for attribute in variant.attrs {
                    if let Some(ident) = attribute.path.get_ident() {
                        match format!("{}", ident).as_str() {
                            "inline_enum" => {
                                should_inline = true;
                            }
                            "discriminant" => {
                                let numeric = match attribute.parse_args::<LitInt>() {
                                    Ok(n) => n,
                                    Err(e) => {
                                        panic!("Error parsing the given discriminant: {}", e);
                                    }
                                };
                                next_discriminant = match numeric.base10_parse() {
                                    Ok(n) => n,
                                    Err(e) => {
                                        panic!("Error parsing the given discriminant: {}", e);
                                    }
                                };
                            }
                            _ => {}
                        }
                    }
                }

                if should_inline {
                    if let Fields::Unnamed(unnamed) = variant.fields {
                        if unnamed.unnamed.len() != 1 {
                            panic!("Enums can only be inlined if they're inside a tuple variant with only 1 field. Example: Variant(BaseEnum)")
                        }
                        match_arms.push(quote! {
                            Self::#variant_name (__inlined_enum) => {
                                sum += protocol::Serializable::to_writer(__inlined_enum, __output)?;
                            }
                        });
                    } else {
                        panic!("Enums can only be inlined if they're inside a tuple variant with only 1 field. Example: Variant(BaseEnum)")
                    }
                } else {
                    // serialize normally
                    let discriminant = quote! {
                        sum += protocol::Serializable::to_writer(
                            &(::core::convert::TryInto::< #discriminant_as >::try_into(#next_discriminant)
                                .expect(&format!(
                                    "Couldn't convert the discriminant {} to type {}",
                                    #next_discriminant,
                                    std::any::type_name::< #discriminant_as >()
                                ))),
                            __output
                        )?;
                    };

                    match variant.fields {
                        Fields::Named(fields) => {
                            let field_names = fields.named.iter().map(|f| &f.ident);
                            let field_names2 = field_names.clone();

                            match_arms.push(quote! {
                                 Self::#variant_name { #( #field_names ),*} => {
                                    #discriminant
                                    #( sum += protocol::Serializable::to_writer( #field_names2, __output)?; )*
                                 }
                             });
                        }
                        Fields::Unnamed(fields) => {
                            let field_names = fields.unnamed.iter().enumerate().map(|(i, _)| {
                                syn::Ident::new(&format!("__field{}", i), Span::call_site())
                            });
                            let field_names2 = field_names.clone();

                            match_arms.push(quote! {
                                 Self::#variant_name ( #( #field_names ),*) => {
                                    #discriminant
                                    #( sum += protocol::Serializable::to_writer( #field_names2, __output)?; )*
                                 }
                             });
                        }
                        Fields::Unit => {
                            match_arms.push(quote! {
                                Self::#variant_name => {
                                   #discriminant
                                }
                            });
                        }
                    }

                    next_discriminant += 1;
                }
            }

            TokenStream::from(quote! {
                impl#generics protocol::Serializable for #name#generics {
                    #[allow(clippy::nonstandard_macro_braces)]
                    fn to_writer<__W: ::std::io::Write>(&self, __output: &mut __W) -> ::std::io::Result<usize> {
                        let mut sum = 0;

                        match self {
                            #(#match_arms,)*
                            _ => {}
                        }

                        Ok(sum)
                    }
                }
            })
        }
        Data::Union(_) => panic!("Unions are not supported!"),
    }
}

/// Derives the `Deserializable` trait for structs and enums.
///
/// Requires that all members of the given type already implement `Deserializable`
///
/// # `#[discriminant_as]`
///
/// The `#[discriminant_as(TYPE)]` attribute can be used on enums to make them use the given `TYPE`
/// when deserializing their discriminants. `TYPE` must implement `TryInto<i32>` (for deserialization)
/// and `TryFrom<i32>` (for serialization).
///
/// # `#[discriminant]`
///
/// `#[discriminant(DISCRIMINANT)]` can be used on individual variants of an enum to set specific
/// discriminants when deserializing. All variants that follow get increased discriminants by 1.
///
/// For example:
///
/// ```rust
/// # use protocol_derive::Deserializable;
/// #[derive(Deserializable)]
/// enum Foo {
///     FirstVariant,  // discriminant = 0
///     SecondVariant, // discriminant = 1
///     #[discriminant(6)]
///     SpecialCase,   // discriminant = 6
///     FourthVariant, // discriminant = 7
/// }
/// ```
///
/// # `#[inline_enum]`
///
/// `#[inline_enum]` can be used on a sinle variant of an enum that is a tuple with a single
/// field which is another enum, and the inner enum will be "inlined".
///
/// It will behave as if the outer enum "extended" the inner one, they will use a shared discriminant,
/// so it's recommended to use `#[inline_enum]` together with `#[discriminant]` on the variant that
/// follows the inlined one, so discriminants don't overlap.
///
/// Example:
///
/// ```rust
/// # use protocol_derive::Deserializable;
/// #[derive(Deserializable)]
/// enum Base {
///     First,
///     Second,
/// }
///
/// #[derive(Deserializable)]
/// enum Child {
///     // when serializing, this variant will be constructed if the discriminant is valid in Base
///     #[inline_enum]
///     Base(Base),
///     // So set the discriminant of the following variant to not overlap
///     #[discriminant(2)]
///     Third,
/// }
/// ```
///
#[proc_macro_derive(Deserializable, attributes(discriminant_as, discriminant, inline_enum))]
pub fn derive_deserializable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let field_names = fields.named.iter().map(|f| &f.ident);

                TokenStream::from(quote! {
                    impl#generics protocol::Deserializable for #name#generics {
                        fn from_reader<__R: ::std::io::Read>(__input: &mut __R) -> ::std::io::Result<Self> {
                            Ok(Self{
                                #(#field_names: protocol::Deserializable::from_reader(__input)?,)*
                            })
                        }
                    }
                })
            }
            Fields::Unnamed(fields) => {
                let field_types = fields.unnamed.iter().map(|f| &f.ty);
                TokenStream::from(quote! {
                    impl#generics protocol::Deserializable for #name#generics {
                        fn from_reader<__R: ::std::io::Read>(__input: &mut __R) -> ::std::io::Result<Self> {
                            Ok(Self (
                                #({
                                    let temp: #field_types = protocol::Deserializable::from_reader(__input)?;
                                    temp
                                },)*
                            ))
                        }
                    }
                })
            }
            Fields::Unit => TokenStream::from(quote! {
                impl#generics protocol::Deserializable for #name#generics {
                    fn from_reader<__R: ::std::io::Read>(__input: &mut __R) -> ::std::io::Result<Self> {
                        Ok(Self)
                    }
                }
            }),
        },
        Data::Enum(data) => {
            let discriminant_as = parse_attrs(input.attrs);

            let mut match_arms = Vec::new();
            let mut inlined_arm = None;

            let mut next_discriminant = 0;

            for variant in data.variants {
                let variant_name = variant.ident;

                // check the attributes to see if it's inlined or with a specific discriminant
                let mut is_inlined = false;

                for attribute in variant.attrs {
                    if let Some(ident) = attribute.path.get_ident() {
                        match format!("{}", ident).as_str() {
                            "inline_enum" => {
                                is_inlined = true;
                            }
                            "discriminant" => {
                                let numeric = match attribute.parse_args::<LitInt>() {
                                    Ok(n) => n,
                                    Err(e) => {
                                        panic!("Error parsing the given discriminant: {}", e);
                                    }
                                };
                                next_discriminant = match numeric.base10_parse() {
                                    Ok(n) => n,
                                    Err(e) => {
                                        panic!("Error parsing the given discriminant: {}", e);
                                    }
                                };
                            }
                            _ => {}
                        }
                    }
                }

                if is_inlined {
                    if let Fields::Unnamed(unnamed) = variant.fields {
                        if unnamed.unnamed.len() != 1 {
                            panic!("Enums can only be inlined if they're inside a tuple variant with only 1 field. Example: Variant(BaseEnum)")
                        }

                        match_arms.push(quote! {
                             _ => {
                                 let mut __peeked_input = protocol::PeekedStream {
                                     peeked: Some(original_discriminant),
                                     stream: __input,
                                 };
                                 Ok(Self::#variant_name ( protocol::Deserializable::from_reader(&mut __peeked_input)? ) )
                             }
                         });

                        inlined_arm = Some(match_arms.len() - 1);
                    } else {
                        panic!("Enums can only be inlined if they're inside a tuple variant with only 1 field. Example: Variant(BaseEnum)")
                    }
                } else {
                    // serialize normally
                    match variant.fields {
                        Fields::Named(fields) => {
                            let field_names = fields.named.iter().map(|f| &f.ident);

                            match_arms.push(quote! {
                                 #next_discriminant => {
                                     Ok(Self::#variant_name{ #(
                                         #field_names: protocol::Deserializable::from_reader(__input)?,
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
                                             let temp: #field_types = protocol::Deserializable::from_reader(__input)?;
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
            }

            if let Some(arm) = inlined_arm {
                // make the inlined arm the last one
                let last = match_arms.len() - 1;
                match_arms.swap(arm, last);
            }

            TokenStream::from(quote! {
                impl#generics protocol::Deserializable for #name#generics {
                    #[allow(clippy::nonstandard_macro_braces)]
                    fn from_reader<__R: ::std::io::Read>(__input: &mut __R) -> ::std::io::Result<Self> {
                        let original_discriminant: #discriminant_as = protocol::Deserializable::from_reader(__input)?;

                        let discriminant: i32 = ::core::convert::TryInto::try_into(original_discriminant.clone())
                                            .expect(&format!(
                                                "Couldn't convert the discriminant {} of type {} to i32",
                                                #next_discriminant,
                                                std::any::type_name::< #discriminant_as >()
                                            ));

                        match discriminant {
                            #(#match_arms,)*
                            _ => Err(::std::io::Error::new(::std::io::ErrorKind::Other, "Invalid enum discriminant")),
                        }
                    }
                }
            })
        }
        Data::Union(_) => panic!("Unions are not supported!"),
    }
}

fn parse_attrs(attrs: Vec<Attribute>) -> Path {
    let mut discriminant_as: Path = parse_quote! {protocol::datatypes::VarInt};

    for attribute in attrs {
        if let Some(ident) = attribute.path.get_ident() {
            if format!("{}", ident).as_str() == "discriminant_as" {
                match attribute.parse_args::<Path>() {
                    Ok(arg) => {
                        discriminant_as = arg;
                    }
                    Err(_) => {
                        panic!("Usage: #[discriminant_as(u32)] with any type that implements Serialize/Deserialize and TryFrom<i32>/TryInto<i32> instead of u32");
                    }
                }
            }
        }
    }

    discriminant_as
}
