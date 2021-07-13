extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, ItemImpl};

use crate::wrappers;

/// Handle trait implementations.
///
/// Example:
/// ```ignore
/// use extendr_api::prelude::*;
/// #[derive(Debug)]
/// struct Person {
///     pub name: String,
/// }
/// #[extendr]
/// impl Person {
///     fn new() -> Self {
///         Self { name: "".to_string() }
///     }
///     fn set_name(&mut self, name: &str) {
///         self.name = name.to_string();
///     }
///     fn name(&self) -> &str {
///         self.name.as_str()
///     }
/// }
/// #[extendr]
/// fn aux_func() {
/// }
/// // Macro to generate exports
/// extendr_module! {
///     mod classes;
///     impl Person;
///     fn aux_func;
/// }
/// ```
pub fn extendr_impl(mut item_impl: ItemImpl) -> TokenStream {
    // Only `impl name { }` allowed
    if item_impl.defaultness.is_some() {
        return quote! { compile_error!("default not allowed in #[extendr] impl"); }.into();
    }

    if item_impl.unsafety.is_some() {
        return quote! { compile_error!("unsafe not allowed in #[extendr] impl"); }.into();
    }

    if item_impl.generics.const_params().count() != 0 {
        return quote! { compile_error!("const params not allowed in #[extendr] impl"); }.into();
    }

    if item_impl.generics.type_params().count() != 0 {
        return quote! { compile_error!("type params not allowed in #[extendr] impl"); }.into();
    }

    // if item_impl.generics.lifetimes().count() != 0 {
    //     return quote! { compile_error!("lifetime params not allowed in #[extendr] impl"); }.into();
    // }

    if item_impl.generics.where_clause.is_some() {
        return quote! { compile_error!("where clause not allowed in #[extendr] impl"); }.into();
    }

    let opts = wrappers::ExtendrOptions {};
    let self_ty = item_impl.self_ty.as_ref();
    let self_ty_name = wrappers::type_name(self_ty);
    let prefix = format!("{}__", self_ty_name);
    let mut method_meta_names = Vec::new();
    let doc_string = wrappers::get_doc_string(&item_impl.attrs);

    // Generate wrappers for methods.
    // eg.
    // ```
    // #[no_mangle]
    // #[allow(non_snake_case)]
    // pub extern "C" fn wrap__Person__new() -> extendr_api::SEXP {
    //     unsafe {
    //         use extendr_api::FromRobj;
    //         extendr_api::Robj::from(<Person>::new()).get()
    //     }
    // }
    // ```
    let mut wrappers: Vec<ItemFn> = Vec::new();
    for impl_item in &mut item_impl.items {
        if let syn::ImplItem::Method(ref mut method) = impl_item {
            method_meta_names.push(format_ident!(
                "{}{}__{}",
                wrappers::META_PREFIX,
                self_ty_name,
                method.sig.ident
            ));
            wrappers::make_function_wrappers(
                &opts,
                &mut wrappers,
                prefix.as_str(),
                &method.attrs,
                &method.sig,
                Some(self_ty),
            );
        }
    }

    let meta_name = format_ident!("{}{}", wrappers::META_PREFIX, self_ty_name);

    let finalizer_name = format_ident!("__finalize__{}", self_ty_name);

    let expanded = TokenStream::from(quote! {
        // The impl itself copied from the source.
        #item_impl

        // Function wrappers
        #( #wrappers )*

        //TODO: Implement a version that works for `#self_ty` -- not only reference

        // Input conversion function for this type.
        impl<'a> extendr_api::FromRobj<'a> for &#self_ty {
            fn from_robj(robj: &'a Robj) -> std::result::Result<Self, &'static str> {
                if robj.check_external_ptr(#self_ty_name) {
                    Ok(unsafe { std::mem::transmute(robj.external_ptr_addr::<#self_ty>()) })
                } else {
                    Err(concat!("expected ", #self_ty_name))
                }
            }
        }

        // Input conversion function for a reference to this type.
        impl<'a> extendr_api::FromRobj<'a> for &mut #self_ty {
            fn from_robj(robj: &'a Robj) -> std::result::Result<Self, &'static str> {
                if robj.check_external_ptr(#self_ty_name) {
                    Ok(unsafe { std::mem::transmute(robj.external_ptr_addr::<#self_ty>()) })
                } else {
                    Err(concat!("expected ", #self_ty_name))
                }
            }
        }

        // Output conversion function for this type.
        impl From<#self_ty> for Robj {
            fn from(value: #self_ty) -> Self {
                unsafe {
                    let ptr = Box::into_raw(Box::new(value));
                    let res = Robj::make_external_ptr(ptr, Robj::from(#self_ty_name), Robj::from(()));
                    res.set_attrib(class_symbol(), #self_ty_name).unwrap();
                    res.register_c_finalizer(Some(#finalizer_name));
                    res
                }
            }
        }

        // Output conversion function for this type.
        impl<'a> From<&'a #self_ty> for Robj {
            fn from(value: &'a #self_ty) -> Self {
                unsafe {
                    let ptr = Box::into_raw(Box::new(value));
                    let res = Robj::make_external_ptr(ptr, Robj::from(#self_ty_name), Robj::from(()));
                    res.set_attrib(class_symbol(), #self_ty_name).unwrap();
                    res.register_c_finalizer(Some(#finalizer_name));
                    res
                }
            }
        }

        // Function to free memory for this type.
        #[allow(non_snake_case)]
        extern "C" fn #finalizer_name (sexp: extendr_api::SEXP) {
            unsafe {
                let robj = extendr_api::new_owned(sexp);
                if robj.check_external_ptr(#self_ty_name) {
                    //eprintln!("finalize {}", #self_ty_name);
                    let ptr = robj.external_ptr_addr::<#self_ty>();
                    Box::from_raw(ptr);
                }
            }
        }

        #[allow(non_snake_case)]
        fn #meta_name(impls: &mut Vec<extendr_api::metadata::Impl>) {
            let mut methods = Vec::new();
            #( #method_meta_names(&mut methods); )*
            impls.push(extendr_api::metadata::Impl {
                doc: #doc_string,
                name: #self_ty_name,
                methods,
            });
        }
    });

    //eprintln!("{}", expanded);
    expanded
}

// This structure contains parameters parsed from the #[extendr_module] definition.
