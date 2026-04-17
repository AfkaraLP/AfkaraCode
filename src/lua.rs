use std::{fs, path::{Path, PathBuf}, time::Duration};

use mlua::{Lua, Table, Value as LuaValue, Function, Result as LuaResult, Error as LuaError, LuaSerdeExt};
use openai_client::{ToolCallArgDescriptor, ToolCallFn};
use serde_json::Value as JsonValue;

#[derive(Clone, Debug)]
struct ArgSpec {
    name: String,
    desc: String,
    ty: String,
    required: bool,
}

pub struct LuaTool {
    name: String,
    description: String,
    args: Vec<ArgSpec>,
    script_path: PathBuf,
    func_name: String,
}

impl LuaTool {
    fn build_arg_descriptors(&self) -> Vec<ToolCallArgDescriptor> {
        self.args
            .iter()
            .map(|a| {
                let name: &'static str = Box::leak(a.name.clone().into_boxed_str());
                let desc: &'static str = Box::leak(a.desc.clone().into_boxed_str());
                let mut d = match a.ty.as_str() {
                    "number" => ToolCallArgDescriptor::number(name, desc),
                    _ => ToolCallArgDescriptor::string(name, desc),
                };
                if a.required { d = d.set_required(); } else { d = d.set_optional(); }
                d
            })
            .collect()
    }
}

impl ToolCallFn for LuaTool {
    fn get_timeout_wait(&self) -> std::time::Duration { Duration::ZERO }

    fn get_args(&self) -> Vec<ToolCallArgDescriptor> { self.build_arg_descriptors() }

    fn get_description(&self) -> &'static str { "lua-defined dynamic tool" }

    fn get_name(&self) -> &'static str { Box::leak(self.name.clone().into_boxed_str()) }

    fn invoke<'invocation>(
        &'invocation self,
        args: &'invocation serde_json::Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'invocation>> {
        async fn run_tool(tool: &LuaTool, args: &JsonValue) -> String {
            match run_lua(tool, args).await { Ok(s) => s, Err(e) => format!("lua tool error: {e}") }
        }
        Box::pin(run_tool(self, args))
    }
}

async fn run_lua(tool: &LuaTool, args: &JsonValue) -> Result<String, LuaError> {
    let lua = Lua::new();
    install_http(&lua)?;

    let code = fs::read_to_string(&tool.script_path)
        .map_err(|e| LuaError::external(format!("failed to read lua script: {e}")))?;

    let value = lua
        .load(&code)
        .set_name(tool.script_path.to_string_lossy().as_ref())
        .eval::<LuaValue>()?;
    let tbl: Table = match value { LuaValue::Table(t) => t, _ => return Err(LuaError::external("lua script must return a table")) };

    let func: Function = tbl.get::<_, Function>(tool.func_name.as_str())?;

    let lua_args: LuaValue = lua.to_value(args)?;
    let res: LuaValue = func.call(lua_args)?;

    match res {
        LuaValue::String(s) => Ok(s.to_str()?.to_string()),
        other => {
            let json: JsonValue = lua.from_value(other)?;
            Ok(serde_json::to_string(&json).unwrap_or_else(|_| "null".to_string()))
        }
    }
}

fn parse_args_table(tbl: &Table) -> LuaResult<Vec<ArgSpec>> {
    let mut specs = Vec::new();
    if let Ok(args_tbl) = tbl.get::<_, Table>("args") {
        for pair in args_tbl.sequence_values::<Table>() {
            let t = pair?;
            let name: String = t.get("name").unwrap_or_else(|_| "arg".to_string());
            let desc: String = t.get("description").unwrap_or_else(|_| "".to_string());
            let ty: String = t.get("type").unwrap_or_else(|_| "string".to_string());
            let required: bool = t.get("required").unwrap_or(true);
            specs.push(ArgSpec { name, desc, ty, required });
        }
    }
    Ok(specs)
}

