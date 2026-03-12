// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::parsing;

pub struct PacketParserGen<'a>(pub &'a parsing::Packet);
impl<'a> quote::ToTokens for PacketParserGen<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let maybe_pub = self.0.maybe_pub.as_ref();
        let name = &self.0.name;
        let packet_type = &self.0.packet_type;
        let maybe_response = self.0.response_type.as_ref().map(|name| AllocResponseMethod { response_name: name.clone() });

        let mut idx = 0;
        let mut accessors = Vec::new();
        let mut mutators = Vec::new();
        for field in &self.0.fields {
            accessors.push(NonRepeatFieldAccessorGen(field.clone(), idx, name.clone()));
            mutators.push(FieldMutatorGen { field: field.clone(), idx, packet_name: name.clone() });
            match &field.ty {
                parsing::FieldType::Primitive(ty) => {
                    idx += ty.data_size();
                },
                parsing::FieldType::Array(ty, count) => {
                    idx += ty.data_size() * count;
                },
                _ => (),
            }
        }

        let commit_method = CommitMethod {
            base_size: idx,
            maybe_slice: self.0.fields.last().and_then(|f| {
                match f.ty.clone() {
                    parsing::FieldType::Slice(ty) => Some(ty),
                    _ => None,
                }
            })
        };

        tokens.extend(quote::quote! {
            #maybe_pub struct #name<B> {
                base: BasePacket<B>,
            }

            impl<B: AsRef<[u8]>> TryFrom<BasePacket<B>> for #name<B> {
                type Error = Error;

                fn try_from(base: BasePacket<B>) -> Result<Self, Self::Error> {
                    assert!(base.get_type() == #packet_type);
                    if base.get_payload().len() >= #idx {
                        Ok(Self { base })
                    } else {
                        Err(Self::Error::NotEnoughBytes)
                    }
                }
            }

            impl<B: AsRef<[u8]>> #name<B> {
                #(#accessors)*

                fn get_payload(&self) -> &[u8] {
                    self.base.get_payload()
                }

                #maybe_response
            }

            impl<B: AsMut<[u8]>> #name<B> {
                #(#mutators)*

                pub fn try_alloc(buf: B) -> Result<Self, Error> {
                    let mut base = BasePacket::try_alloc(buf)?;
                    if base.get_payload_mut().len() >= #idx {
                        base.set_type(#packet_type);
                        Ok(Self {base})
                    } else {
                        Err(Error::NotEnoughBytes)
                    }
                }

                #commit_method

                fn get_payload_mut(&mut self) -> &mut [u8] {
                    self.base.get_payload_mut()
                }

                fn commit_with_repeat(self, size: usize) -> usize {
                    self.base.commit(#idx + size)
                }
            }
        });

        if let Some(repeat_field) = self.0.repeat.as_ref() {
            if let parsing::FieldType::Repeat(fields) = &repeat_field.ty {
                let repeat = RepeatBuilder {
                    maybe_pub: self.0.maybe_pub.clone(),
                    packet_name: name.clone(),
                    fields: fields.clone()
                };
                tokens.extend(quote::quote! {
                    #repeat
                });
            }
        }
    }
}

struct AllocResponseMethod {
    response_name: syn::Ident,
}
impl quote::ToTokens for AllocResponseMethod {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let response_name = self.response_name.clone();
        tokens.extend(quote::quote! {
            pub fn try_alloc_response<R: AsMut<[u8]>>(&self, buf: R) -> Result<#response_name<R>, Error> {
                let mut res = #response_name::try_alloc(buf)?;
                res.base.set_id(self.base.get_id());
                Ok(res)
            }
        })
    }
}

