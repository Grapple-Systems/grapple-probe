// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

pub struct Packet {
    pub maybe_pub: Option<syn::token::Pub>,
    pub name: syn::Ident,
    pub packet_type: syn::Expr,
    pub response_type: Option<syn::Ident>,
    pub fields: Vec<Field>,
    pub repeat: Option<Field>,
}

impl syn::parse::Parse for Packet {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let maybe_pub = input.parse::<syn::token::Pub>().ok();
        let name = input.parse()?;

        let decorator_scope;
        _ = syn::parenthesized!(decorator_scope in input);
        let packet_type = decorator_scope.parse()?;
        let response_type = if decorator_scope.parse::<syn::Token![,]>().is_ok() {
            decorator_scope.parse().ok()
        } else {
            None
        };
        
        let field_scope;
        _ = syn::braced!(field_scope in input);
        let mut fields = Vec::new();
        let mut repeat = None;
        if let Ok(field_scope_iter) = field_scope.parse_terminated(Field::parse, syn::Token![,]) {
            for field in field_scope_iter {
                if repeat.is_some() {
                    return Err(syn::Error::new(field_scope.span(), "more fields after a repeat is not supported"))
                }

                if matches!(field.ty, FieldType::Slice(_)) || matches!(field.ty, FieldType::Repeat(_)) {
                    repeat = Some(field.clone());
                }
                fields.push(field.clone());
            }
        }

        Ok(Self { maybe_pub, name, packet_type, response_type, fields, repeat })
    }
}

#[derive(Clone, PartialEq)]
pub enum PrimitiveType {
    Uint8,
    Uint16,
    Uint32,
    Float,
}

impl PrimitiveType {
    pub fn data_size(&self) -> usize {
        match self {
            Self::Uint8 => size_of::<u8>(),
            Self::Uint16 => size_of::<u16>(),
            Self::Uint32 => size_of::<u32>(),
            Self::Float => size_of::<f32>(),
        }
    }
}

impl syn::parse::Parse for PrimitiveType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let field_type = input.parse::<syn::Ident>()?;
        match field_type.to_string().as_str() {
            "uint8" => Ok(Self::Uint8),
            "uint16" => Ok(Self::Uint16),
            "uint32" => Ok(Self::Uint32),
            "float" => Ok(Self::Float),
            _ => Err(syn::Error::new(field_type.span(), "unsupported primitive")),
        }
    }
}

#[derive(Clone)]
pub enum FieldType {
    Primitive(PrimitiveType),
    Array(PrimitiveType, usize),
    Repeat(Vec<PrimitiveField>),
    Slice(PrimitiveType),
}

impl syn::parse::Parse for FieldType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.peek(syn::token::Bracket) {
            let bracket_inner;
            _ = syn::bracketed!(bracket_inner in input);

            if bracket_inner.peek(syn::token::Brace) {
                // this is a repeat
                let paren_inner;
                _ = syn::braced!(paren_inner in bracket_inner);
                let fields = paren_inner.parse_terminated(PrimitiveField::parse, syn::Token![,])?;
                let fields: Vec<PrimitiveField> = fields.iter().map(|f| f.clone()).collect();
                if fields.is_empty() {
                    Err(syn::Error::new(paren_inner.span(), "repeat has no fields"))
                } else {
                    Ok(Self::Repeat(fields))
                }
            } else {
                // this is a slice or an array
                let inner_type = bracket_inner.parse()?;
                if bracket_inner.peek(syn::Token![;]) {
                    _ = bracket_inner.parse::<syn::Token![;]>()?;
                    let len = bracket_inner.parse::<syn::LitInt>()?;
                    Ok(Self::Array(inner_type, len.base10_parse::<usize>()?))
                } else {
                    Ok(Self::Slice(inner_type))
                }
            }
        } else {
            let field_type = input.parse()?;
            Ok(Self::Primitive(field_type))
        }
    }
}

#[derive(Clone)]
pub struct PrimitiveField {
    pub name: syn::Ident,
    pub ty: PrimitiveType,
}

impl syn::parse::Parse for PrimitiveField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        _ = input.parse::<syn::Token![:]>()?;
        let ty = input.parse()?;
        Ok(Self { name, ty })
    }
}

#[derive(Clone)]
pub struct Field {
    pub name: syn::Ident,
    pub ty: FieldType,
}

impl syn::parse::Parse for Field {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        _ = input.parse::<syn::Token![:]>()?;
        let ty = input.parse()?;
        Ok(Self { name, ty })
    }
}