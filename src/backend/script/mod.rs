//! JavaScript pre-processing scripts, executed with the pure-Rust Boa engine.
//!
//! Contract (as in the previous version): a script may define
//! `process(payload, topic)` (sync or async); its return value replaces the
//! payload. Objects are JSON-serialized, `undefined`/`null` keep the input.
//! Without a `process` function the script body itself runs and the payload
//! is passed through. The server's environment variables are available as
//! `env.NAME`, `env.get(name)`, `env.replace(text)` and `env.all()`.
//!
//! New in this version: `payloadBytes` holds the payload's bytes (binary
//! messages are not readable from the lossy `payload` string), returning a
//! `Uint8Array` makes the payload those bytes, and `zlib.*` compresses.

pub mod crypto;
pub mod zlib;

use super::env::{self, EnvVars};
use boa_engine::object::ObjectInitializer;
use boa_engine::object::builtins::{JsArrayBuffer, JsPromise, JsUint8Array};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsString, JsValue, NativeFunction, Source, js_string};
use std::borrow::Cow;
use std::cell::RefCell;

/// Guards against runaway scripts (`while (true) {}`) freezing a worker.
const LOOP_ITERATION_LIMIT: u64 = 5_000_000;
const RECURSION_LIMIT: usize = 400;

thread_local! {
    static CONSOLE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Environment variables of the running script (for `env.*`).
    static ENV: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

/// A payload going through scripts: text, or bytes (a received message, or
/// what a script returned as a `Uint8Array`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    Text(String),
    Bytes(Vec<u8>),
}

/// How a payload's text stands for its bytes: UTF-8, or HEX for a message
/// published in HEX format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextForm {
    Utf8,
    Hex,
}

impl Payload {
    /// The `payload` string of a script.
    fn text(&self, form: TextForm) -> Cow<'_, str> {
        match (self, form) {
            (Payload::Text(text), _) => Cow::Borrowed(text),
            (Payload::Bytes(bytes), TextForm::Utf8) => String::from_utf8_lossy(bytes),
            (Payload::Bytes(bytes), TextForm::Hex) => Cow::Owned(crypto::hex_encode(bytes)),
        }
    }

    /// The `payloadBytes` of a script. HEX text that an earlier script broke
    /// counts as its UTF-8 bytes.
    fn bytes(&self, form: TextForm) -> Cow<'_, [u8]> {
        match (self, form) {
            (Payload::Bytes(bytes), _) => Cow::Borrowed(bytes),
            (Payload::Text(text), TextForm::Utf8) => Cow::Borrowed(text.as_bytes()),
            (Payload::Text(text), TextForm::Hex) => crypto::hex_decode(text)
                .map(Cow::Owned)
                .unwrap_or(Cow::Borrowed(text.as_bytes())),
        }
    }
}

/// Result of a single script run, including `console.*` output.
#[derive(Debug, Clone)]
pub struct ScriptOutput {
    pub result: Result<Payload, String>,
    pub console: Vec<String>,
}

/// Error of a script chain, naming the script that failed.
#[derive(Debug, Clone)]
pub struct ScriptChainError {
    pub script_name: String,
    pub message: String,
}

impl std::fmt::Display for ScriptChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&tr!(
            "script.chain_failed",
            name = self.script_name,
            error = self.message
        ))
    }
}

/// Run `(name, code)` scripts in order, feeding each output into the next.
pub fn run_chain(
    scripts: &[(String, String)],
    payload: Payload,
    form: TextForm,
    topic: &str,
    env: &EnvVars,
) -> Result<Payload, ScriptChainError> {
    let mut current = payload;
    for (name, code) in scripts {
        current = run_script(code, &current, form, topic, env)
            .result
            .map_err(|message| ScriptChainError {
                script_name: name.clone(),
                message,
            })?;
    }
    Ok(current)
}

/// Check the script for syntax errors without running it.
pub fn validate(code: &str) -> Result<(), String> {
    let mut ctx = Context::default();
    let wrapped = wrap(code);
    boa_engine::Script::parse(Source::from_bytes(wrapped.as_bytes()), None, &mut ctx)
        .map(|_| ())
        .map_err(|e| tr!("script.syntax_error", error = e).to_string())
}

fn wrap(code: &str) -> String {
    // Keep the user's code on its own lines so reported positions stay close.
    format!(
        "(async function () {{\n\"use strict\";\n{code}\n;\nif (typeof process === 'function') {{ return await process(payload, topic); }}\nreturn payload;\n}})()"
    )
}