struct CommitMethod {
    base_size: usize,
    maybe_slice: Option<parsing::PrimitiveType>,
}
impl quote::ToTokens for CommitMethod {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let base_size = self.base_size;
        if let Some(slice_type) = self.maybe_slice.as_ref() {
            let data_size = slice_type.data_size();
            tokens.extend(quote::quote! {
                pub fn commit(self, count: usize) -> usize {
                    self.base.commit(#base_size + count * #data_size)
                }
            });
        } else {
            tokens.extend(quote::quote! {
                pub fn commit(self) -> usize {
                    self.base.commit(#base_size)
                }
            });
        }
    }
}

struct NonRepeatFieldAccessorGen(parsing::Field, usize, syn::Ident);
impl quote::ToTokens for NonRepeatFieldAccessorGen {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let idx = self.1;
        let fn_name = syn::Ident::new(&format!("get_{}", self.0.name), proc_macro2::Span::call_site());
        match &self.0.ty {
            parsing::FieldType::Primitive(field_type) => {
                let ty = PrimitiveType(field_type.clone());
                let size = field_type.data_size();
                tokens.extend(quote::quote! {
                    pub fn #fn_name(&self) -> #ty {
                        #ty::from_le_bytes(self.get_payload()[#idx..#idx+#size].try_into().unwrap())
                    }
                });
            },
            parsing::FieldType::Array(field_type, count) => {
                if *field_type == parsing::PrimitiveType::Uint8 {
                    // treat a byte array a little differently
                    tokens.extend(quote::quote! {
                        pub fn #fn_name(&self) -> &[u8] {
                            &self.get_payload()[#idx..#idx+#count]
                        }
                    });
                } else {
                    let ty = PrimitiveType(field_type.clone());
                    let size = field_type.data_size();
                    let end_idx = idx + size * count;
                    tokens.extend(quote::quote! {
                        pub fn #fn_name(&self) -> impl ::core::iter::Iterator<Item = #ty> {
                            self.get_payload()[#idx..#end_idx].chunks(#size).
                                map(|chunk| #ty::from_le_bytes(chunk.try_into().unwrap()))
                        }
                    });
                }
            },
            parsing::FieldType::Slice(field_type) => {
                if *field_type == parsing::PrimitiveType::Uint8 {
                    tokens.extend(quote::quote! {
                        pub fn #fn_name(&self) -> &[u8] {
                            &self.get_payload()[#idx..]
                        }
                    });
                } else {
                    let ty = PrimitiveType(field_type.clone());
                    let size = field_type.data_size();
                    tokens.extend(quote::quote! {
                        pub fn #fn_name(&self) -> impl ::core::iter::Iterator<Item = #ty> {
                            self.get_payload()[#idx..].chunks(#size).
                                map(|chunk| #ty::from_le_bytes(chunk.try_into().unwrap()))
                        }
                    });
                }
            },
            parsing::FieldType::Repeat(_) => {
                let name = syn::Ident::new(&format!("{}Repeat", self.2), proc_macro2::Span::call_site());
                tokens.extend(quote::quote! {
                    pub fn #fn_name(&self) -> impl ::core::iter::Iterator<Item=#name> {
                        self.get_repeat_region().chunks(#name::get_size()).
                            map(|c| #name::from_bytes(c))
                    }

                    pub fn get_repeat_region(&self) -> &[u8] {
                        &self.get_payload()[#idx..]
                    }
                });
            },
        }
    }
}

struct FieldMutatorGen {
    field: parsing::Field,
    idx: usize,
    packet_name: syn::Ident,
}
impl quote::ToTokens for FieldMutatorGen {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let idx = self.idx;
        let fn_name = syn::Ident::new(&format!("set_{}", self.field.name), proc_macro2::Span::call_site());
        match &self.field.ty {
            parsing::FieldType::Primitive(field_type) => {
                let ty = PrimitiveType(field_type.clone());
                let size = field_type.data_size();
                tokens.extend(quote::quote! {
                    pub fn #fn_name(&mut self, value: #ty) {
                        self.get_payload_mut()[#idx..#idx+#size].copy_from_slice(&value.to_le_bytes());
                    }
                });
            },
            parsing::FieldType::Array(field_type, count) => {
                if *field_type == parsing::PrimitiveType::Uint8 {
                    let fn_name = syn::Ident::new(&format!("get_{}_mut", self.field.name), proc_macro2::Span::call_site());
                    tokens.extend(quote::quote! {
                        pub fn #fn_name(&mut self) -> &mut [u8] {
                            &mut self.get_payload_mut()[#idx..#idx+#count]
                        }
                    });
                } else {
                    let fn_name = syn::Ident::new(&format!("set_{}", self.field.name), proc_macro2::Span::call_site());
                    let ty = PrimitiveType(field_type.clone());
                    let size = field_type.data_size();
                    let end_idx = idx + size * count;
                    tokens.extend(quote::quote! {
                        pub fn #fn_name(&mut self, values: impl ::core::iter::Iterator<Item=#ty>) {
                            let payload_region = &mut self.get_payload_mut()[#idx..#end_idx];
                            for (buffer, v) in payload_region.chunks_mut(#size).zip(values) {
                                buffer.copy_from_slice(&v.to_le_bytes())
                            }
                        }
                    });
                }
            },
            parsing::FieldType::Slice(field_type) => {
                if *field_type == parsing::PrimitiveType::Uint8 {
                    let fn_name = syn::Ident::new(&format!("get_{}_mut", self.field.name), proc_macro2::Span::call_site());
                    tokens.extend(quote::quote! {
                        pub fn #fn_name(&mut self) -> &mut [u8] {
                            &mut self.get_payload_mut()[#idx..]
                        }
                    });
                } else {
                    let fn_name = syn::Ident::new(&format!("set_{}", self.field.name), proc_macro2::Span::call_site());
                    let ty = PrimitiveType(field_type.clone());
                    let size = field_type.data_size();
                    tokens.extend(quote::quote! {
                        pub fn #fn_name(&mut self, values: impl ::core::iter::Iterator<Item=#ty>) {
                            let payload_region = &mut self.get_payload_mut()[#idx..];
                            for (buffer, v) in payload_region.chunks_mut(#size).zip(values) {
                                buffer.copy_from_slice(&v.to_le_bytes())
                            }
                        }
                    });
                }
            }
            parsing::FieldType::Repeat(_) => {
                let fn_name = syn::Ident::new(&format!("build_{}", self.field.name), proc_macro2::Span::call_site());
                let repeat_builder = syn::Ident::new(&format!("{}RepeatBuilder", self.packet_name), proc_macro2::Span::call_site());
                tokens.extend(quote::quote! {
                    pub fn #fn_name(self) -> #repeat_builder<B> {
                        #repeat_builder {
                            inner: self,
                            count: 0,
                        }
                    }

                    pub fn get_repeat_region_mut(&mut self) -> &mut [u8] {
                        &mut self.get_payload_mut()[#idx..]
                    }
                });
            },
        }
    }
}

struct RepeatFieldParam(parsing::PrimitiveField);
impl quote::ToTokens for RepeatFieldParam {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = self.0.name.clone();
        let ty = PrimitiveType(self.0.ty.clone());
        tokens.extend(quote::quote! {
            #name: #ty
        });
    }
}