fn install_http(lua: &Lua) -> LuaResult<()> {
    use std::time::Duration as StdDuration;
    use reqwest::blocking::Client;

    #[derive(Clone, Default)]
    enum BodyKind { #[default] None, Text(String), Json(serde_json::Value) }

    let http_tbl = lua.create_table()?;

    let get = lua.create_function(move |_lua, url: String| {
        let res = tokio::task::block_in_place(|| -> Result<String, String> {
            let client = Client::builder().timeout(StdDuration::from_secs(30)).build().map_err(|e| e.to_string())?;
            let resp = client.get(&url).send().map_err(|e| e.to_string())?;
            let resp = resp.error_for_status().map_err(|e| e.to_string())?;
            resp.text().map_err(|e| e.to_string())
        });
        match res { Ok(s) => Ok(s), Err(e) => Err(LuaError::external(e)) }
    })?;

    let post = lua.create_function(move |lua, (url, body): (String, LuaValue)| {
        let kind = match body {
            LuaValue::String(s) => BodyKind::Text(s.to_str()?.to_string()),
            LuaValue::Nil => BodyKind::None,
            other => match lua.from_value::<serde_json::Value>(other) {
                Ok(v) => BodyKind::Json(v),
                Err(_) => BodyKind::None,
            }
        };
        let res = tokio::task::block_in_place(move || -> Result<String, String> {
            let client = Client::builder().timeout(StdDuration::from_secs(30)).build().map_err(|e| e.to_string())?;
            let mut req = client.post(&url);
            match kind {
                BodyKind::Json(v) => { req = req.json(&v); }
                BodyKind::Text(s) => { req = req.body(s); }
                BodyKind::None => {}
            }
            let resp = req.send().map_err(|e| e.to_string())?;
            let resp = resp.error_for_status().map_err(|e| e.to_string())?;
            resp.text().map_err(|e| e.to_string())
        });
        match res { Ok(s) => Ok(s), Err(e) => Err(LuaError::external(e)) }
    })?;

    let request = lua.create_function(move |lua, (method, url, opt): (String, String, Option<Table>)| {
        #[derive(Default, Clone)]
        struct ReqOptions { headers: Vec<(String, String)>, body: BodyKind, json: Option<serde_json::Value> }
        let mut options: ReqOptions = ReqOptions::default();
        if let Some(opt_tbl) = opt {
            if let Ok(headers_tbl) = opt_tbl.get::<_, Table>("headers") {
                for pair in headers_tbl.pairs::<LuaValue, LuaValue>() {
                    let (k, v) = pair.map_err(|e| LuaError::external(e.to_string()))?;
                    let key = match k { LuaValue::String(s) => s.to_str().map_err(LuaError::external)?.to_string(), _ => continue };
                    let val_str = match v { LuaValue::String(s) => s.to_str().map_err(LuaError::external)?.to_string(), LuaValue::Number(n) => n.to_string(), LuaValue::Boolean(b) => b.to_string(), _ => continue };
                    options.headers.push((key, val_str));
                }
            }
            if let Ok(body) = opt_tbl.get::<_, LuaValue>("body") {
                options.body = match body {
                    LuaValue::String(s) => BodyKind::Text(s.to_str()?.to_string()),
                    LuaValue::Nil => BodyKind::None,
                    other => match lua.from_value::<serde_json::Value>(other) { Ok(v) => BodyKind::Json(v), Err(_) => BodyKind::None }
                };
            }
            if let Ok(json_val) = opt_tbl.get::<_, LuaValue>("json") {
                if !matches!(json_val, LuaValue::Nil) {
                    if let Ok(v) = lua.from_value::<serde_json::Value>(json_val) { options.json = Some(v); }
                }
            }
        }
        let res = tokio::task::block_in_place(move || -> Result<String, String> {
            use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
            let client = Client::builder().timeout(StdDuration::from_secs(30)).build().map_err(|e| e.to_string())?;
            let m = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
            let mut req = client.request(m, &url);
            if !options.headers.is_empty() {
                let mut hm = HeaderMap::new();
                for (k, v) in options.headers {
                    if let (Ok(hn), Ok(hv)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(&v)) { hm.insert(hn, hv); }
                }
                req = req.headers(hm);
            }
            match options.body { BodyKind::Json(v) => { req = req.json(&v); } BodyKind::Text(s) => { req = req.body(s); } BodyKind::None => {} }
            if let Some(v) = options.json { req = req.json(&v); }
            let resp = req.send().map_err(|e| e.to_string())?;
            let resp = resp.error_for_status().map_err(|e| e.to_string())?;
            resp.text().map_err(|e| e.to_string())
        });
        match res { Ok(s) => Ok(s), Err(e) => Err(LuaError::external(e)) }
    })?;

    http_tbl.set("get", get)?;
    http_tbl.set("post", post)?;
    http_tbl.set("request", request)?;

    lua.globals().set("http", http_tbl)?;
    Ok(())
}

pub fn load_lua_tools_from_dir(dir: &str) -> Vec<LuaTool> {
    let mut tools = Vec::new();
    let path = Path::new(dir);
    if !path.exists() { return tools; }

    let entries = match fs::read_dir(path) { Ok(r) => r, Err(_) => return tools };

    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("lua") { continue; }

        match parse_lua_tool(&p) {
            Ok(t) => tools.push(t),
            Err(e) => { eprintln!("failed to load lua tool from {}: {}", p.display(), e); }
        }
    }

    tools
}

pub fn load_lua_tools_from_dirs(dirs: &[std::path::PathBuf]) -> Vec<LuaTool> {
    let mut out = Vec::new();
    for d in dirs {
        let path = Path::new(d);
        if !path.exists() { continue; }
        let Ok(entries) = fs::read_dir(path) else { continue; };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("lua") { continue; }
            match parse_lua_tool(&p) { Ok(t) => out.push(t), Err(e) => eprintln!("failed to load lua tool from {}: {}", p.display(), e) }
        }
    }
    out
}

fn parse_lua_tool(path: &Path) -> Result<LuaTool, String> {
    let code = fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let lua = Lua::new();
    install_http(&lua).map_err(|e| e.to_string())?;
    let value = lua
        .load(&code)
        .set_name(path.to_string_lossy().as_ref())
        .eval::<LuaValue>()
        .map_err(|e| format!("eval error: {e}"))?;

    let tbl = match value { LuaValue::Table(t) => t, _ => return Err("script must return a table".to_string()) };

    let name: String = tbl.get("name").map_err(|e| format!("missing name: {e}"))?;
    let description: String = tbl.get("description").unwrap_or_else(|_| "lua tool".to_string());
    let func_name: String = tbl.get("entry").unwrap_or_else(|_| "run".to_string());
    let _: Function = tbl.get(func_name.as_str()).map_err(|e| format!("missing entry function '{func_name}': {e}"))?;

    let args = parse_args_table(&tbl).map_err(|e| e.to_string())?;

    Ok(LuaTool { name, description, args, script_path: path.to_path_buf(), func_name })
}