pub fn run_script(code: &str, payload: &Payload, form: TextForm, topic: &str, env: &EnvVars) -> ScriptOutput {
    CONSOLE.with(|c| c.borrow_mut().clear());
    ENV.with(|e| *e.borrow_mut() = env.to_vec());
    let result = execute(code, payload, form, topic, env);
    ENV.with(|e| e.borrow_mut().clear());
    let console = CONSOLE.with(|c| std::mem::take(&mut *c.borrow_mut()));
    ScriptOutput { result, console }
}

fn execute(code: &str, payload: &Payload, form: TextForm, topic: &str, env: &EnvVars) -> Result<Payload, String> {
    let mut ctx = Context::default();
    ctx.runtime_limits_mut().set_loop_iteration_limit(LOOP_ITERATION_LIMIT);
    ctx.runtime_limits_mut().set_recursion_limit(RECURSION_LIMIT);
    let text = payload.text(form);
    install_globals(&mut ctx, &text, &payload.bytes(form), topic, env).map_err(|e| e.to_string())?;

    let value = ctx
        .eval(Source::from_bytes(wrap(code).as_bytes()))
        .map_err(|e| describe_error(e, &mut ctx))?;
    ctx.run_jobs().map_err(|e| describe_error(e, &mut ctx))?;

    let promise = value
        .as_object()
        .and_then(|obj| JsPromise::from_object(obj).ok())
        .ok_or_else(|| tr!("script.no_promise").to_string())?;
    match promise.state() {
        boa_engine::builtins::promise::PromiseState::Fulfilled(v) => value_to_payload(&v, payload, &text, &mut ctx),
        boa_engine::builtins::promise::PromiseState::Rejected(reason) => Err(reason
            .to_string(&mut ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|_| tr!("script.unknown_error").to_string())),
        boa_engine::builtins::promise::PromiseState::Pending => Err(tr!("script.pending_promise").to_string()),
    }
}

fn describe_error(err: JsError, ctx: &mut Context) -> String {
    if let Some(native) = err.as_native() {
        // RuntimeLimit errors cannot be turned into JS values (Boa panics).
        if matches!(native.kind, boa_engine::error::JsNativeErrorKind::RuntimeLimit) {
            return tr!("script.runtime_limit", error = native.message()).to_string();
        }
        return format!("{}: {}", native.kind, native.message());
    }
    let opaque = err.to_opaque(ctx);
    opaque
        .to_string(ctx)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|_| err.to_string())
}

/// Returning the `payload` string unchanged keeps the input as it was, so a
/// script that only reads a binary message does not mangle its bytes.
fn value_to_payload(value: &JsValue, input: &Payload, input_text: &str, ctx: &mut Context) -> Result<Payload, String> {
    if value.is_null_or_undefined() {
        return Ok(input.clone());
    }
    let text = if let Some(s) = value.as_string() {
        s.to_std_string_escaped()
    } else if let Some(obj) = value.as_object() {
        if let Ok(arr) = JsUint8Array::from_object(obj) {
            return uint8_bytes(&arr, ctx)
                .map(Payload::Bytes)
                .map_err(|e| describe_error(e, ctx));
        }
        match value.to_json(ctx) {
            Ok(Some(json)) => json.to_string(),
            Ok(None) => return Ok(input.clone()),
            Err(e) => return Err(tr!("script.not_serializable", error = describe_error(e, ctx)).to_string()),
        }
    } else {
        value
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .map_err(|e| describe_error(e, ctx))?
    };
    Ok(if text == input_text {
        input.clone()
    } else {
        Payload::Text(text)
    })
}

// ---------------------------------------------------------------------------
// Argument helpers

fn err(message: impl Into<String>) -> JsError {
    JsNativeError::error().with_message(message.into()).into()
}

fn to_js<T, E: std::fmt::Display>(r: Result<T, E>) -> JsResult<T> {
    r.map_err(|e| err(e.to_string()))
}

fn arg_string(args: &[JsValue], i: usize, ctx: &mut Context) -> JsResult<String> {
    let value = args.get(i).cloned().unwrap_or_default();
    Ok(value.to_string(ctx)?.to_std_string_escaped())
}

fn arg_opt_string(args: &[JsValue], i: usize, ctx: &mut Context) -> JsResult<Option<String>> {
    match args.get(i) {
        None => Ok(None),
        Some(v) if v.is_null_or_undefined() => Ok(None),
        Some(v) => {
            let s = v.to_string(ctx)?.to_std_string_escaped();
            Ok((!s.is_empty()).then_some(s))
        }
    }
}

