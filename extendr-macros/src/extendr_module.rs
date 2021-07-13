use crate::wrappers;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::ParseStream, parse_macro_input, Ident, Token, Type};

pub fn extendr_module(item: TokenStream) -> TokenStream {
    let module = parse_macro_input!(item as Module);
    let Module {
        modname,
        fnnames,
        implnames,
        usenames,
    } = module;
    let modname = modname.unwrap();
    let modname_string = modname.to_string();
    let module_init_name = format_ident!("R_init_{}_extendr", modname);

    let module_metadata_name = format_ident!("get_{}_metadata", modname);
    let module_metadata_name_string = module_metadata_name.to_string();
    let wrap_module_metadata_name =
        format_ident!("{}get_{}_metadata", wrappers::WRAP_PREFIX, modname);

    let make_module_wrappers_name = format_ident!("make_{}_wrappers", modname);
    let make_module_wrappers_name_string = make_module_wrappers_name.to_string();
    let wrap_make_module_wrappers =
        format_ident!("{}make_{}_wrappers", wrappers::WRAP_PREFIX, modname);

    let fnmetanames = fnnames
        .iter()
        .map(|id| format_ident!("{}{}", wrappers::META_PREFIX, id));
    let implmetanames = implnames
        .iter()
        .map(|id| format_ident!("{}{}", wrappers::META_PREFIX, wrappers::type_name(id)));
    let usemetanames = usenames
        .iter()
        .map(|id| format_ident!("get_{}_metadata", id))
        .collect::<Vec<Ident>>();

    TokenStream::from(quote! {
        #[no_mangle]
        #[allow(non_snake_case)]
        pub fn #module_metadata_name() -> extendr_api::metadata::Metadata {
            let mut functions = Vec::new();
            let mut impls = Vec::new();

            // Pushes metadata (eg. extendr_api::metadata::Func) to functions and impl vectors.
            #( #fnmetanames(&mut functions); )*
            #( #implmetanames(&mut impls); )*

            // Extends functions and impls with the submodules metadata
            #( functions.extend(#usemetanames().functions); )*
            #( impls.extend(#usemetanames().impls); )*

            // Add this function to the list, but set hidden: true.
            functions.push(extendr_api::metadata::Func {
                doc: "Metadata access function.",
                name: #module_metadata_name_string,
                args: Vec::new(),
                return_type: "Metadata",
                func_ptr: #wrap_module_metadata_name as * const u8,
                hidden: true,
            });

            // Add this function to the list, but set hidden: true.
            functions.push(extendr_api::metadata::Func {
                doc: "Wrapper generator.",
                name: #make_module_wrappers_name_string,
                args: vec![
                    extendr_api::metadata::Arg { name: "use_symbols", arg_type: "bool" },
                    extendr_api::metadata::Arg { name: "package_name", arg_type: "&str" },
                    ],
                return_type: "String",
                func_ptr: #wrap_make_module_wrappers as * const u8,
                hidden: true,
            });

            extendr_api::metadata::Metadata {
                name: #modname_string,
                functions,
                impls,
            }
        }

        #[no_mangle]
        #[allow(non_snake_case)]
        pub extern "C" fn #wrap_module_metadata_name() -> extendr_api::SEXP {
            unsafe { extendr_api::Robj::from(#module_metadata_name()).get() }
        }

        #[no_mangle]
        #[allow(non_snake_case)]
        pub extern "C" fn #wrap_make_module_wrappers(
            use_symbols_sexp: extendr_api::SEXP,
            package_name_sexp: extendr_api::SEXP,
        ) -> extendr_api::SEXP {
            unsafe {
                use extendr_api::robj::*;
                let robj = new_owned(use_symbols_sexp);
                let use_symbols: bool = <bool>::from_robj(&robj).unwrap();

                let robj = new_owned(package_name_sexp);
                let package_name: &str = <&str>::from_robj(&robj).unwrap();

                extendr_api::Robj::from(
                    #module_metadata_name()
                        .make_r_wrappers(
                            use_symbols,
                            package_name,
                        ).unwrap()
                ).get()
            }
        }

        #[no_mangle]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn #module_init_name(info: * mut extendr_api::DllInfo) {
            extendr_api::register_call_methods(info, #module_metadata_name());
        }
    })
}

#[derive(Debug)]
struct Module {
    modname: Option<Ident>,
    fnnames: Vec<Ident>,
    implnames: Vec<Type>,
    usenames: Vec<Ident>,
}

// Custom parser for the module.
impl syn::parse::Parse for Module {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        use syn::spanned::Spanned;
        let mut res = Self {
            modname: None,
            fnnames: Vec::new(),
            implnames: Vec::new(),
            usenames: Vec::new(),
        };
        while !input.is_empty() {
            if let Ok(kmod) = input.parse::<Token![mod]>() {
                let name: Ident = input.parse()?;
                if res.modname.is_some() {
                    return Err(syn::Error::new(kmod.span(), "only one mod allowed"));
                }
                res.modname = Some(name);
            } else if input.parse::<Token![fn]>().is_ok() {
                res.fnnames.push(input.parse()?);
            } else if input.parse::<Token![impl]>().is_ok() {
                res.implnames.push(input.parse()?);
            } else if input.parse::<Token![use]>().is_ok() {
                res.usenames.push(input.parse()?);
            } else {
                return Err(syn::Error::new(input.span(), "expected mod, fn or impl"));
            }

            input.parse::<Token![;]>()?;
        }
        if res.modname.is_none() {
            return Err(syn::Error::new(input.span(), "expected one 'mod name'"));
        }
        Ok(res)
    }
}
