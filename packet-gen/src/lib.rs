// Copyright (c) 2025 Grapple Systems LLC, All rights reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use proc_macro::TokenStream;

mod parsing;
mod embedded_gen;

#[proc_macro]
pub fn packet(input: TokenStream) -> TokenStream {
    let packet = syn::parse_macro_input!(input as parsing::Packet);
    let embedded_packet = embedded_gen::PacketParserGen(&packet);
    quote::quote!{ #embedded_packet }.into()
}