/// Accepts a string (UTF-8 encoded) or any byte container
/// (`Uint8Array`, `Array` of numbers, array-like object).
fn arg_bytes(args: &[JsValue], i: usize, ctx: &mut Context) -> JsResult<Vec<u8>> {
    let value = args.get(i).cloned().unwrap_or_default();
    if let Some(s) = value.as_string() {
        return Ok(s.to_std_string_escaped().into_bytes());
    }
    if let Some(obj) = value.as_object() {
        if let Ok(arr) = JsUint8Array::from_object(obj.clone()) {
            return uint8_bytes(&arr, ctx);
        }
        let len = obj.get(js_string!("length"), ctx)?.to_length(ctx)?;
        let mut bytes = Vec::with_capacity(len as usize);
        for idx in 0..len {
            let n = obj.get(idx, ctx)?.to_number(ctx)?;
            bytes.push(n as i64 as u8);
        }
        return Ok(bytes);
    }
    if value.is_null_or_undefined() {
        return Ok(Vec::new());
    }
    Ok(value.to_string(ctx)?.to_std_string_escaped().into_bytes())
}

/// Copied from the backing buffer in one go: `JsUint8Array::iter` reads
/// element by element through property lookups, slow for large payloads.
fn uint8_bytes(arr: &JsUint8Array, ctx: &mut Context) -> JsResult<Vec<u8>> {
    let (offset, len) = (arr.byte_offset(ctx)?, arr.byte_length(ctx)?);
    let buffer = arr.buffer(ctx)?;
    let buffer = buffer
        .as_object()
        .and_then(|o| JsArrayBuffer::from_object(o.clone()).ok());
    let bytes = buffer
        .as_ref()
        .and_then(|b| b.data())
        .and_then(|data| data.get(offset..offset + len).map(<[u8]>::to_vec));
    // A SharedArrayBuffer is not a JsArrayBuffer: take the slow path.
    Ok(bytes.unwrap_or_else(|| arr.iter(ctx).collect()))
}

fn bytes_value(bytes: Vec<u8>, ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsUint8Array::from_iter(bytes, ctx)?.into())
}

fn str_value(s: impl AsRef<str>) -> JsValue {
    JsValue::from(JsString::from(s.as_ref()))
}

fn arg_usize(args: &[JsValue], i: usize, default: usize, ctx: &mut Context) -> JsResult<usize> {
    match args.get(i) {
        Some(v) if !v.is_null_or_undefined() => {
            let n = v.to_number(ctx)?;
            if !n.is_finite() || !(0.0..=1_048_576.0).contains(&n) {
                return Err(err(tr!("script.invalid_length")));
            }
            Ok(n as usize)
        }
        _ => Ok(default),
    }
}

// ---------------------------------------------------------------------------
// crypto.* functions

fn string_to_bytes(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let s = arg_string(args, 0, ctx)?;
    bytes_value(s.into_bytes(), ctx)
}

fn bytes_to_string(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let bytes = arg_bytes(args, 0, ctx)?;
    Ok(str_value(String::from_utf8_lossy(&bytes)))
}

fn bytes_to_base64(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    Ok(str_value(crypto::base64_encode(&arg_bytes(args, 0, ctx)?)))
}

fn base64_to_bytes(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let bytes = to_js(crypto::base64_decode(&arg_string(args, 0, ctx)?))?;
    bytes_value(bytes, ctx)
}

fn bytes_to_hex(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    Ok(str_value(crypto::hex_encode(&arg_bytes(args, 0, ctx)?)))
}

fn hex_to_bytes(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let bytes = to_js(crypto::hex_decode(&arg_string(args, 0, ctx)?))?;
    bytes_value(bytes, ctx)
}

fn random_bytes(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let len = arg_usize(args, 0, 16, ctx)?;
    bytes_value(crypto::random_bytes(len), ctx)
}

fn generate_key(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let bits = arg_usize(args, 0, 128, ctx)?;
    if !matches!(bits, 128 | 192 | 256) {
        return Err(err(tr!("script.invalid_key_bits")));
    }
    Ok(str_value(crypto::base64_encode(&crypto::random_bytes(bits / 8))))
}

fn generate_iv(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let len = arg_usize(args, 0, crypto::GCM_IV_LEN, ctx)?;
    Ok(str_value(crypto::base64_encode(&crypto::random_bytes(len))))
}

fn sha256(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    Ok(str_value(crypto::sha256(&arg_bytes(args, 0, ctx)?)))
}

fn sha1(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    Ok(str_value(crypto::sha1(&arg_bytes(args, 0, ctx)?)))
}

fn md5(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    Ok(str_value(crypto::md5(&arg_bytes(args, 0, ctx)?)))
}

