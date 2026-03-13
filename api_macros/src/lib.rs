use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use syn::visit_mut::{self, VisitMut};
use syn::{
    Expr, ExprCall, ExprMethodCall, ExprPath, Ident, Item, ItemFn, ItemMod, Path, Stmt,
    parse_macro_input, parse_quote,
};

#[proc_macro_attribute]
pub fn use_backend(attr: TokenStream, item: TokenStream) -> TokenStream {
    let backend = if attr.is_empty() {
        "opengl".to_string()
    } else {
        parse_macro_input!(attr as Ident).to_string()
    };
    let crate_path = resolved_api_path();

    let output = match parse_macro_input!(item as Item) {
        Item::Fn(item_fn) => expand_function(item_fn, &backend, &crate_path),
        Item::Mod(item_mod) => expand_module(item_mod, &backend, &crate_path),
        other => {
            return syn::Error::new_spanned(
                other,
                "#[use_backend(...)] supports functions or inline modules",
            )
            .to_compile_error()
            .into();
        }
    };

    output.into()
}

fn expand_function(item_fn: ItemFn, backend: &str, crate_path: &Path) -> proc_macro2::TokenStream {
    let mut item_fn = item_fn;
    rewrite_function(&mut item_fn, backend, crate_path);
    quote! { #item_fn }
}

fn expand_module(item_mod: ItemMod, backend: &str, crate_path: &Path) -> proc_macro2::TokenStream {
    let mut item_mod = item_mod;
    let Some((_, items)) = item_mod.content.as_mut() else {
        return syn::Error::new_spanned(
            item_mod,
            "#[use_backend(...)] requires an inline module body",
        )
        .to_compile_error();
    };

    for item in items.iter_mut() {
        match item {
            Item::Fn(item_fn) => rewrite_function(item_fn, backend, crate_path),
            Item::Mod(nested_mod) => {
                let expanded = expand_module(nested_mod.clone(), backend, crate_path);
                if let Ok(parsed) = syn::parse2::<ItemMod>(expanded) {
                    *nested_mod = parsed;
                }
            }
            _ => {}
        }
    }

    quote! { #item_mod }
}

fn rewrite_function(item_fn: &mut ItemFn, backend: &str, crate_path: &Path) {
    if backend == "opengl" {
        GlCompatibilityRewriter.visit_block_mut(&mut item_fn.block);
    }
    let backend_ident = Ident::new(backend, proc_macro2::Span::call_site());

    let prelude_stmt: Stmt = parse_quote! {
        use #crate_path::syntax::#backend_ident::*;
    };
    let ctx_stmt: Stmt = parse_quote! {
        let mut ctx = #crate_path::syntax::#backend_ident::BackendContext::new();
    };

    item_fn.block.stmts.insert(0, ctx_stmt);
    item_fn.block.stmts.insert(0, prelude_stmt);
}

fn resolved_api_path() -> Path {
    match crate_name("nu") {
        Ok(FoundCrate::Itself) => parse_quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            parse_quote!(::#ident)
        }
        Err(_) => parse_quote!(::nu),
    }
}

struct GlCompatibilityRewriter;

impl VisitMut for GlCompatibilityRewriter {
    fn visit_expr_mut(&mut self, node: &mut Expr) {
        if let Some(replacement) = rewrite_gl_free_function(node) {
            *node = replacement;
            return;
        }
        visit_mut::visit_expr_mut(self, node);
    }

    fn visit_expr_method_call_mut(&mut self, node: &mut ExprMethodCall) {
        visit_mut::visit_expr_method_call_mut(self, node);
        if let Expr::Path(ExprPath { path, .. }) = node.receiver.as_mut() {
            if path.is_ident("gl") {
                *path = parse_quote!(ctx);
            }
        }
    }
}

fn rewrite_gl_free_function(node: &Expr) -> Option<Expr> {
    let Expr::Call(ExprCall { func, args, .. }) = node else {
        return None;
    };
    let Expr::Path(ExprPath { path, .. }) = func.as_ref() else {
        return None;
    };
    let ident = path.get_ident()?;
    let args = args.clone().into_iter().collect::<Vec<_>>();

    match (ident.to_string().as_str(), args.as_slice()) {
        ("glClearColor", [r, g, b, a]) => Some(parse_quote!(ctx.clear_color(#r, #g, #b, #a))),
        ("glClear", [flags]) => Some(parse_quote!(ctx.clear(#flags))),
        ("glUseProgram", [shader]) => Some(parse_quote!(ctx.use_program(#shader))),
        ("glBindVertexArray", [mesh]) => Some(parse_quote!(ctx.bind_vertex_array(#mesh))),
        ("glBindFramebuffer", [_, framebuffer]) => {
            Some(parse_quote!(ctx.bind_framebuffer(#framebuffer)))
        }
        ("glBindBuffer", [target, buffer]) => Some(parse_quote!(ctx.bind_buffer(#target, #buffer))),
        ("glBindBufferBase", [target, index, buffer]) => {
            Some(parse_quote!(ctx.bind_buffer_base(#target, #index, #buffer)))
        }
        ("glBufferData", [target, size, data, usage]) => {
            Some(parse_quote!(ctx.buffer_data(#target, (#size) as u64, (#data) as usize, #usage)))
        }
        ("glBufferSubData", [target, offset, size, data]) => Some(parse_quote!(
            ctx.buffer_sub_data(#target, (#offset) as u64, (#size) as u64, (#data) as usize)
        )),
        ("glBindTexture", [_, texture]) => Some(parse_quote!(ctx.bind_texture_2d(#texture))),
        ("glActiveTexture", [slot]) => Some(parse_quote!(ctx.active_texture(#slot))),
        ("glFramebufferTexture2D", [_, attachment, _, texture, level]) => Some(parse_quote!(
            ctx.framebuffer_texture_2d(#attachment, #texture, (#level) as i32)
        )),
        ("glFramebufferRenderbuffer", [_, attachment, _, renderbuffer]) => Some(parse_quote!(
            ctx.framebuffer_renderbuffer(#attachment, #renderbuffer)
        )),
        ("glUniformMatrix4fv", [name, value]) => {
            Some(parse_quote!(ctx.uniform_mat4(#name, #value)))
        }
        ("glUniform3fv", [name, value]) => Some(parse_quote!(ctx.uniform_vec3(#name, #value))),
        ("glVertexAttribPointer", [index, size, attrib_type, normalized, stride, offset]) => {
            Some(parse_quote!(ctx.vertex_attrib_pointer(
                #index,
                #size,
                #attrib_type,
                #normalized,
                #stride,
                (#offset) as u64,
            )))
        }
        ("glEnableVertexAttribArray", [index]) => {
            Some(parse_quote!(ctx.enable_vertex_attrib_array(#index)))
        }
        ("glDisableVertexAttribArray", [index]) => {
            Some(parse_quote!(ctx.disable_vertex_attrib_array(#index)))
        }
        ("glVertexAttribDivisor", [index, divisor]) => {
            Some(parse_quote!(ctx.vertex_attrib_divisor(#index, #divisor)))
        }
        ("glGenBuffers", [count, ids]) => Some(parse_quote!(ctx.gen_buffers(#count, #ids))),
        ("glDeleteBuffers", [count, ids]) => Some(parse_quote!(ctx.delete_buffers(#count, #ids))),
        ("glGenTextures", [count, ids]) => Some(parse_quote!(ctx.gen_textures(#count, #ids))),
        ("glDeleteTextures", [count, ids]) => Some(parse_quote!(ctx.delete_textures(#count, #ids))),
        ("glGenVertexArrays", [count, ids]) => {
            Some(parse_quote!(ctx.gen_vertex_arrays(#count, #ids)))
        }
        ("glDeleteVertexArrays", [count, ids]) => {
            Some(parse_quote!(ctx.delete_vertex_arrays(#count, #ids)))
        }
        ("glGenFramebuffers", [count, ids]) => {
            Some(parse_quote!(ctx.gen_framebuffers(#count, #ids)))
        }
        ("glDeleteFramebuffers", [count, ids]) => {
            Some(parse_quote!(ctx.delete_framebuffers(#count, #ids)))
        }
        ("glGenRenderbuffers", [count, ids]) => {
            Some(parse_quote!(ctx.gen_renderbuffers(#count, #ids)))
        }
        ("glDeleteRenderbuffers", [count, ids]) => {
            Some(parse_quote!(ctx.delete_renderbuffers(#count, #ids)))
        }
        ("glDrawElements", [mode, count, index_type, offset]) => {
            Some(parse_quote!(ctx.draw_elements_typed(#mode, #count, #index_type, #offset)))
        }
        ("glDrawArrays", [mode, first, count]) => {
            Some(parse_quote!(ctx.draw_arrays(#mode, #first, #count)))
        }
        ("glEnable", [flag]) => Some(parse_quote!(ctx.enable(#flag))),
        ("glDisable", [flag]) => Some(parse_quote!(ctx.disable(#flag))),
        ("glViewport", [x, y, width, height]) => {
            Some(parse_quote!(ctx.viewport(#x, #y, #width, #height)))
        }
        _ => None,
    }
}
