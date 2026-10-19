// Copyright (c) 2011, Vicent Marti. ISC license; see COPYING.
#![deny(unsafe_op_in_unsafe_fn)]

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

// IDs refer to Ruby's permanent interned symbols, not unmarked heap VALUEs.
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
    // SAFETY: value is a VM-rooted argument or a stack-local Magnus handle.
    // The GVL is held. rb_check_type may raise; protect catches the nonlocal
    // exit before returning to Rust frames with owned allocations.
    protect(|| unsafe {
        rb_sys::rb_check_type(value.as_raw(), kind as i32);
        0
    })?;
    Ok(())
}

fn bytes(string: RString) -> Vec<u8> {
    // SAFETY: called under the GVL with a live, stack-rooted string. No Ruby
    // calls/allocations occur until the slice has been copied and discarded.
    unsafe { string.as_slice() }.to_vec()
}

fn ruby_string(ruby: &Ruby, bytes: &[u8], encoding: Index) -> Result<RString, Error> {
    // Magnus's infallible constructor allocates in Ruby. Protect it as well so
    // even an allocation exception returns through Result and drops buffers.
    // The closure holds only references/Copy handles, no Rust drop resources.
    let raw = protect(|| ruby.enc_str_new(bytes, encoding).as_raw())?;
    // SAFETY: raw is the new, valid Ruby string returned under the GVL. It is
    // immediately materialized on this native stack before any further call.
    Ok(RString::from_value(unsafe { Value::from_raw(raw) }).unwrap())
}

fn call_link(
    ruby: &Ruby,
    block: magnus::block::Proc,
    bytes: &[u8],
    encoding: Index,
) -> Result<Value, Error> {
    let call = LazyId::get_inner_with(&CALL, ruby);
    // Fuse argument allocation and Proc#call in one protected frame. Separate
    // Magnus calls would need two rb_protect frames per link. All captured
    // resources are Copy handles/references; bytes points to owned Rust memory.
    let raw = protect(|| {
        let argument = ruby.enc_str_new(bytes, encoding);
        let arguments = [argument.as_raw()];
        // SAFETY: GVL held; block is a stack-rooted Proc, call is an interned
        // ID. arguments is a native-stack array and Ruby roots its contents
        // during dispatch/GC. No Rust drop resources exist in this closure.
        // protect catches allocation errors, exceptions, and nonlocal exits.
        unsafe { rb_sys::rb_funcallv(block.as_raw(), call.as_raw(), 1, arguments.as_ptr()) }
    })?;
    // SAFETY: raw is a valid VM result, immediately rooted on this stack.
    Ok(unsafe { Value::from_raw(raw) })
}

fn tag_bytes(tags: RArray) -> Result<Vec<Vec<u8>>, Error> {
    let mut result = Vec::with_capacity(tags.len());
    for i in 0..tags.len() {
        let tag: Value = tags.entry(i as isize)?;
        check_type(tag, ruby_value_type::RUBY_T_STRING)?;
        let tag = RString::from_value(tag).unwrap();
        // StringValueCStr's encoding and NUL checks, without accepting coercion.
        // SAFETY: tag is rooted by tags and the stack. The protected closure
        // contains no Rust resources that a Ruby longjmp could bypass.
        let mut raw = tag.as_raw();
        protect(|| unsafe {
            rb_sys::rb_string_value_cstr(&mut raw);
            0
        })?;
        result.push(bytes(tag));
    }
    Ok(result)
}