fn hmac_sha256(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let key = arg_bytes(args, 0, ctx)?;
    let data = arg_bytes(args, 1, ctx)?;
    Ok(str_value(crypto::hmac_sha256(&key, &data)))
}

fn crc32(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    Ok(str_value(crypto::crc32(&arg_bytes(args, 0, ctx)?)))
}

/// XOR over UTF-16 code units, symmetric (same as the previous version).
fn xor(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let data: Vec<u16> = arg_string(args, 0, ctx)?.encode_utf16().collect();
    let key: Vec<u16> = arg_string(args, 1, ctx)?.encode_utf16().collect();
    if key.is_empty() {
        return Err(err(tr!("script.xor_empty_key")));
    }
    let out: Vec<u16> = data.iter().enumerate().map(|(i, c)| c ^ key[i % key.len()]).collect();
    Ok(JsValue::from(JsString::from(out.as_slice())))
}

type EncryptFn = fn(&[u8], &[u8], Option<&[u8]>) -> Result<Vec<u8>, String>;
type DecryptFn = fn(&[u8], &[u8]) -> Result<Vec<u8>, String>;

#[derive(Clone, Copy)]
enum Codec {
    Base64,
    Hex,
}

impl Codec {
    fn decode(self, s: &str) -> Result<Vec<u8>, String> {
        match self {
            Codec::Base64 => crypto::base64_decode(s),
            Codec::Hex => crypto::hex_decode(s),
        }
    }

    fn encode(self, bytes: &[u8]) -> String {
        match self {
            Codec::Base64 => crypto::base64_encode(bytes),
            Codec::Hex => crypto::hex_encode(bytes),
        }
    }
}

fn aes_encrypt(args: &[JsValue], ctx: &mut Context, codec: Codec, f: EncryptFn) -> JsResult<JsValue> {
    let plaintext = arg_bytes(args, 0, ctx)?;
    let key = to_js(codec.decode(&arg_string(args, 1, ctx)?))?;
    let iv = match arg_opt_string(args, 2, ctx)? {
        Some(iv) => Some(to_js(codec.decode(&iv))?),
        None => None,
    };
    let out = to_js(f(&plaintext, &key, iv.as_deref()))?;
    Ok(str_value(codec.encode(&out)))
}

fn aes_decrypt(args: &[JsValue], ctx: &mut Context, codec: Codec, f: DecryptFn) -> JsResult<JsValue> {
    let data = to_js(codec.decode(&arg_string(args, 0, ctx)?))?;
    let key = to_js(codec.decode(&arg_string(args, 1, ctx)?))?;
    let out = to_js(f(&data, &key))?;
    Ok(str_value(String::from_utf8_lossy(&out)))
}

fn aes_gcm_encrypt(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    aes_encrypt(a, c, Codec::Base64, crypto::aes_gcm_encrypt)
}
fn aes_gcm_decrypt(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    aes_decrypt(a, c, Codec::Base64, crypto::aes_gcm_decrypt)
}
fn aes_cbc_encrypt(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    aes_encrypt(a, c, Codec::Base64, crypto::aes_cbc_encrypt)
}
fn aes_cbc_decrypt(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    aes_decrypt(a, c, Codec::Base64, crypto::aes_cbc_decrypt)
}
fn aes_gcm_encrypt_hex(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    aes_encrypt(a, c, Codec::Hex, crypto::aes_gcm_encrypt)
}
fn aes_gcm_decrypt_hex(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    aes_decrypt(a, c, Codec::Hex, crypto::aes_gcm_decrypt)
}
fn aes_cbc_encrypt_hex(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    aes_encrypt(a, c, Codec::Hex, crypto::aes_cbc_encrypt)
}
fn aes_cbc_decrypt_hex(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    aes_decrypt(a, c, Codec::Hex, crypto::aes_cbc_decrypt)
}

// ---------------------------------------------------------------------------
// zlib.* functions

/// Optional compression level, 0 to 9 (default 6, as in Node.js).
fn arg_level(args: &[JsValue], i: usize, ctx: &mut Context) -> JsResult<u32> {
    match args.get(i) {
        Some(v) if !v.is_null_or_undefined() => {
            let n = v.to_number(ctx)?;
            if n.fract() != 0.0 || !(0.0..=9.0).contains(&n) {
                return Err(err(tr!("zlib.invalid_level")));
            }
            Ok(n as u32)
        }
        _ => Ok(6),
    }
}

fn compress(args: &[JsValue], ctx: &mut Context, format: zlib::Format) -> JsResult<JsValue> {
    let data = arg_bytes(args, 0, ctx)?;
    let level = arg_level(args, 1, ctx)?;
    bytes_value(to_js(zlib::compress(&data, format, level))?, ctx)
}