struct RepeatFieldFromBytes {
    field: parsing::PrimitiveField,
    idx: usize,
}
impl quote::ToTokens for RepeatFieldFromBytes {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.field.name;
        let ty = PrimitiveType(self.field.ty.clone());
        let idx = self.idx;
        let end_idx = self.idx + self.field.ty.data_size();
        tokens.extend(quote::quote! {
            #name: #ty::from_le_bytes(buf[#idx..#end_idx].try_into().unwrap())
        });
    }
}

struct RepeatFieldCopyBytes {
    field: parsing::PrimitiveField,
    idx: usize,
}
impl quote::ToTokens for RepeatFieldCopyBytes {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.field.name;
        let idx = self.idx;
        let end_idx = self.idx + self.field.ty.data_size();
        tokens.extend(quote::quote! {
            buf[#idx..#end_idx].copy_from_slice(&self.#name.to_le_bytes());
        });
    }
}

struct RepeatField {
    maybe_pub: Option<syn::token::Pub>,
    name: syn::Ident,
    fields: Vec<parsing::PrimitiveField>
}
impl quote::ToTokens for RepeatField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let maybe_pub = self.maybe_pub.as_ref();
        let name = &self.name;
        let mut size = 0;
        let mut from_bytes = Vec::new();
        let mut copy_bytes = Vec::new();
        for field in &self.fields {
            from_bytes.push(RepeatFieldFromBytes{field: field.clone(), idx: size});
            copy_bytes.push(RepeatFieldCopyBytes{field: field.clone(), idx: size});
            size += field.ty.data_size();
        }

        let fields = self.fields.iter().map(|f| RepeatFieldParam(f.clone()));

        tokens.extend(quote::quote! {
            #[derive(Debug, PartialEq)]
            #maybe_pub struct #name {
                #(pub #fields,)*
            }

            impl #name {
                pub fn get_size() -> usize {
                    #size
                }

                pub fn from_bytes(buf: &[u8]) -> Self {
                    Self {
                        #(#from_bytes,)*
                    }
                }

                pub fn copy_to_bytes(&self, buf: &mut [u8]) {
                    #(#copy_bytes)*
                }
            }
        });
    }
}

