#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::disallowed_methods)]

use magnus::{
    encoding::{EncodingCapable, Index},
    method,
    prelude::*,
    rb_sys::{protect, AsRawId, AsRawValue, FromRawValue},
    value::{Fixnum, Id, LazyId},
    Error, RArray, RString, Ruby, Symbol, Value,
};
use rb_sys::ruby_value_type;
use rinku_core::{Mode, Renderer, Scanner, DEFAULT_SKIP_TAGS};

// These IDs are interned Ruby symbols, not unmarked heap values.
static SKIP_TAGS: LazyId = LazyId::new("@skip_tags");
static ALL: LazyId = LazyId::new("all");
static URLS: LazyId = LazyId::new("urls");
static EMAILS: LazyId = LazyId::new("email_addresses");
static CALL: LazyId = LazyId::new("call");

fn check_type(value: Value, kind: ruby_value_type) -> Result<(), Error> {
    let valid = match kind {
        ruby_value_type::RUBY_T_STRING => RString::from_value(value).is_some(),
        ruby_value_type::RUBY_T_ARRAY => RArray::from_value(value).is_some(),
        ruby_value_type::RUBY_T_SYMBOL => Symbol::from_value(value).is_some(),
        ruby_value_type::RUBY_T_FIXNUM => Fixnum::from_value(value).is_some(),
        _ => false,
    };
    if valid {
        return Ok(());
    }
    // SAFETY: Ruby or Magnus roots `value`. The GVL is held.
    // `rb_check_type` can raise. `protect` catches it before Ruby can skip Rust destructors.
    protect(|| unsafe {
        rb_sys::rb_check_type(value.as_raw(), kind as i32);
        0
    })?;
    Ok(())
}

fn bytes(string: RString) -> Vec<u8> {
    string.to_bytes().into()
}

fn ruby_string(ruby: &Ruby, bytes: &[u8], encoding: Index) -> Result<RString, Error> {
    // `enc_str_new` can allocate in Ruby.
    // Use `protect` so an error does not skip Rust destructors.
    // The closure captures references and `Copy` handles only.
    let raw = protect(|| ruby.enc_str_new(bytes, encoding).as_raw())?;
    // SAFETY: Ruby returned a valid string under the GVL.
    // Root it on this stack before another Ruby call.
    Ok(RString::from_value(unsafe { Value::from_raw(raw) }).unwrap())
}

fn call_link(
    ruby: &Ruby,
    block: magnus::block::Proc,
    bytes: &[u8],
    encoding: Index,
) -> Result<Value, Error> {
    let call = LazyId::get_inner_with(&CALL, ruby);
    // Build the argument and call the Proc in one protected frame.
    // Two Magnus calls would need two frames per link.
    // Captures are references or `Copy` handles. `bytes` points to Rust-owned memory.
    let raw = protect(|| {
        let argument = ruby.enc_str_new(bytes, encoding);
        let arguments = [argument.as_raw()];
        // SAFETY: The GVL is held. Ruby roots `block` and `arguments` during the call and GC.
        // The closure has no Rust values with destructors. `protect` catches allocation errors,
        // exceptions, and nonlocal exits.
        unsafe { rb_sys::rb_funcallv(block.as_raw(), call.as_raw(), 1, arguments.as_ptr()) }
    })?;
    // SAFETY: Ruby returned a valid value. Root it on this stack before another call.
    Ok(unsafe { Value::from_raw(raw) })
}

fn tag_bytes(tags: RArray) -> Result<Vec<Vec<u8>>, Error> {
    let mut result = Vec::with_capacity(tags.len());
    for i in 0..tags.len() {
        let tag: Value = tags.entry(i as isize)?;
        check_type(tag, ruby_value_type::RUBY_T_STRING)?;
        let tag = RString::from_value(tag).unwrap();
        // Use `StringValueCStr` to match C's encoding and NUL checks. Do not coerce the value.
        // SAFETY: The Ruby array and this stack root `tag`. The closure has no Rust values
        // that a Ruby longjmp could skip.
        let mut raw = tag.as_raw();
        #[allow(clippy::disallowed_methods)] // Validation only; discard the returned pointer.
        protect(|| unsafe {
            rb_sys::rb_string_value_cstr(&mut raw);
            0
        })?;
        result.push(bytes(tag));
    }
    Ok(result)
}

// Keep Ruby handles on the native stack.
// The Ruby array keeps the original tag strings visible to GC.
struct Arguments {
    text: RString,
    attrs: Option<RString>,
    tags: Option<RArray>,
    tag_snapshot: Vec<Vec<u8>>,
    mode: Mode,
    flags: u32,
}

