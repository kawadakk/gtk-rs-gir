use std::fmt::Display;
use std::io::{Result, Write};

use analysis;
use analysis::general::StatusedTypeId;
use analysis::imports::Imports;
use analysis::namespaces;
use config::Config;
use config::derives::Derive;
use env::Env;
use gir_version::VERSION;
use version::Version;
use writer::primitives::tabs;

pub fn start_comments(w: &mut Write, conf: &Config) -> Result<()> {
    if conf.single_version_file.is_some() {
        return start_comments_no_version(w);
    }
    writeln!(
        w,
        "// This file was generated by gir (https://github.com/gtk-rs/gir @ {})
// from gir-files (https://github.com/gtk-rs/gir-files @ {})
// DO NOT EDIT",
        VERSION, conf.girs_version
    )
}

pub fn start_comments_no_version(w: &mut Write) -> Result<()> {
    writeln!(
        w,
        "// This file was generated by gir (https://github.com/gtk-rs/gir)
// from gir-files (https://github.com/gtk-rs/gir-files)
// DO NOT EDIT"
    )
}

pub fn single_version_file(w: &mut Write, conf: &Config) -> Result<()> {
    writeln!(
        w,
        "Generated by gir (https://github.com/gtk-rs/gir @ {})
from gir-files (https://github.com/gtk-rs/gir-files @ {})",
        VERSION, conf.girs_version
    )
}

pub fn uses(w: &mut Write, env: &Env, imports: &Imports) -> Result<()> {
    try!(writeln!(w));
    for (name, &(ref version, ref constraints)) in imports.iter() {
        if constraints.len() == 1 {
            try!(writeln!(w, "#[cfg(feature = \"{}\")]", constraints[0]));
        } else if !constraints.is_empty() {
            try!(writeln!(w, "#[cfg(any({}))]", constraints.iter().map(|c| format!("feature = \"{}\"", c)).collect::<Vec<_>>().join(", ")));
        }

        try!(version_condition(w, env, *version, false, 0));
        try!(writeln!(w, "use {};", name));
    }

    Ok(())
}

fn format_parent_name(
    env: &Env,
    p: &StatusedTypeId,
) -> String {
   if p.type_id.ns_id == namespaces::MAIN {
        p.name.clone()
    } else {
        format!(
            "{krate}::{name}",
            krate = env.namespaces[p.type_id.ns_id].crate_name,
            name = p.name,
        )
    }
}

pub fn define_object_type(
    w: &mut Write,
    env: &Env,
    type_name: &str,
    glib_name: &str,
    glib_class_name: &Option<&str>,
    rust_class_name: &Option<&str>,
    glib_func_name: &str,
    is_interface: bool,
    parents: &[StatusedTypeId],
) -> Result<()> {
    let sys_crate_name = env.main_sys_crate_name();
    let class_name = {
        if let Some(s) = *glib_class_name {
            format!(", {}::{}", sys_crate_name, s)
        } else {
            "".to_string()
        }
    };

    let rust_class_name = {
        if let Some(s) = *rust_class_name {
            format!(", {}", s)
        } else {
            "".to_string()
        }
    };

    let kind_name = if is_interface { "Interface" } else { "Object" };

    let parents: Vec<StatusedTypeId> = parents
        .iter()
        .filter(|p| !p.status.ignored())
        .cloned()
        .collect();

    try!(writeln!(w));
    try!(writeln!(w, "glib_wrapper! {{"));
    if parents.is_empty() {
        try!(writeln!(
            w,
            "\tpub struct {}({}<{}::{}{}{}>);",
            type_name,
            kind_name,
            sys_crate_name,
            glib_name,
            class_name,
            rust_class_name
        ));
    } else if is_interface {
        let prerequisites: Vec<String> = parents
            .iter()
            .map(|p| format_parent_name(env, p))
            .collect();

        try!(writeln!(
            w,
            "\tpub struct {}({}<{}::{}{}{}>) @requires {};",
            type_name,
            kind_name,
            sys_crate_name,
            glib_name,
            class_name,
            rust_class_name,
            prerequisites.join(", ")
        ));
    } else {
        let interfaces: Vec<String> = parents
            .iter()
            .filter(|p| {
                use library::*;

                match *env.library.type_(p.type_id) {
                    Type::Interface { ..} if !p.status.ignored() => true,
                    _ => false,
                }
            })
            .map(|p| format_parent_name(env, p))
            .collect();

        let parents: Vec<String> = parents
            .iter()
            .filter(|p| {
                use library::*;

                match *env.library.type_(p.type_id) {
                    Type::Class { ..} if !p.status.ignored() => true,
                    _ => false,
                }
            })
            .map(|p| format_parent_name(env, p))
            .collect();

        let mut parents_string = String::new();
        if !parents.is_empty() {
            parents_string.push_str(format!(" @extends {}", parents.join(", ")).as_str());
        }

        if !interfaces.is_empty() {
            if !parents.is_empty () {
                parents_string.push(',');
            }
            parents_string.push_str(format!(" @implements {}", interfaces.join(", ")).as_str());
        }

        try!(writeln!(
            w,
            "\tpub struct {}({}<{}::{}{}{}>){};",
            type_name,
            kind_name,
            sys_crate_name,
            glib_name,
            class_name,
            rust_class_name,
            parents_string,
        ));
    }
    try!(writeln!(w));
    try!(writeln!(w, "\tmatch fn {{"));
    try!(writeln!(w, "\t\tget_type => || {}::{}(),", sys_crate_name, glib_func_name));
    try!(writeln!(w, "\t}}"));
    try!(writeln!(w, "}}"));

    Ok(())
}

pub fn define_boxed_type(
    w: &mut Write,
    env: &Env,
    type_name: &str,
    glib_name: &str,
    copy_fn: &str,
    free_fn: &str,
    get_type_fn: &Option<String>,
    derive: &[Derive],
) -> Result<()> {
    let sys_crate_name = env.main_sys_crate_name();
    try!(writeln!(w));
    try!(writeln!(w, "glib_wrapper! {{"));

    try!(derives(w, derive, 1));
    try!(writeln!(
        w,
        "\tpub struct {}(Boxed<{}::{}>);",
        type_name,
        sys_crate_name,
        glib_name
    ));
    try!(writeln!(w));
    try!(writeln!(w, "\tmatch fn {{"));
    try!(writeln!(
        w,
        "\t\tcopy => |ptr| {}::{}(mut_override(ptr)),",
        sys_crate_name,
        copy_fn
    ));
    try!(writeln!(w, "\t\tfree => |ptr| {}::{}(ptr),", sys_crate_name, free_fn));
    if let Some(ref get_type_fn) = *get_type_fn {
        try!(writeln!(w, "\t\tget_type => || {}::{}(),", sys_crate_name, get_type_fn));
    }
    try!(writeln!(w, "\t}}"));
    try!(writeln!(w, "}}"));

    Ok(())
}

pub fn define_auto_boxed_type(
    w: &mut Write,
    env: &Env,
    type_name: &str,
    glib_name: &str,
    get_type_fn: &str,
    derive: &[Derive],
) -> Result<()> {
    let sys_crate_name = env.main_sys_crate_name();
    try!(writeln!(w));
    try!(writeln!(w, "glib_wrapper! {{"));
    try!(derives(w, derive, 1));
    try!(writeln!(
        w,
        "\tpub struct {}(Boxed<{}::{}>);",
        type_name,
        sys_crate_name,
        glib_name
    ));
    try!(writeln!(w));
    try!(writeln!(w, "\tmatch fn {{"));
    try!(writeln!(
        w,
        "\t\tcopy => |ptr| gobject_sys::g_boxed_copy({}::{}(), ptr as *mut _) as *mut {}::{},",
        sys_crate_name, get_type_fn, sys_crate_name, glib_name
    ));
    try!(writeln!(
        w,
        "\t\tfree => |ptr| gobject_sys::g_boxed_free({}::{}(), ptr as *mut _),",
        sys_crate_name, get_type_fn
    ));
    try!(writeln!(w, "\t\tget_type => || {}::{}(),", sys_crate_name, get_type_fn));
    try!(writeln!(w, "\t}}"));
    try!(writeln!(w, "}}"));

    Ok(())
}

pub fn define_shared_type(
    w: &mut Write,
    env: &Env,
    type_name: &str,
    glib_name: &str,
    ref_fn: &str,
    unref_fn: &str,
    get_type_fn: &Option<String>,
    derive: &[Derive],
) -> Result<()> {
    let sys_crate_name = env.main_sys_crate_name();
    try!(writeln!(w));
    try!(writeln!(w, "glib_wrapper! {{"));
    try!(derives(w, derive, 1));
    try!(writeln!(
        w,
        "\tpub struct {}(Shared<{}::{}>);",
        type_name,
        sys_crate_name,
        glib_name
    ));
    try!(writeln!(w));
    try!(writeln!(w, "\tmatch fn {{"));
    try!(writeln!(w, "\t\tref => |ptr| {}::{}(ptr),", sys_crate_name, ref_fn));
    try!(writeln!(w, "\t\tunref => |ptr| {}::{}(ptr),", sys_crate_name, unref_fn));
    if let Some(ref get_type_fn) = *get_type_fn {
        try!(writeln!(w, "\t\tget_type => || {}::{}(),", sys_crate_name, get_type_fn));
    }
    try!(writeln!(w, "\t}}"));
    try!(writeln!(w, "}}"));

    Ok(())
}

pub fn cfg_deprecated(
    w: &mut Write,
    env: &Env,
    deprecated: Option<Version>,
    commented: bool,
    indent: usize,
) -> Result<()> {
    if let Some(s) = cfg_deprecated_string(deprecated, env, commented, indent) {
        try!(writeln!(w, "{}", s));
    }
    Ok(())
}

pub fn cfg_deprecated_string(
    deprecated: Option<Version>,
    env: &Env,
    commented: bool,
    indent: usize,
) -> Option<String> {
    let comment = if commented { "//" } else { "" };
    if env.is_too_low_version(deprecated) {
        Some(format!("{}{}#[deprecated]", tabs(indent), comment))
    } else if let Some(v) = deprecated {
        Some(format!(
            "{}{}#[cfg_attr({}, deprecated)]",
            tabs(indent),
            comment,
            v.to_cfg()
        ))
    } else {
        None
    }
}

pub fn version_condition(
    w: &mut Write,
    env: &Env,
    version: Option<Version>,
    commented: bool,
    indent: usize,
) -> Result<()> {
    if let Some(s) = version_condition_string(env, version, commented, indent) {
        try!(writeln!(w, "{}", s));
    }
    Ok(())
}

pub fn version_condition_string(
    env: &Env,
    version: Option<Version>,
    commented: bool,
    indent: usize,
) -> Option<String> {
    match version {
        Some(v) if v > env.config.min_cfg_version => {
            let comment = if commented { "//" } else { "" };
            Some(format!(
                "{}{}#[cfg(any({}, feature = \"dox\"))]",
                tabs(indent),
                comment,
                v.to_cfg()
            ))
        }
        _ => None,
    }
}

pub fn not_version_condition(
    w: &mut Write,
    version: Option<Version>,
    commented: bool,
    indent: usize,
) -> Result<()> {
    if let Some(v) = version {
        let comment = if commented { "//" } else { "" };
        let s = format!(
            "{}{}#[cfg(any(not({}), feature = \"dox\"))]",
            tabs(indent),
            comment,
            v.to_cfg()
        );
        try!(writeln!(w, "{}", s));
    }
    Ok(())
}

pub fn cfg_condition(
    w: &mut Write,
    cfg_condition: &Option<String>,
    commented: bool,
    indent: usize,
) -> Result<()> {
    let s = cfg_condition_string(cfg_condition, commented, indent);
    if let Some(s) = s {
        try!(writeln!(w, "{}", s));
    }
    Ok(())
}

pub fn cfg_condition_string(
    cfg_condition: &Option<String>,
    commented: bool,
    indent: usize,
) -> Option<String> {
    match cfg_condition.as_ref() {
        Some(v) => {
            let comment = if commented { "//" } else { "" };
            Some(format!(
                "{}{}#[cfg(any({}, feature = \"dox\"))]",
                tabs(indent),
                comment,
                v
            ))
        }
        None => None,
    }
}

pub fn derives(
    w: &mut Write,
    derives: &[Derive],
    indent: usize,
) -> Result<()> {
    for derive in derives {
        let s = match &derive.cfg_condition {
            Some(condition) => format!(
                "#[cfg_attr({}, derive({}))]",
                condition,
                derive.names.join(", ")
            ),
            None => format!("#[derive({})]", derive.names.join(", ")),
        };
        try!(writeln!(w, "{}{}", tabs(indent), s));
    }
    Ok(())
}

pub fn doc_hidden(
    w: &mut Write,
    doc_hidden: bool,
    comment_prefix: &str,
    indent: usize,
) -> Result<()> {
    if doc_hidden {
        writeln!(w, "{}{}#[doc(hidden)]", tabs(indent), comment_prefix)
    } else {
        Ok(())
    }
}

pub fn write_vec<T: Display>(w: &mut Write, v: &[T]) -> Result<()> {
    for s in v {
        try!(writeln!(w, "{}", s));
    }
    Ok(())
}

pub fn declare_default_from_new(
    w: &mut Write,
    env: &Env,
    name: &str,
    functions: &[analysis::functions::Info],
) -> Result<()> {
    if let Some(func) = functions.iter().find(|f| {
        !f.visibility.hidden() && f.name == "new" && f.parameters.rust_parameters.is_empty()
    }) {
        try!(writeln!(w));
        try!(cfg_deprecated(w, env, func.deprecated_version, false, 0));
        try!(version_condition(w, env, func.version, false, 0));
        try!(writeln!(w, "impl Default for {} {{", name));
        try!(writeln!(w, "    fn default() -> Self {{"));
        try!(writeln!(w, "        Self::new()"));
        try!(writeln!(w, "    }}"));
        try!(writeln!(w, "}}"));
    }

    Ok(())
}

/// Escapes string in format suitable for placing inside double quotes.
pub fn escape_string(s: &str) -> String {
    let mut es = String::with_capacity(s.len() * 2);
    let _ = s.chars()
        .map(|c| match c {
            '\"' | '\\' => {
                es.push('\\');
                es.push(c)
            }
            _ => es.push(c),
        })
        .count();
    es
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string(""),
                   "");
        assert_eq!(escape_string("no escaping here"),
                   "no escaping here");
        assert_eq!(escape_string(r#"'"\"#),
                   r#"'\"\\"#);
    }
}
