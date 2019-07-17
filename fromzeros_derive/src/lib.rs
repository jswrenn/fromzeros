#![feature(option_result_contains)]

extern crate proc_macro;

use if_chain::if_chain;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned};
use syn;

#[proc_macro_derive(FromZeros)]
pub fn from_zeros_derive(input: TokenStream) -> TokenStream {
  let ast : syn::DeriveInput = syn::parse(input).unwrap();

  let name = &ast.ident;
  let attrs = &ast.attrs;
  let generics = ast.generics;

  match ast.data {
    // `FromZeros` may be implemented for any struct whose fields all implement
    // `FromZeros`.
    syn::Data::Struct(data)
      => impl_fromzeros(name, generics, None, &data.fields),

    // `FromZeros` may be implemented for any union in which there exists any
    // variant that implements `FromZeros`. Unfortunately, this 'any'
    // requirement is not expressible by a macro. We therefore require that all
    // variants implement `FromZeros`.
    syn::Data::Union(data)
      => impl_fromzeros(name, generics, None, &data.fields.into()),

    // `FromZeros` may be implemented for any enum whose memory layout is
    // well-defined and possesses a zero-discriminant variant in which all
    // fields implement `FromZeros`.
    //
    // An enum's layout is well-defined if either:
    //  * it is a C-like enum
    //  * it uses a primitive repr
    //
    // Such an enum will have a zero discriminant if:
    //  * there exists a variant with the explicit discriminant '0'
    //  * the first variant does not have an explicit discriminant
    syn::Data::Enum(ref data)
      => {
      if !(is_clike(data) || has_primitive_repr(attrs)) {
        panic!("{} must be either C-like, or use a primitive repr.", name);
      }

      if let Some(variant) = zero_variant(data) {
        impl_fromzeros(name, generics, Some(&variant.ident), &variant.fields)
      } else {
        panic!("{} does not have a variant with a provably-zero discriminant.");
      }
    }
  }.into()
}

// implement `FromZeros` for a given type
fn impl_fromzeros(
  name      : &syn::Ident,
  generics  : syn::Generics,
  variant   : Option<&syn::Ident>,
  fields    : &syn::Fields
) -> TokenStream2
{
  let zeroed = zeroed_fields(fields);
  let generics = add_trait_bounds(generics);
  let (impl_generics, ty_generics, where_clause) =  generics.split_for_impl();

  return quote! {
    unsafe impl #impl_generics fromzeros::FromZeros for #name #ty_generics
    #where_clause
    {
      #[inline(always)]
      fn zeroed() -> Self
      where Self: Sized
      {
        #name #(:: #variant)* #zeroed
      }
    }
  };

  // helper functions:

  // adds `FromZeros` bounds to each generic parameter
  fn add_trait_bounds(mut generics: syn::Generics) -> syn::Generics {
    for param in &mut generics.params {
        if let syn::GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(syn::parse_quote!(fromzeros::FromZeros));
        }
    }
    generics
  }

  // Given `n` fields, produce `n` comma-separated calls to `FromZeros::zeroed()`
  fn zeroed_fields(fields: &syn::Fields) -> TokenStream2 {
    use syn::spanned::Spanned;

    match fields {
      syn::Fields::Unit => quote!{},

      syn::Fields::Unnamed(ref fields) => {
        let fields = fields.unnamed.iter().map(|f| {
          let ty = &f.ty;
          quote_spanned! {f.span() =>
            <#ty as fromzeros::FromZeros>::zeroed()
          }
        });
        quote! { ( #(#fields),* ) }
      },

      syn::Fields::Named(ref fields) => {
        let fields = fields.named.iter().map(|f| {
          let name = &f.ident;
          let ty = &f.ty;
          quote_spanned! {f.span() =>
            #name : <#ty as fromzeros::FromZeros>::zeroed()
          }
        });
        quote! { {#(#fields),* } }
      },
    }
  }

}

// given an enum, produce the variant with a zero discriminant, if any
fn zero_variant(ast: &syn::DataEnum) -> Option<&syn::Variant> {
  let mut variants = ast.variants.iter();

  let first = variants.next()?;

  // the discriminant of the first variant is implicitly zero unless specified
  let first_discriminant = explicit_discriminant(first).unwrap_or(0);

  if first_discriminant == 0 {
    return Some(first);
  } else {
    return variants.find(|variant| explicit_discriminant(variant).contains(&0));
  }

  // helpers:

  // given a variant, produce the value of its explicit discriminant, if any
  fn explicit_discriminant(variant: &syn::Variant) -> Option<u64> {
    if_chain! {
      if let Some((_, ref disr)) = variant.discriminant;
      if let syn::Expr::Lit(disr) = disr;
      if let syn::Lit::Int(ref disr) = disr.lit;
      then {
        Some(disr.value())
      } else {
        None
      }
    }
  }

}

// produces `true` if all variants are unit-like
fn is_clike(ast: &syn::DataEnum) -> bool {
  ast.variants.iter()
    .all(|variant|
      match variant.fields {
        syn::Fields::Unit => true,
        _ => false,
      })
}

fn has_primitive_repr(attrs: &[syn::Attribute]) -> bool {
  return if_chain!{
    if let Some(repr_attr) = attrs.iter().find_map(repr);
    then {
      repr_attr.nested.iter().any(is_primitive)
    } else {
      false
    }
  };

  // helper functions:

  // produces the repr attr, if any
  fn repr(ast: &syn::Attribute) -> Option<syn::MetaList> {
    if_chain! {
      if let Some(attr) = ast.interpret_meta();
      if let syn::Meta::List(attr) = attr;
      if attr.ident == "repr";
      then {
        return Some(attr)
      } else {
        return None
      }
    };
  }

  // produces true if the representation is primitive
  fn is_primitive(meta: &syn::NestedMeta) -> bool {
    const VALID: [&str; 12] =
      [
        "i8", "i16", "i32", "i64", "i128", "isize",
        "u8", "u16", "u32", "u64", "u128", "usize",
      ];

    if_chain! {
      if let syn::NestedMeta::Meta(meta) = meta;
      if let syn::Meta::Word(repr) = meta;
      then {
        return VALID.iter().any(|t| repr == t);
      } else {
        return false;
      }
    }
  }

}