fn validate_arguments(ruby: &Ruby, receiver: Value, args: &[Value]) -> Result<Arguments, Error> {
    // Match C's validation order. A later argument error must not replace an earlier one.
    ruby.check_arity(args.len(), 1..=5)?;
    let nil = ruby.qnil().as_value();
    let get = |i| args.get(i).copied().unwrap_or(nil);
    let text = get(0);
    check_type(text, ruby_value_type::RUBY_T_STRING)?;
    let text = RString::from_value(text).unwrap();
    let mode = get(1);
    if !mode.is_nil() {
        check_type(mode, ruby_value_type::RUBY_T_SYMBOL)?;
    }
    let attrs = get(2);
    if !attrs.is_nil() {
        check_type(attrs, ruby_value_type::RUBY_T_STRING)?;
    }
    let attrs = RString::from_value(attrs);
    let mut skip = get(3);
    if skip.is_nil() {
        let id = LazyId::get_inner_with(&SKIP_TAGS, ruby);
        // SAFETY: The VM roots `receiver`, `id` is interned, and `protect` catches Ruby exceptions.
        // Ruby returns a valid `VALUE`.
        let raw = protect(|| unsafe { rb_sys::rb_ivar_get(receiver.as_raw(), id.as_raw()) })?;
        // SAFETY: Ruby returned this `VALUE` while the GVL was held.
        skip = unsafe { Value::from_raw(raw) };
    }
    // Keep a Ruby array, not `Vec<Value>`, so GC marks original tags after callbacks.
    let tags = if skip.is_nil() {
        None
    } else {
        check_type(skip, ruby_value_type::RUBY_T_ARRAY)?;
        let array = RArray::from_value(skip).unwrap();
        // `dup` allocates in Ruby. Catch its exceptions before Rust unwinds.
        let raw = protect(|| array.dup().as_raw())?;
        // SAFETY: Ruby returned an array. Root it on this stack before another call.
        Some(RArray::from_value(unsafe { Value::from_raw(raw) }).unwrap())
    };
    let tag_snapshot = match tags {
        Some(tags) => tag_bytes(tags)?,
        None => Vec::new(),
    };
    let flags = get(4);
    let flags = if flags.is_nil() {
        0
    } else {
        check_type(flags, ruby_value_type::RUBY_T_FIXNUM)?;
        // SAFETY: The type check passed. Protected `FIX2INT` preserves C's range errors.
        // Magnus's integer conversion does not.
        protect(|| unsafe { rb_sys::rb_fix2int(flags.as_raw()) as rb_sys::VALUE })? as u32
    };
    let mode_id = Symbol::from_value(mode).map(Id::from);
    let mode = if mode.is_nil() || mode_id.is_some_and(|id| id == *ALL) {
        Mode::All
    } else if mode_id.is_some_and(|id| id == *URLS) {
        Mode::Urls
    } else if mode_id.is_some_and(|id| id == *EMAILS) {
        Mode::Emails
    } else {
        return Err(Error::new(
            ruby.exception_type_error(),
            "Invalid linking mode (possible values are :all, :urls, :email_addresses)",
        ));
    };
    Ok(Arguments {
        text,
        attrs,
        tags,
        tag_snapshot,
        mode,
        flags,
    })
}

fn without_callback(args: &Arguments) -> Option<Vec<u8>> {
    let input = bytes(args.text);
    let attributes = args.attrs.map(bytes);
    let attributes = attributes.as_deref().map(|a| &a[trim_start(a)..]);
    if args.tags.is_none() {
        rinku_core::auto_link(&input, args.mode, args.flags, attributes, DEFAULT_SKIP_TAGS)
    } else {
        rinku_core::auto_link(
            &input,
            args.mode,
            args.flags,
            attributes,
            &args.tag_snapshot,
        )
    }
}

fn frozen_snapshot(text: RString) -> Result<RString, Error> {
    // SAFETY: The VM or this stack roots `text`. The GVL is held.
    // Ruby can share the buffer with a frozen alias. A later write must detach it.
    // Ruby copies embedded strings instead of sharing their buffers.
    // Call `rb_str_new_frozen` directly to avoid overridden String methods.
    let raw = protect(|| unsafe { rb_sys::rb_str_new_frozen(text.as_raw()) })?;
    // SAFETY: Ruby returned a valid string. Root it on this stack before another call.
    Ok(RString::from_value(unsafe { Value::from_raw(raw) }).unwrap())
}