fn decompress(args: &[JsValue], ctx: &mut Context, format: Option<zlib::Format>) -> JsResult<JsValue> {
    let data = arg_bytes(args, 0, ctx)?;
    let format = format.unwrap_or_else(|| zlib::Format::detect(&data));
    bytes_value(to_js(zlib::decompress(&data, format))?, ctx)
}

fn gzip(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    compress(a, c, zlib::Format::Gzip)
}
fn gunzip(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    decompress(a, c, Some(zlib::Format::Gzip))
}
fn deflate(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    compress(a, c, zlib::Format::Zlib)
}
fn inflate(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    decompress(a, c, Some(zlib::Format::Zlib))
}
fn deflate_raw(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    compress(a, c, zlib::Format::Raw)
}
fn inflate_raw(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    decompress(a, c, Some(zlib::Format::Raw))
}
fn unzip(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    decompress(a, c, None)
}

// ---------------------------------------------------------------------------
// atob / btoa / console

fn btoa(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let s = arg_string(args, 0, ctx)?;
    let mut bytes = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let code = ch as u32;
        if code > 0xFF {
            return Err(err(tr!("script.btoa_latin1")));
        }
        bytes.push(code as u8);
    }
    Ok(str_value(crypto::base64_encode(&bytes)))
}

fn atob(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let bytes = to_js(crypto::base64_decode(&arg_string(args, 0, ctx)?))?;
    Ok(str_value(bytes.iter().map(|&b| b as char).collect::<String>()))
}

fn console_line(level: &str, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let mut parts = Vec::with_capacity(args.len());
    for arg in args {
        let text = if arg.is_object() && !arg.is_callable() {
            match arg.to_json(ctx) {
                Ok(Some(json)) => json.to_string(),
                _ => arg.to_string(ctx)?.to_std_string_escaped(),
            }
        } else {
            arg.to_string(ctx)?.to_std_string_escaped()
        };
        parts.push(text);
    }
    let line = parts.join(" ");
    log::info!("[Script {level}] {line}");
    CONSOLE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() < 200 {
            c.push(if level == "log" {
                line
            } else {
                format!("[{level}] {line}")
            });
        }
    });
    Ok(JsValue::undefined())
}

fn console_log(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    console_line("log", a, c)
}
fn console_warn(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    console_line("warn", a, c)
}
fn console_error(_: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    console_line("error", a, c)
}

// ---------------------------------------------------------------------------
// env.* functions (variables are read from the thread-local `ENV`)

fn env_get(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let name = arg_string(args, 0, ctx)?;
    Ok(ENV.with(|e| {
        e.borrow()
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| str_value(v))
            .unwrap_or_default()
    }))
}

fn env_replace(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let text = arg_string(args, 0, ctx)?;
    Ok(str_value(ENV.with(|e| env::replace(&text, &e.borrow()))))
}

fn env_all(_: &JsValue, _: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let vars = ENV.with(|e| e.borrow().clone());
    let array = boa_engine::object::builtins::JsArray::new(ctx);
    for (name, value) in vars {
        let entry = ObjectInitializer::new(ctx)
            .property(js_string!("name"), str_value(&name), Attribute::all())
            .property(js_string!("value"), str_value(&value), Attribute::all())
            .build();
        array.push(entry, ctx)?;
    }
    Ok(array.into())
}

type NativeFn = fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>;

/// Names and arities of the `crypto` object, also used by the help dialog.
pub const CRYPTO_FUNCTIONS: &[(&str, usize, NativeFn)] = &[
    ("stringToBytes", 1, string_to_bytes),
    ("bytesToString", 1, bytes_to_string),
    ("bytesToBase64", 1, bytes_to_base64),
    ("base64ToBytes", 1, base64_to_bytes),
    ("bytesToHex", 1, bytes_to_hex),
    ("hexToBytes", 1, hex_to_bytes),
    ("randomBytes", 1, random_bytes),
    ("generateKey", 1, generate_key),
    ("generateIv", 1, generate_iv),
    ("sha256", 1, sha256),
    ("sha1", 1, sha1),
    ("md5", 1, md5),
    ("hmacSha256", 2, hmac_sha256),
    ("aesGcmEncrypt", 3, aes_gcm_encrypt),
    ("aesGcmDecrypt", 2, aes_gcm_decrypt),
    ("aesCbcEncrypt", 3, aes_cbc_encrypt),
    ("aesCbcDecrypt", 2, aes_cbc_decrypt),
    ("aesGcmEncryptHex", 3, aes_gcm_encrypt_hex),
    ("aesGcmDecryptHex", 2, aes_gcm_decrypt_hex),
    ("aesCbcEncryptHex", 3, aes_cbc_encrypt_hex),
    ("aesCbcDecryptHex", 2, aes_cbc_decrypt_hex),
    ("xor", 2, xor),
    ("crc32", 1, crc32),
];

