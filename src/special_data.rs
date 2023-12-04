use proc_macro_error::abort;
use syn::punctuated::Punctuated;
use syn::{braced, Expr, FieldMutability, Generics, Ident, parenthesized, Token, token, Visibility, WhereClause};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use crate::attributes::{FieldAttribute, ItemAttribute};
use crate::ty::SpecialType;

use syn::DeriveInput;



// most of the comments are stolen from the `syn` crate doc because im lazzzzzzy

/// The base type definition. It allows recursive definition expansions therefore
/// it is *Special*
pub struct Special {
    pub attrs: Vec<ItemAttribute>, // used to be RecAttribute
    pub vis: Visibility,
    pub ident: Ident,
    pub generics: Generics,
    pub body: Body
}

/// The body of a definition; Contains the data for the item
pub enum Body {
    Struct(BodyStruct),
    Enum(BodyEnum),
    // Union(BodyUnion), todo
}

/// Structure Body aka Data in syn
pub struct BodyStruct {
    pub struct_token: Token![struct],
    pub fields: SpecialFields,
    pub semi_token: Option<Token![;]>
}

/// Enumeration Body aka Data in syn
pub struct BodyEnum {
    enum_token: Token![enum],
    brace_token: token::Brace,
    variants: Punctuated<SpecialVariant, Token![,]>
}

// struct BodyUnion {
//     union_token: Token![union],
// }

/// An enum variant
pub struct SpecialVariant {
    /// Attributes belonging to variant:
    /// ```txt
    /// #[...]*<?> // standard attribute application
    /// #>[...]*<?> // applied to type definitions in variant
    /// Variant
    /// ```
    attrs: Vec<FieldAttribute>, // field attribute

    /// Name of the variant.
    ident: Ident,

    /// Content stored in the variant.
    fields: SpecialFields,

    /// Explicit discriminant: `Variant = 1`
    pub discriminant: Option<(Token![=], Expr)>,
}

/// Data stored in an enum variant or structure
pub enum SpecialFields {
    /// Named fields of a struct or struct variant such as
    /// `Point {
    ///     x: f64,
    ///     y: f64
    /// }`
    Named(FieldsNamed), //

    /// Unnamed fields of a tuple struct or tuple variant such as
    /// `Some(T)`.
    Unnamed(FieldsUnnamed),

    /// Unit struct or unit variant such as `None`.
    Unit,
}

/// Named fields of a struct or struct variant such as
/// `Point {
/// x: f64,
/// y: f64
/// }`
pub struct FieldsNamed {
    brace_token: token::Brace,
    named: Punctuated<SpecialField, Token![,]>
}

/// Unnamed fields of a tuple struct or tuple variant such as `Some(T)`.
pub struct FieldsUnnamed {
    paren_token: token::Paren,
    unnamed: Punctuated<SpecialField, Token![,]>
}

// note: refactor to a new file eventually

/// A field of a struct or enum variant.
struct SpecialField {
    pub attrs: Vec<FieldAttribute>,
    pub vis: Visibility,
    pub mutability: FieldMutability,
    /// Name of the field if any
    pub ident: Option<Ident>,
    pub colon_token: Option<Token![:]>,
    pub ty: SpecialType,
}


impl Parse for Special {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(ItemAttribute::parse_outer)?;
        let vis = input.parse::<Visibility>()?;

        let lookahead = input.lookahead1();
        if lookahead.peek(Token![struct]) {
            let struct_token = input.parse::<Token![struct]>()?;
            let ident = input.parse::<Ident>()?;
            let generics = input.parse::<Generics>()?;
            let (where_clause, fields, semi) = parse_data_struct(input)?;
            Ok(Special {
                attrs,
                vis,
                ident,
                generics: Generics {
                    where_clause,
                    ..generics
                },
                body: Body::Struct(BodyStruct {
                    struct_token,
                    fields,
                    semi_token: semi,
                })
            })
        } else if lookahead.peek(Token![enum]) {
            let enum_token = input.parse::<Token![enum]>()?;
            let ident = input.parse::<Ident>()?;
            let generics = input.parse::<Generics>()?;
            let (where_clause, brace, variants) = parse_data_enum(input)?;
            Ok(Special {
                attrs,
                vis,
                ident,
                generics: Generics {
                    where_clause,
                    ..generics
                },
                body: Body::Enum(BodyEnum {
                    enum_token,
                    brace_token: brace,
                    variants
                })
            })
        } else if lookahead.peek(Token![union]) {
            Err(input.error("unions remain unimplemented"))
        } else {
            Err(lookahead.error())
        }
    }
}