fn shares_storage(text: RString, snapshot: RString) -> bool {
    // SAFETY: Both handles are rooted on the native stack. The GVL is held.
    // No Ruby allocation or callback occurs between the pointer reads.
    // Compare addresses only. Do not dereference, slice, or retain the pointers.
    #[allow(clippy::disallowed_methods)] // Compare identity only; do not read string bytes.
    unsafe {
        rb_sys::RSTRING_PTR(text.as_raw()) == rb_sys::RSTRING_PTR(snapshot.as_raw())
    }
}

fn refresh_source(
    text: RString,
    snapshot: &mut RString,
    input: &mut [u8],
    unread: usize,
) -> Result<(), Error> {
    // Keep the old snapshot when the source length changes. Scanner offsets use it.
    // Shared storage means the bytes are unchanged: Ruby must detach before a write.
    if text.len() != input.len() || shares_storage(text, *snapshot) {
        return Ok(());
    }
    let current = bytes(text);
    // Refresh unread bytes only. The renderer already owns the emitted prefix.
    input[unread..].copy_from_slice(&current[unread..]);
    *snapshot = frozen_snapshot(text)?;
    Ok(())
}

fn with_callback(
    ruby: &Ruby,
    args: &mut Arguments,
    block: magnus::block::Proc,
) -> Result<Option<Vec<u8>>, Error> {
    let encoding = args.text.enc_get();
    let mut attributes = args.attrs.map(bytes);
    let attr_start = attributes.as_deref().map_or(0, trim_start);
    // For mutable sources, use a frozen alias to detect writes without borrowing Ruby storage.
    // A frozen source needs only its owned byte snapshot.
    let mut source_snapshot = if args.text.is_frozen() {
        None
    } else {
        Some(frozen_snapshot(args.text)?)
    };
    let mut input = bytes(args.text);
    let mut scanner = Scanner::new(args.mode, args.flags);
    let mut renderer = Renderer::default();
    while let Some(link) = if args.tags.is_none() {
        scanner.next(&input, DEFAULT_SKIP_TAGS)
    } else {
        scanner.next(&input, &args.tag_snapshot)
    } {
        renderer.start_link(
            &input,
            &link,
            attributes.as_deref().map(|a| &a[attr_start.min(a.len())..]),
        );
        let result = call_link(ruby, block, &input[link.range.clone()], encoding)?;
        check_type(result, ruby_value_type::RUBY_T_STRING)?;
        let rendered = bytes(RString::from_value(result).unwrap());
        renderer.end_link(&rendered);
        if let Some(snapshot) = &mut source_snapshot {
            refresh_source(args.text, snapshot, &mut input, link.range.end)?;
        }
        if let (Some(attrs), Some(snapshot)) = (args.attrs, &mut attributes) {
            *snapshot = bytes(attrs);
        }
        if let Some(tags) = args.tags {
            // Treat a NUL added to a tag as the C-string terminator, as C does.
            for (i, snapshot) in args.tag_snapshot.iter_mut().enumerate() {
                *snapshot = bytes(tags.entry::<RString>(i as isize)?);
            }
        }
    }
    Ok(renderer.finish(&input))
}

fn auto_link(ruby: &Ruby, receiver: Value, args: &[Value]) -> Result<Value, Error> {
    let mut args = validate_arguments(ruby, receiver, args)?;
    let output = if ruby.block_given() {
        with_callback(ruby, &mut args, ruby.block_proc()?)?
    } else {
        without_callback(&args)
    };
    match output {
        None => Ok(args.text.as_value()),
        // Match C: output uses the source's current encoding.
        // Callback arguments use the encoding captured before scanning.
        Some(output) => Ok(ruby_string(ruby, &output, args.text.enc_get())?.as_value()),
    }
}

fn trim_start(a: &[u8]) -> usize {
    a.iter()
        .take_while(|&&c| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
        .count()
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    for id in [&SKIP_TAGS, &ALL, &URLS, &EMAILS, &CALL] {
        LazyId::force(id, ruby);
    }
    let module = ruby.define_module("Rinku")?;
    // Magnus catches Rust panics at the ABI boundary.
    // `Result` returns Ruby exceptions and nonlocal exits after Rust drops owned buffers.
    module.define_method("auto_link", method!(auto_link, -1))?;
    module.const_set("AUTOLINK_SHORT_DOMAINS", rinku_core::SHORT_DOMAINS)?;
    Ok(())
}