/// Names and arities of the `zlib` object (the names of Node.js' `zlib`).
const ZLIB_FUNCTIONS: &[(&str, usize, NativeFn)] = &[
    ("gzip", 2, gzip),
    ("gunzip", 1, gunzip),
    ("deflate", 2, deflate),
    ("inflate", 1, inflate),
    ("deflateRaw", 2, deflate_raw),
    ("inflateRaw", 1, inflate_raw),
    ("unzip", 1, unzip),
];

fn install_globals(ctx: &mut Context, payload: &str, bytes: &[u8], topic: &str, env: &EnvVars) -> JsResult<()> {
    let mut crypto_obj = ObjectInitializer::new(ctx);
    for (name, len, f) in CRYPTO_FUNCTIONS {
        crypto_obj.function(NativeFunction::from_fn_ptr(*f), JsString::from(*name), *len);
    }
    let crypto_obj = crypto_obj.build();

    let mut zlib_obj = ObjectInitializer::new(ctx);
    for (name, len, f) in ZLIB_FUNCTIONS {
        zlib_obj.function(NativeFunction::from_fn_ptr(*f), JsString::from(*name), *len);
    }
    let zlib_obj = zlib_obj.build();

    let console = ObjectInitializer::new(ctx)
        .function(NativeFunction::from_fn_ptr(console_log), js_string!("log"), 0)
        .function(NativeFunction::from_fn_ptr(console_log), js_string!("info"), 0)
        .function(NativeFunction::from_fn_ptr(console_log), js_string!("debug"), 0)
        .function(NativeFunction::from_fn_ptr(console_warn), js_string!("warn"), 0)
        .function(NativeFunction::from_fn_ptr(console_error), js_string!("error"), 0)
        .build();

    // `env.NAME` for every variable; the methods win over equally named
    // variables (as before).
    let mut env_obj = ObjectInitializer::new(ctx);
    for (name, value) in env {
        env_obj.property(JsString::from(name.as_str()), str_value(value), Attribute::all());
    }
    let env_obj = env_obj
        .function(NativeFunction::from_fn_ptr(env_get), js_string!("get"), 1)
        .function(NativeFunction::from_fn_ptr(env_replace), js_string!("replace"), 1)
        .function(NativeFunction::from_fn_ptr(env_all), js_string!("all"), 0)
        .build();

    let attr = Attribute::WRITABLE | Attribute::CONFIGURABLE;
    let payload_bytes = bytes_value(bytes.to_vec(), ctx)?;
    ctx.register_global_property(js_string!("crypto"), crypto_obj, attr)?;
    ctx.register_global_property(js_string!("zlib"), zlib_obj, attr)?;
    ctx.register_global_property(js_string!("env"), env_obj, attr)?;
    ctx.register_global_property(js_string!("console"), console, attr)?;
    ctx.register_global_property(js_string!("payload"), str_value(payload), attr)?;
    ctx.register_global_property(js_string!("payloadBytes"), payload_bytes, attr)?;
    ctx.register_global_property(js_string!("topic"), str_value(topic), attr)?;
    ctx.register_global_builtin_callable(js_string!("atob"), 1, NativeFunction::from_fn_ptr(atob))?;
    ctx.register_global_builtin_callable(js_string!("btoa"), 1, NativeFunction::from_fn_ptr(btoa))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(code: &str, payload: &str) -> Result<String, String> {
        run_script(code, &Payload::Text(payload.into()), TextForm::Utf8, "t/1", &[])
            .result
            .map(text)
    }

    fn text(payload: Payload) -> String {
        match payload {
            Payload::Text(text) => text,
            Payload::Bytes(bytes) => panic!("expected text, got bytes {bytes:?}"),
        }
    }

    #[test]
    fn process_function_contract() {
        assert_eq!(
            run("function process(p){ return p.toUpperCase(); }", "abc").unwrap(),
            "ABC"
        );
        assert_eq!(
            run("async function process(p, t){ return t + ':' + p; }", "x").unwrap(),
            "t/1:x"
        );
        assert_eq!(
            run("function process(p){ return {a: JSON.parse(p).v + 1}; }", r#"{"v":1}"#).unwrap(),
            r#"{"a":2}"#
        );
        assert_eq!(run("function process(p){ }", "keep").unwrap(), "keep");
        assert_eq!(run("const x = 1;", "pass").unwrap(), "pass");
        assert_eq!(run("function process(p){ return 42; }", "").unwrap(), "42");
    }

    #[test]
    fn errors_are_reported() {
        let e = run("function process(p){ throw new Error('boom'); }", "").unwrap_err();
        assert!(e.contains("boom"), "{e}");
        assert!(run("function process(p){ return undefinedVar; }", "").is_err());
        assert!(validate("function process(p) { return p; }").is_ok());
        assert!(validate("function process(p) { return p;").is_err());
        let e = run("while (true) {}", "").unwrap_err();
        assert!(!e.is_empty());
    }

    #[test]
    fn crypto_api_works_from_js() {
        assert_eq!(
            run("async function process(p){ return await crypto.sha256(p); }", "abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            run("function process(p){ return crypto.md5(p); }", "abc").unwrap(),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        let code = r#"
            async function process(p) {
                const key = "8568230E4BBAF3A979B6F510A38717D5";
                const iv = "0CA0FE0BA0BE0FA0CE0DB0AD";
                const enc = await crypto.aesGcmEncryptHex(p, key, iv);
                const dec = await crypto.aesGcmDecryptHex(enc, key);
                const k64 = crypto.generateKey(256);
                const cbc = await crypto.aesCbcDecrypt(await crypto.aesCbcEncrypt(dec, k64), k64);
                const bytes = crypto.stringToBytes(cbc);
                return crypto.bytesToString(crypto.hexToBytes(crypto.bytesToHex(crypto.base64ToBytes(crypto.bytesToBase64(bytes)))));
            }"#;
        assert_eq!(run(code, "你好 mqtt").unwrap(), "你好 mqtt");
        assert_eq!(
            run(
                "function process(p){ return crypto.xor(crypto.xor(p, 'k'), 'k'); }",
                "secret"
            )
            .unwrap(),
            "secret"
        );
        assert_eq!(run("function process(p){ return atob(btoa(p)); }", "hi").unwrap(), "hi");
        assert_eq!(
            run("function process(p){ return crypto.randomBytes(8).length; }", "").unwrap(),
            "8"
        );
    }

    #[test]
    fn console_is_captured_and_chain_runs_in_order() {
        let out = run_script(
            "console.log('a', {b: 1}); function process(p){ return p; }",
            &Payload::Text("x".into()),
            TextForm::Utf8,
            "",
            &[],
        );
        assert_eq!(out.console, vec![r#"a {"b":1}"#.to_string()]);

        let x = || Payload::Text("x".into());
        let chain = vec![
            ("one".to_string(), "function process(p){ return p + '1'; }".to_string()),
            ("two".to_string(), "function process(p){ return p + '2'; }".to_string()),
        ];
        assert_eq!(
            run_chain(&chain, x(), TextForm::Utf8, "", &[]).unwrap(),
            Payload::Text("x12".into())
        );
        let bad = vec![("bad".to_string(), "function process(){ throw 'nope'; }".to_string())];
        let e = run_chain(&bad, x(), TextForm::Utf8, "", &[]).unwrap_err();
        assert_eq!(e.script_name, "bad");
    }

    const GUNZIP: &str = "function process(){ return crypto.bytesToString(zlib.gunzip(payloadBytes)); }";

    #[test]
    fn binary_payloads_reach_scripts_as_bytes() {
        let json = r#"{"temp":21.5,"msg":"你好"}"#;
        let gz = zlib::compress(json.as_bytes(), zlib::Format::Gzip, 6).unwrap();
        let received = Payload::Bytes(gz.clone());
        let run_bytes = |code: &str, payload: &Payload| run_script(code, payload, TextForm::Utf8, "t/1", &[]).result;

        // The case of issue #1: a gzip-compressed message, decoded on receipt.
        assert_eq!(run_bytes(GUNZIP, &received), Ok(Payload::Text(json.into())));
        // Reading the (lossy) payload string without changing it keeps the bytes.
        assert_eq!(
            run_bytes("function process(p){ return p; }", &received),
            Ok(received.clone())
        );
        assert_eq!(
            run_bytes("console.log(payload.length);", &received),
            Ok(received.clone())
        );
        let chain = vec![
            ("log".to_string(), "function process(p){ return p; }".to_string()),
            ("gunzip".to_string(), GUNZIP.to_string()),
        ];
        assert_eq!(
            run_chain(&chain, received.clone(), TextForm::Utf8, "", &[]).unwrap(),
            Payload::Text(json.into())
        );

        // A returned Uint8Array is the payload's new bytes, views included.
        let bytes = Payload::Bytes(vec![1, 2, 3, 4]);
        assert_eq!(
            run_bytes("function process(){ return payloadBytes.subarray(1, 3); }", &bytes),
            Ok(Payload::Bytes(vec![2, 3]))
        );
        assert_eq!(
            run_bytes(
                "function process(){ return crypto.bytesToHex(payloadBytes.subarray(1)); }",
                &bytes
            ),
            Ok(Payload::Text("020304".into()))
        );
        let packed = run_bytes(
            "function process(p){ return zlib.gzip(p); }",
            &Payload::Text(json.into()),
        );
        let Ok(Payload::Bytes(packed)) = packed else {
            panic!("{packed:?}")
        };
        assert_eq!(zlib::decompress(&packed, zlib::Format::Gzip).unwrap(), json.as_bytes());

        // Text payloads have their UTF-8 bytes.
        assert_eq!(
            run("function process(){ return payloadBytes.length; }", "你好"),
            Ok("6".into())
        );
    }

    #[test]
    fn hex_payloads_are_bytes_in_hex_form() {
        let gz = zlib::compress(b"hi", zlib::Format::Gzip, 6).unwrap();
        let typed = Payload::Text(gz.iter().map(|b| format!("{b:02X} ")).collect());
        let out = run_script(GUNZIP, &typed, TextForm::Hex, "", &[]).result;
        assert_eq!(out, Ok(Payload::Text("hi".into())));
        // Bytes left by an earlier script read as HEX in the next one.
        let out = run_script(
            "function process(p){ return p.toUpperCase(); }",
            &Payload::Bytes(vec![0xab, 0x01]),
            TextForm::Hex,
            "",
            &[],
        );
        assert_eq!(out.result, Ok(Payload::Text("AB01".into())));
    }

    #[test]
    fn zlib_api_works_from_js() {
        let code = r#"
            function process(p) {
                const pairs = [[zlib.gzip, zlib.gunzip], [zlib.deflate, zlib.inflate], [zlib.deflateRaw, zlib.inflateRaw]];
                const out = [];
                for (const [pack, unpack] of pairs) {
                    out.push(crypto.bytesToString(unpack(pack(p))));
                    out.push(crypto.bytesToString(unpack(pack(crypto.stringToBytes(p), 9))));
                }
                out.push(crypto.bytesToString(zlib.unzip(zlib.gzip(p))));
                out.push(crypto.bytesToString(zlib.unzip(zlib.deflate(p, 0))));
                return out.join('|');
            }"#;
        assert_eq!(run(code, "压缩 mqtt").unwrap(), ["压缩 mqtt"; 8].join("|"));
        assert!(run("function process(){ return zlib.gunzip(payloadBytes); }", "plain").is_err());
        assert!(run("function process(){ return zlib.inflate(zlib.gzip('x')); }", "").is_err());
        for level in ["10", "-1", "1.5", "'x'"] {
            assert!(
                run(&format!("function process(p){{ return zlib.gzip(p, {level}); }}"), "x").is_err(),
                "level {level}"
            );
        }
    }

    #[test]
    fn env_is_exposed_to_scripts() {
        let env = vec![
            ("DEVICE_ID".to_string(), "dev-1".to_string()),
            ("KEY".to_string(), "k".to_string()),
            ("get".to_string(), "shadowed".to_string()),
        ];
        let run_env = |code: &str, payload: &str| {
            text(
                run_script(code, &Payload::Text(payload.into()), TextForm::Utf8, "t/1", &env)
                    .result
                    .unwrap(),
            )
        };
        assert_eq!(
            run_env("function process(p){ return env.DEVICE_ID + ':' + p; }", "x"),
            "dev-1:x"
        );
        assert_eq!(
            run_env("function process(){ return env.get('KEY') + env.get('NOPE'); }", ""),
            "kundefined"
        );
        assert_eq!(
            run_env("function process(p){ return env.replace(p); }", "{{DEVICE_ID}}/{{X}}"),
            "dev-1/{{X}}"
        );
        assert_eq!(
            run_env(
                "function process(){ return env.all().map(v => v.name + '=' + v.value).join(','); }",
                ""
            ),
            "DEVICE_ID=dev-1,KEY=k,get=shadowed"
        );
        assert_eq!(run_env("function process(){ return typeof env.get; }", ""), "function");
        // Without variables `env` still exists.
        assert_eq!(
            run(
                "function process(){ return env.all().length + (env.X === undefined ? 'u' : ''); }",
                ""
            ),
            Ok("0u".into())
        );
        // Variables do not leak into the next run on the same thread.
        assert_eq!(
            run("function process(){ return String(env.get('KEY')); }", ""),
            Ok("undefined".into())
        );
    }
}