fn parse_data_struct(
    input: ParseStream,
) -> syn::Result<(Option<WhereClause>, SpecialFields, Option<Token![;]>)> {
    let mut lookahead = input.lookahead1();
    let mut where_clause: Option<WhereClause> = None;
    if lookahead.peek(Token![where]) {
        where_clause = Some(input.parse()?);
        lookahead = input.lookahead1();
    }

    if where_clause.is_none() && lookahead.peek(token::Paren) {
        let fields: FieldsUnnamed = input.parse()?;

        lookahead = input.lookahead1();
        if lookahead.peek(Token![where]) {
            where_clause = Some(input.parse()?);
            lookahead = input.lookahead1();
        }

        if lookahead.peek(Token![;]) {
            let semi = input.parse()?;
            Ok((where_clause, SpecialFields::Unnamed(fields), Some(semi)))
        } else {
            Err(lookahead.error())
        }
    } else if lookahead.peek(token::Brace) {
        let fields: FieldsNamed = input.parse()?;

        Ok((where_clause, SpecialFields::Named(fields), None))
    } else if lookahead.peek(Token![;]) {
        let semi = input.parse()?;
        Ok((where_clause, SpecialFields::Unit, Some(semi)))
    } else {
        Err(lookahead.error())
    }
}

fn parse_data_enum(
    input: ParseStream,
) -> syn::Result<(
    Option<WhereClause>,
    token::Brace,
    Punctuated<SpecialVariant, Token![,]>)>
{
    let where_clause = input.parse()?;

    let content;
    let brace = braced!(content in input);
    let variants = content.parse_terminated(SpecialVariant::parse, Token![,])?;

    Ok((where_clause, brace, variants))
}

impl Parse for SpecialVariant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(FieldAttribute::parse_outer)?;
        let _vis: Visibility = input.parse()?; // todo: check if can be removed because its stupid
        let ident: Ident = input.parse()?;
        let fields = if input.peek(token::Brace) {
            SpecialFields::Named(input.parse()?)
        } else if input.peek(token::Paren) {
            SpecialFields::Unnamed(input.parse()?)
        } else {
            SpecialFields::Unit
        };
        let discriminant = if input.peek(Token![=]) {
            let eq_token: Token![=] = input.parse()?;
            let discriminant: Expr = input.parse()?;
            Some((eq_token, discriminant))
        } else {
            None
        };
        Ok(SpecialVariant {
            attrs,
            ident,
            fields,
            discriminant,
        })
    }
}

impl Parse for FieldsNamed {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(FieldsNamed {
            brace_token: braced!(content in input),
            named: content.parse_terminated(SpecialField::parse_named, Token![,])?,
        })
    }
}

impl Parse for FieldsUnnamed {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(FieldsUnnamed {
            paren_token: parenthesized!(content in input),
            unnamed: content.parse_terminated(SpecialField::parse_unnamed, Token![,])?,
        })
    }
}

impl SpecialField {
    pub fn parse_named(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(FieldAttribute::parse_outer)?;
        let vis: Visibility = input.parse()?;

        // note: has cfg!(feature = "full") | data.rs
        let unnamed_field = input.peek(Token![_]);

        let ident = if unnamed_field {
            input.call(Ident::parse_any)
        } else {
            input.parse()
        }?;

        let colon_token: Token![:] = input.parse()?;

        let ty: SpecialType = if unnamed_field
            && (input.peek(Token![struct])
            || input.peek(Token![union]) && input.peek2(token::Brace)) {
            let span= input.span();
            abort!(
                span,
                "Not implemented Yet";
                note = "Requires a rewrite of the syn::verbatim module"
            ); // todo
        } else {
            input.parse()?
        };

        Ok( SpecialField {
            attrs,
            vis,
            mutability: FieldMutability::None,
            ident: Some(ident),
            colon_token: Some(colon_token),
            ty
        })
    }

    pub fn parse_unnamed(input: ParseStream) -> syn::Result<Self> {
        Ok( SpecialField {
            attrs: input.call(FieldAttribute::parse_outer)?,
            vis: input.parse()?,
            mutability: FieldMutability::None,
            ident: None,
            colon_token: None,
            ty: input.parse()?,
        })
    }
}

// todo: actually refactor a bunch of stuff
