//! JavaScript pre-processing scripts, executed with the pure-Rust Boa engine.
//!
//! Contract (unchanged from the previous version): a script may define
//! `process(payload, topic)` (sync or async); its return value replaces the
//! payload. Objects are JSON-serialized, `undefined`/`null` keep the input.
//! Without a `process` function the script body itself runs and the payload
//! is passed through.

pub mod crypto;

use boa_engine::object::ObjectInitializer;
use boa_engine::object::builtins::{JsPromise, JsUint8Array};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsString, JsValue, NativeFunction, Source, js_string};
use std::cell::RefCell;

/// Guards against runaway scripts (`while (true) {}`) freezing a worker.
const LOOP_ITERATION_LIMIT: u64 = 5_000_000;
const RECURSION_LIMIT: usize = 400;

thread_local! {
    static CONSOLE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Result of a single script run, including `console.*` output.
#[derive(Debug, Clone)]
pub struct ScriptOutput {
    pub result: Result<String, String>,
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
        write!(f, "脚本执行失败 [{}]: {}", self.script_name, self.message)
    }
}

/// Run `(name, code)` scripts in order, feeding each output into the next.
pub fn run_chain(scripts: &[(String, String)], payload: &str, topic: &str) -> Result<String, ScriptChainError> {
    let mut current = payload.to_string();
    for (name, code) in scripts {
        current = run_script(code, &current, topic)
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
        .map_err(|e| format!("语法错误: {e}"))
}

fn wrap(code: &str) -> String {
    // Keep the user's code on its own lines so reported positions stay close.
    format!(
        "(async function () {{\n\"use strict\";\n{code}\n;\nif (typeof process === 'function') {{ return await process(payload, topic); }}\nreturn payload;\n}})()"
    )
}

pub fn run_script(code: &str, payload: &str, topic: &str) -> ScriptOutput {
    CONSOLE.with(|c| c.borrow_mut().clear());
    let result = execute(code, payload, topic);
    let console = CONSOLE.with(|c| std::mem::take(&mut *c.borrow_mut()));
    ScriptOutput { result, console }
}

fn execute(code: &str, payload: &str, topic: &str) -> Result<String, String> {
    let mut ctx = Context::default();
    ctx.runtime_limits_mut().set_loop_iteration_limit(LOOP_ITERATION_LIMIT);
    ctx.runtime_limits_mut().set_recursion_limit(RECURSION_LIMIT);
    install_globals(&mut ctx, payload, topic).map_err(|e| e.to_string())?;

    let value = ctx
        .eval(Source::from_bytes(wrap(code).as_bytes()))
        .map_err(|e| describe_error(e, &mut ctx))?;
    ctx.run_jobs().map_err(|e| describe_error(e, &mut ctx))?;

    let promise = value
        .as_object()
        .and_then(|obj| JsPromise::from_object(obj).ok())
        .ok_or_else(|| "脚本执行异常：未返回 Promise".to_string())?;
    match promise.state() {
        boa_engine::builtins::promise::PromiseState::Fulfilled(v) => value_to_payload(&v, payload, &mut ctx),
        boa_engine::builtins::promise::PromiseState::Rejected(reason) => Err(reason
            .to_string(&mut ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|_| "未知错误".to_string())),
        boa_engine::builtins::promise::PromiseState::Pending => {
            Err("脚本未完成：存在无法完成的 Promise（不支持定时器）".to_string())
        }
    }
}

fn describe_error(err: JsError, ctx: &mut Context) -> String {
    if let Some(native) = err.as_native() {
        // RuntimeLimit errors cannot be turned into JS values (Boa panics).
        if matches!(native.kind, boa_engine::error::JsNativeErrorKind::RuntimeLimit) {
            return format!("脚本超出运行限制（可能存在死循环或递归过深）: {}", native.message());
        }
        return format!("{}: {}", native.kind, native.message());
    }
    let opaque = err.to_opaque(ctx);
    opaque
        .to_string(ctx)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|_| err.to_string())
}

