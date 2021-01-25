//! Module metadata
//!
//! This data is returned by get_module_metadata()
//! which is generated by [extendr_module!].
use crate::*;
use std::io::Write;

/// Metadata function argument.
#[derive(Debug, PartialEq)]
pub struct Arg {
    pub name: &'static str,
    pub arg_type: &'static str,
}

/// Metadata function.
#[derive(Debug, PartialEq)]
pub struct Func {
    pub doc: &'static str,
    pub name: &'static str,
    pub args: Vec<Arg>,
    pub return_type: &'static str,
    pub func_ptr: *const u8,
    pub hidden: bool,
}

/// Metadata Impl.
#[derive(Debug, PartialEq)]
pub struct Impl {
    pub doc: &'static str,
    pub name: &'static str,
    pub methods: Vec<Func>,
}

/// Module metadata.
#[derive(Debug, PartialEq)]
pub struct Metadata {
    pub name: &'static str,
    pub functions: Vec<Func>,
    pub impls: Vec<Impl>,
}

impl From<Arg> for Robj {
    fn from(val: Arg) -> Self {
        let res: Robj = List(&[r!(val.name), r!(val.arg_type)]).into();
        res.set_attrib(names_symbol(), r!(["name", "arg_type"]));
        res
    }
}

impl From<Func> for Robj {
    fn from(val: Func) -> Self {
        let res: Robj = List(&[
            r!(val.doc),
            r!(val.name),
            r!(List(val.args)),
            r!(val.return_type),
            r!(val.hidden),
        ])
        .into();
        res.set_attrib(
            names_symbol(),
            r!(["doc", "name", "args", "return.type", "hidden"]),
        );
        res
    }
}

impl From<Impl> for Robj {
    fn from(val: Impl) -> Self {
        let res: Robj = List(&[r!(val.doc), r!(val.name), r!(List(val.methods))]).into();
        res.set_attrib(names_symbol(), r!(["doc", "name", "methods"]));
        res
    }
}

impl From<Metadata> for Robj {
    fn from(val: Metadata) -> Self {
        let res: Robj = List(&[r!(val.name), r!(List(val.functions)), r!(List(val.impls))]).into();
        res.set_attrib(names_symbol(), r!(["name", "functions", "impls"]));
        res
    }
}

fn write_doc(w: &mut Vec<u8>, doc: &str) -> std::io::Result<()> {
    if !doc.is_empty() {
        write!(w, "#'")?;
        for c in doc.chars() {
            if c == '\n' {
                write!(w, "\n#'")?;
            } else {
                write!(w, "{}", c)?;
            }
        }
        writeln!(w, "")?;
    }
    Ok(())
}

/// Wraps invalid R identifers, like `_function_name`, into back quotes.
fn sanitize_identifier(name : &str) -> String{
    match name.chars().next() {
        Some('_') => format!("`{}`", name),
        _ => name.to_string()
    }
}

/// Generate a wrapper for a non-method function.
fn write_function_wrapper(
    w: &mut Vec<u8>,
    func: &Func,
    package_name: &str,
    use_symbols: bool,
) -> std::io::Result<()> {
    if func.hidden {
        return Ok(());
    }

    write_doc(w, func.doc)?;

    let args = func
        .args
        .iter()
        .map(|arg| sanitize_identifier(arg.name))
        .collect::<Vec<_>>()
        .join(", ");

    write!(w, "{} <- function({}) .Call(", sanitize_identifier(func.name), args)?;

    if use_symbols {
        write!(w, "wrap__{}", func.name)?;
    } else {
        write!(w, "\"wrap__{}\"", func.name)?;
    }

    if !func.args.is_empty() {
        write!(w, ", {}", args)?;
    }

    if !use_symbols {
        write!(w, ", PACKAGE = \"{}\"", package_name)?;
    }

    writeln!(w, ")\n")?;

    Ok(())
}

/// Generate a wrapper for a method.
fn write_method_wrapper(
    w: &mut Vec<u8>,
    func: &Func,
    package_name: &str,
    use_symbols: bool,
    class_name: &str,
) -> std::io::Result<()> {
    if func.hidden {
        return Ok(());
    }

    let actual_args = func.args.iter().map(|arg| sanitize_identifier(arg.name)).collect::<Vec<_>>();
    let formal_args = if !actual_args.is_empty() && actual_args[0] == "self" {
        // Skip a leading "self" argument.
        // This is supplied by the environment.
        actual_args
            .iter()
            .skip(1)
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
    } else {
        actual_args.clone()
    };

    let formal_args = formal_args.join(", ");
    let actual_args = actual_args.join(", ");

    write!(
        w,
        "{}${} <- function({}) .Call(",
        class_name,
        sanitize_identifier(func.name), 
        formal_args
    )?;

    if use_symbols {
        write!(w, "wrap__{}__{}", class_name, func.name)?;
    } else {
        write!(w, "\"wrap__{}__{}\"", class_name, func.name)?;
    }

    if !actual_args.is_empty() {
        write!(w, ", {}", actual_args)?;
    }

    if !use_symbols {
        write!(w, ", PACKAGE = \"{}\"", package_name)?;
    }

    writeln!(w, ")\n")?;

    Ok(())
}

/// Generate a wrapper for an implementation block.
fn write_impl_wrapper(
    w: &mut Vec<u8>,
    imp: &Impl,
    package_name: &str,
    use_symbols: bool,
) -> std::io::Result<()> {
    let exported = imp.doc.contains("@export");

    write_doc(w, imp.doc)?;

    let imp_name = sanitize_identifier(imp.name);
    writeln!(w, "{} <- new.env(parent = emptyenv())\n", imp_name)?;

    for func in &imp.methods {
        // write_doc(& mut w, func.doc)?;
        write_method_wrapper(w, func, package_name, use_symbols, &imp_name)?;
    }

    if exported {
        writeln!(w, "#' @rdname {}", imp_name)?;
        writeln!(w, "#' @usage NULL")?;
        writeln!(w, "#' @export")?;
    }
    writeln!(w, "`$.{}` <- function (self, name) {{ func <- {}[[name]]; environment(func) <- environment(); func }}\n", imp_name, imp_name)?;

    Ok(())
}

impl Metadata {
    pub fn make_r_wrappers(
        &self,
        use_symbols: bool,
        package_name: &str,
    ) -> std::io::Result<String> {
        let mut w = Vec::new();

        writeln!(
            w,
            r#"# Generated by extendr: Do not edit by hand
#
# This file was created with the following call:
#   .Call("wrap__make_{}_wrappers", use_symbols = {}, package_name = "{}")
"#,
            self.name,
            if use_symbols { "TRUE" } else { "FALSE" },
            package_name
        )?;

        if use_symbols {
            writeln!(w, "#' @docType package")?;
            writeln!(w, "#' @usage NULL")?;
            writeln!(w, "#' @useDynLib {}, .registration = TRUE", package_name)?;
            writeln!(w, "NULL")?;
            writeln!(w, "")?;
        }

        for func in &self.functions {
            write_function_wrapper(&mut w, func, package_name, use_symbols)?;
        }

        for imp in &self.impls {
            write_impl_wrapper(&mut w, imp, package_name, use_symbols)?;
        }
        unsafe { Ok(String::from_utf8_unchecked(w)) }
    }
}