struct RepeatBuilder {
    maybe_pub: Option<syn::token::Pub>,
    packet_name: syn::Ident,
    fields: Vec<parsing::PrimitiveField>,
}
impl quote::ToTokens for RepeatBuilder {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let maybe_pub = self.maybe_pub.as_ref();
        let name = syn::Ident::new(&format!("{}RepeatBuilder", self.packet_name), proc_macro2::Span::call_site());
        let repeat_name = syn::Ident::new(&format!("{}Repeat", self.packet_name), proc_macro2::Span::call_site());
        let packet_name = self.packet_name.clone();
        let repeat = RepeatField{
            maybe_pub: self.maybe_pub.clone(),
            name: repeat_name.clone(),
            fields: self.fields.clone()
        };
        tokens.extend(quote::quote! {
            #repeat

            #maybe_pub struct #name<B> {
                inner: #packet_name<B>,
                count: usize,
            }

            impl<B: AsMut<[u8]>> #name<B> {
                pub fn get_inner(&mut self) -> &mut #packet_name<B> {
                    &mut self.inner
                }

                pub fn try_add(&mut self, repeat: #repeat_name) -> bool {
                    let idx = self.count * #repeat_name::get_size();
                    let end_idx = idx + #repeat_name::get_size();
                    if let Some(region) = self.inner.get_repeat_region_mut().get_mut(idx..end_idx) {
                        repeat.copy_to_bytes(region);
                        self.count += 1;
                        true
                    } else {
                        false
                    }
                }

                pub fn commit(self) -> usize {
                    self.inner.commit_with_repeat(self.count * #repeat_name::get_size())
                }
            }
        });
    }
}

struct PrimitiveType(parsing::PrimitiveType);
impl quote::ToTokens for PrimitiveType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ty = match &self.0 {
            parsing::PrimitiveType::Uint8 => syn::Ident::new("u8", proc_macro2::Span::call_site()),
            parsing::PrimitiveType::Uint16 => syn::Ident::new("u16", proc_macro2::Span::call_site()),
            parsing::PrimitiveType::Uint32 => syn::Ident::new("u32", proc_macro2::Span::call_site()),
            parsing::PrimitiveType::Float => syn::Ident::new("f32", proc_macro2::Span::call_site()),
        };
        tokens.extend(quote::quote! { #ty });
    }
}