fn auto_link(ruby: &Ruby, receiver: Value, args: &[Value]) -> Result<Value, Error> {
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
        // SAFETY: receiver is rooted by the VM, id is a valid interned ID,
        // and protect catches any VM exception. The returned VALUE is valid.
        let raw = protect(|| unsafe { rb_sys::rb_ivar_get(receiver.as_raw(), id.as_raw()) })?;
        // SAFETY: raw was returned by Ruby while holding the GVL.
        skip = unsafe { Value::from_raw(raw) };
    }
    // A Ruby array, not Vec<Value>: original tag objects must remain marked
    // even when a callback removes them from the caller's array or compacts.
    let tags = if skip.is_nil() {
        None
    } else {
        check_type(skip, ruby_value_type::RUBY_T_ARRAY)?;
        let array = RArray::from_value(skip).unwrap();
        Some(array.dup())
    };
    let mut tag_snapshot = match tags {
        Some(tags) => tag_bytes(tags)?,
        None => Vec::new(),
    };
    let flags = get(4);
    let flags = if flags.is_nil() {
        0
    } else {
        check_type(flags, ruby_value_type::RUBY_T_FIXNUM)?;
        // SAFETY: strict Fixnum check above; protected FIX2INT preserves C's
        // range checks/messages, unlike Magnus's Rust integer conversion.
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
    let encoding = text.enc_get();
    let block = if ruby.block_given() {
        Some(ruby.block_proc()?)
    } else {
        None
    };
    let output = if let Some(block) = block {
        let mut attributes = attrs.map(bytes);
        let attr_start = attributes.as_deref().map_or(0, trim_start);
        let mut input = bytes(text);
        let mut scanner = Scanner::new(mode, flags);
        let mut renderer = Renderer::default();
        while let Some(link) = if tags.is_none() {
            scanner.next(&input, DEFAULT_SKIP_TAGS)
        } else {
            scanner.next(&input, &tag_snapshot)
        } {
            renderer.start_link(
                &input,
                &link,
                attributes.as_deref().map(|a| &a[attr_start.min(a.len())..]),
            );
            let result = call_link(ruby, block, &input[link.range.clone()], encoding)?;
            check_type(result, ruby_value_type::RUBY_T_STRING)?;
            let rendered = RString::from_value(result).unwrap();
            // SAFETY: result is live on this native stack, GVL held. Copy
            // straight into the owned renderer before any further Ruby call.
            renderer.end_link(unsafe { rendered.as_slice() });
            // Same-length in-place edits have defined behavior in C. Refresh
            // snapshots after the callback, never holding Ruby buffer pointers.
            // Resizing the source had undefined C behavior: retain the last
            // stable source snapshot for that case instead.
            // SAFETY: VM-rooted text, no Ruby calls until this copy completes.
            let current = unsafe { text.as_slice() };
            if current.len() == input.len() {
                // Emitted bytes cannot be scanned, rewound into, or copied
                // again. Only the unread suffix can affect later output.
                let unread = link.range.end;
                if current[unread..] != input[unread..] {
                    input[unread..].copy_from_slice(&current[unread..]);
                }
            }
            if let (Some(attrs), Some(snapshot)) = (attrs, &mut attributes) {
                refresh(attrs, snapshot);
            }
            if let Some(tags) = tags {
                // Mutated tag bytes are C strings; new NULs safely terminate.
                for (i, snapshot) in tag_snapshot.iter_mut().enumerate() {
                    refresh(tags.entry::<RString>(i as isize)?, snapshot);
                }
            }
        }
        renderer.finish(&input)
    } else {
        // SAFETY: text is rooted on the Ruby VM stack and in this native frame.
        // The entire core call is Ruby-free under the GVL. Rust allocations do
        // not invoke Ruby GC; no borrowed memory survives this expression.
        let input = unsafe { text.as_slice() };
        // SAFETY: same Ruby-free region as input; attrs is VM-rooted in args.
        let attributes = attrs.as_ref().map(|a| unsafe { a.as_slice() });
        let attributes = attributes.map(|a| &a[trim_start(a)..]);
        if tags.is_none() {
            rinku_core::auto_link(input, mode, flags, attributes, DEFAULT_SKIP_TAGS)
        } else {
            rinku_core::auto_link(input, mode, flags, attributes, &tag_snapshot)
        }
    };
    match output {
        None => Ok(text.as_value()),
        // C copies the source's *current* encoding on output, while callback
        // arguments use the encoding captured before scanning.
        Some(output) => Ok(ruby_string(ruby, &output, text.enc_get())?.as_value()),
    }
}

fn trim_start(a: &[u8]) -> usize {
    a.iter()
        .take_while(|&&c| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
        .count()
}

fn refresh(string: RString, snapshot: &mut Vec<u8>) {
    // SAFETY: live stack/rooted array handle under the GVL; no Ruby calls
    // occur while the buffer is borrowed. Rust allocation cannot invoke GC.
    let current = unsafe { string.as_slice() };
    snapshot.clear();
    snapshot.extend_from_slice(current);
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    for id in [&SKIP_TAGS, &ALL, &URLS, &EMAILS, &CALL] {
        LazyId::force(id, ruby);
    }
    let module = ruby.define_module("Rinku")?;
    // method! and init catch Rust panics at the native ABI boundary. All Ruby
    // exceptions/nonlocal exits return through Result, dropping owned buffers.
    module.define_method("auto_link", method!(auto_link, -1))?;
    module.const_set("AUTOLINK_SHORT_DOMAINS", rinku_core::SHORT_DOMAINS)?;
    Ok(())
}
