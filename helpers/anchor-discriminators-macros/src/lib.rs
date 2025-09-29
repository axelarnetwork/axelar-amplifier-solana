extern crate proc_macro;

use convert_case::{Case, Casing};
use quote::quote;
use syn::parse_macro_input;

use anchor_discriminators::{sighash, SIGHASH_GLOBAL_NAMESPACE};

fn gen_discriminator(namespace: &str, name: impl ToString) -> proc_macro2::TokenStream {
    let discriminator = sighash(namespace, name.to_string().as_str());
    format!("{discriminator:?}").parse().unwrap()
}

/// Derive macro that generates 8-byte instruction discriminators for enums with unit and named field variants.
///
/// This macro automatically:
/// - Generates unique 8-byte discriminators for each enum variant
/// - Implements custom `BorshSerialize` that writes discriminator + field data
/// - Implements custom `BorshDeserialize` that reads discriminator + field data
/// - Creates a `discriminators` module with constants for each variant
///
/// # Supported Variant Types
/// - Unit variants: `Initialize`
/// - Named field variants: `Transfer { amount: u64, recipient: Pubkey }`
///
///
/// ```ignore
/// #[derive(InstructionDiscriminator)]
/// pub enum MyInstruction {
///     Initialize,
///     Transfer { amount: u64, recipient: Pubkey },
///     Close,
/// }
/// ```
#[allow(clippy::too_many_lines)]
#[proc_macro_derive(InstructionDiscriminator)]
pub fn derive_instruction_discriminator(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);

    // Only support enums
    let enum_data = match &input.data {
        syn::Data::Enum(data) => data,
        syn::Data::Struct(_) | syn::Data::Union(_) => {
            return syn::Error::new_spanned(
                input,
                "InstructionDiscriminator can only be derived for enums",
            )
            .to_compile_error()
            .into();
        }
    };

    let enum_name = &input.ident;
    let enum_vis = &input.vis;

    // Generate discriminator constants and match arms
    let mut discriminator_constants = Vec::new();
    let mut discriminator_match_arms = Vec::new();
    let mut serialize_match_arms = Vec::new();
    let mut deserialize_match_arms = Vec::new();

    for variant in &enum_data.variants {
        let variant_name = &variant.ident;
        let variant_name_snake = variant_name.to_string().to_case(Case::Snake);
        let variant_name_constant = variant_name.to_string().to_case(Case::Constant);
        let const_name = syn::Ident::new(&variant_name_constant, variant.ident.span());

        // Generate discriminator constant
        let discriminator = gen_discriminator(SIGHASH_GLOBAL_NAMESPACE, &variant_name_snake);

        discriminator_constants.push(quote! {
            pub const #const_name: [u8; 8] = #discriminator;
        });

        match &variant.fields {
            // Unit variant: Initialize
            syn::Fields::Unit => {
                discriminator_match_arms.push(quote! {
                    #[doc = concat!("Discriminator for ", stringify!(#variant_name))]
                    #[doc = concat!("sha256(global::", #variant_name_snake, ")[..8]")]
                    Self::#variant_name => &discriminators::#const_name
                });

                serialize_match_arms.push(quote! {
                    Self::#variant_name => {
                        writer.write_all(&discriminators::#const_name)?;
                    }
                });

                deserialize_match_arms.push(quote! {
                    discriminators::#const_name => Ok(Self::#variant_name)
                });
            }

            // Named fields variant: Transfer { amount: u64, recipient: Pubkey }
            syn::Fields::Named(fields) => {
                // Extract field names and types for serialization
                let field_names: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| {
                        f.ident
                            .as_ref()
                            .expect("Named fields must have identifiers")
                    })
                    .collect();
                let field_types: Vec<_> = fields.named.iter().map(|f| &f.ty).collect();

                discriminator_match_arms.push(quote! {
                    #[doc = concat!("Discriminator for ", stringify!(#variant_name))]
                    #[doc = concat!("sha256(global::", #variant_name_snake, ")[..8]")]
                    Self::#variant_name {..} => &discriminators::#const_name
                });

                // For serialization, we need to serialize each field
                serialize_match_arms.push(quote! {
                    Self::#variant_name { #(#field_names),* } => {
                        writer.write_all(&discriminators::#const_name)?;
                        #(#field_names.serialize(writer)?;)*
                    }
                });

                // For deserialization, we need to deserialize each field
                deserialize_match_arms.push(quote! {
                    discriminators::#const_name => {
                        #(
                            let #field_names = <#field_types>::deserialize_reader(reader)?;
                        )*
                        Ok(Self::#variant_name { #(#field_names),* })
                    }
                });
            }

            // We don't support unnamed fields (tuples)
            syn::Fields::Unnamed(fields) => {
                if fields.unnamed.len() != 1 {
                    return syn::Error::new_spanned(
                        variant,
                        "InstructionDiscriminator only supports a single unnamed field.",
                    )
                    .to_compile_error()
                    .into();
                }

                discriminator_match_arms.push(quote! {
                    #[doc = concat!("Discriminator for ", stringify!(#variant_name))]
                    #[doc = concat!("sha256(global::", #variant_name_snake, ")[..8]")]
                    Self::#variant_name(..) => &discriminators::#const_name
                });

                serialize_match_arms.push(quote! {
                    Self::#variant_name(data) => {
                        writer.write_all(&discriminators::#const_name)?;
                        data.serialize(writer)?;
                    }
                });

                deserialize_match_arms.push(quote! {
                    discriminators::#const_name => {
                        let data = borsh::BorshDeserialize::deserialize_reader(reader)?;
                        Ok(Self::#variant_name(data))
                    }
                });
            }
        }
    }

    let expanded = quote! {
        #enum_vis mod discriminators {
            #(#discriminator_constants)*
        }

        impl #enum_name {
            /// Get the discriminator for this instruction variant
            pub fn discriminator(&self) -> &'static [u8; 8] {
                match self {
                    #(#discriminator_match_arms,)*
                }
            }
        }

        impl borsh::BorshSerialize for #enum_name {
            fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
                match self {
                    #(#serialize_match_arms)*
                }
                Ok(())
            }
        }

        impl borsh::BorshDeserialize for #enum_name {
            fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
                let mut discriminator = [0u8; 8];
                reader.read_exact(&mut discriminator)?;

                match discriminator {
                    #(#deserialize_match_arms,)*
                    _ => Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Unknown {} discriminator: {:?}",  stringify!(#enum_name), discriminator),
                    )),
                }
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}
