use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, punctuated::Punctuated, Expr, Token};

#[allow(non_snake_case)]
pub fn R(item: TokenStream) -> TokenStream {
    // Check if the input is a string.
    let lit = match syn::parse2::<syn::LitStr>(item.clone()) {
        Ok(lit) => lit,
        Err(_) => {
            // If not a string, expand the tokens to make a string.
            let src = format!("{}", item);
            return quote!(eval_string(#src));
        }
    };

    let mut src = lit.value();

    // Replace rust expressions in {{..}} with _expr0, _expr1, ...
    let mut expressions: Punctuated<Expr, Token!(,)> = Punctuated::new();
    while let Some(start) = src.find("{{") {
        if let Some(end) = src[start + 2..].find("}}") {
            if let Ok(param) = syn::parse_str::<Expr>(&src[start + 2..start + 2 + end]) {
                src = format!(
                    "{} param.{} {}",
                    &src[0..start],
                    expressions.len(),
                    &src[start + 2 + end + 2..]
                );
                expressions.push(parse_quote!(&extendr_api::Robj::from(#param)));
            } else {
                return quote!(compile_error!("Not a valid rust expression."));
            }
        } else {
            return quote!(compile_error!("Unterminated {{ block."));
        }
    }

    if expressions.is_empty() {
        quote!(eval_string(#src))
    } else {
        quote!(
            {
                let params = &[#expressions];
                eval_string_with_params(#src, params)
            }
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_r_macro() {
        // Note: strip spaces to cover differences between compilers.

        assert_eq!(
            format!("{}", R(quote!("data.frame"))).replace(" ", ""),
            "eval_string(\"data.frame\")"
        );

        assert_eq!(format!("{}", R(quote!("a <- {{1}}"))).replace(" ", ""),
        "{letparams=&[&extendr_api::Robj::from(1)];eval_string_with_params(\"a<-param.0\",params)}");

        assert_eq!(format!("{}", R(quote!(r"
        a <- 1
        b <- {{1}}
        "))).replace(" ", ""),
        "{letparams=&[&extendr_api::Robj::from(1)];eval_string_with_params(\"\\na<-1\\nb<-param.0\\n\",params)}");
    }
}
