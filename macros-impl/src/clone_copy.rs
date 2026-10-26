use proc_macro::{TokenStream, TokenTree};

pub fn derive(input: TokenStream) -> TokenStream {
    let mut tokens = input.into_iter();
    let name = loop {
        match tokens.next() {
            Some(TokenTree::Ident(keyword)) if keyword.to_string() == "enum" => {
                break match tokens.next() {
                    Some(TokenTree::Ident(name)) => name,
                    _ => return error("EnumArg requires an enum name"),
                };
            }
            Some(_) => {}
            None => return error("EnumArg can only be derived for enums"),
        }
    };

    format!(
        "impl ::core::marker::Copy for {name} {{}}\n\
         impl ::core::clone::Clone for {name} {{\n\
             fn clone(&self) -> Self {{ *self }}\n\
         }}"
    )
    .parse()
    .expect("generated EnumArg implementation is valid")
}

fn error(message: &str) -> TokenStream {
    format!("::core::compile_error!({message:?});")
        .parse()
        .expect("generated EnumArg error is valid")
}