fn value_to_payload(value: &JsValue, original: &str, ctx: &mut Context) -> Result<String, String> {
    if value.is_null_or_undefined() {
        return Ok(original.to_string());
    }
    if let Some(s) = value.as_string() {
        return Ok(s.to_std_string_escaped());
    }
    if value.is_object() {
        if let Some(obj) = value.as_object()
            && let Ok(arr) = JsUint8Array::from_object(obj)
        {
            let bytes: Vec<u8> = arr.iter(ctx).collect();
            return Ok(String::from_utf8_lossy(&bytes).into_owned());
        }
        return match value.to_json(ctx) {
            Ok(Some(json)) => Ok(json.to_string()),
            Ok(None) => Ok(original.to_string()),
            Err(e) => Err(format!("返回值无法序列化为 JSON: {}", describe_error(e, ctx))),
        };
    }
    value
        .to_string(ctx)
        .map(|s| s.to_std_string_escaped())
        .map_err(|e| describe_error(e, ctx))
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
            return Ok(arr.iter(ctx).collect());
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
                return Err(err("长度参数无效"));
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
        return Err(err("密钥位数必须为 128/192/256"));
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
        return Err(err("xor 密钥不能为空"));
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
// atob / btoa / console

fn btoa(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let s = arg_string(args, 0, ctx)?;
    let mut bytes = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let code = ch as u32;
        if code > 0xFF {
            return Err(err("btoa: 字符串包含 Latin1 范围之外的字符"));
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

fn install_globals(ctx: &mut Context, payload: &str, topic: &str) -> JsResult<()> {
    let mut crypto_obj = ObjectInitializer::new(ctx);
    for (name, len, f) in CRYPTO_FUNCTIONS {
        crypto_obj.function(NativeFunction::from_fn_ptr(*f), JsString::from(*name), *len);
    }
    let crypto_obj = crypto_obj.build();

    let console = ObjectInitializer::new(ctx)
        .function(NativeFunction::from_fn_ptr(console_log), js_string!("log"), 0)
        .function(NativeFunction::from_fn_ptr(console_log), js_string!("info"), 0)
        .function(NativeFunction::from_fn_ptr(console_log), js_string!("debug"), 0)
        .function(NativeFunction::from_fn_ptr(console_warn), js_string!("warn"), 0)
        .function(NativeFunction::from_fn_ptr(console_error), js_string!("error"), 0)
        .build();

    let attr = Attribute::WRITABLE | Attribute::CONFIGURABLE;
    ctx.register_global_property(js_string!("crypto"), crypto_obj, attr)?;
    ctx.register_global_property(js_string!("console"), console, attr)?;
    ctx.register_global_property(js_string!("payload"), str_value(payload), attr)?;
    ctx.register_global_property(js_string!("topic"), str_value(topic), attr)?;
    ctx.register_global_builtin_callable(js_string!("atob"), 1, NativeFunction::from_fn_ptr(atob))?;
    ctx.register_global_builtin_callable(js_string!("btoa"), 1, NativeFunction::from_fn_ptr(btoa))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(code: &str, payload: &str) -> Result<String, String> {
        run_script(code, payload, "t/1").result
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
        let out = run_script("console.log('a', {b: 1}); function process(p){ return p; }", "x", "");
        assert_eq!(out.console, vec![r#"a {"b":1}"#.to_string()]);

        let chain = vec![
            ("one".to_string(), "function process(p){ return p + '1'; }".to_string()),
            ("two".to_string(), "function process(p){ return p + '2'; }".to_string()),
        ];
        assert_eq!(run_chain(&chain, "x", "").unwrap(), "x12");
        let bad = vec![("bad".to_string(), "function process(){ throw 'nope'; }".to_string())];
        let e = run_chain(&bad, "x", "").unwrap_err();
        assert_eq!(e.script_name, "bad");
    }
}
