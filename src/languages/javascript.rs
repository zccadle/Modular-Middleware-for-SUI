use anyhow::{Result, anyhow};
use boa_engine::{Context, Source, JsValue, property::Attribute};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JavaScriptExecutionResult {
    pub success: bool,
    pub output: Value,
    pub error: Option<String>,
}

pub struct JavaScriptExecutor;

impl JavaScriptExecutor {
    pub fn execute(code: &str, params: Option<Value>) -> Result<JavaScriptExecutionResult> {
        // Create execution context
        let mut context = Context::default();
        
        // Add parameters to context if provided
        if let Some(param_value) = params {
            let params_json = serde_json::to_string(&param_value)?;
            let params_js = format!("const params = {};\n{}", params_json, code);
            
            match context.eval(Source::from_bytes(&params_js)) {
                Ok(value) => Self::process_result(value),
                Err(e) => {
                    Ok(JavaScriptExecutionResult {
                        success: false,
                        output: Value::Null,
                        error: Some(e.to_string()),
                    })
                }
            }
        } else {
            // Execute without params
            match context.eval(Source::from_bytes(code)) {
                Ok(value) => Self::process_result(value),
                Err(e) => {
                    Ok(JavaScriptExecutionResult {
                        success: false,
                        output: Value::Null,
                        error: Some(e.to_string()),
                    })
                }
            }
        }
    }
    
    fn process_result(js_value: JsValue) -> Result<JavaScriptExecutionResult> {
        // Convert Boa JsValue to serde_json::Value
        let result = match js_value {
            JsValue::Null => Value::Null,
            JsValue::Undefined => Value::Null,
            JsValue::Boolean(b) => Value::Bool(b),
            JsValue::Integer(i) => Value::Number(i.into()),
            JsValue::Rational(f) => {
                if f.is_finite() {
                    // Fix: Using match instead of unwrap_or_default
                    match serde_json::Number::from_f64(f) {
                        Some(num) => Value::Number(num),
                        None => Value::Null
                    }
                } else {
                    Value::Null
                }
            },
            JsValue::String(s) => Value::String(s.to_std_string_escaped()),
            JsValue::Object(o) => {
                // Try to convert to a JSON string using JSON.stringify in JavaScript
                let mut context = Context::default();
                context.register_global_property(
                    "obj",
                    o.clone(),
                    Attribute::all()
                ).map_err(|e| anyhow!("Failed to register object: {}", e))?;
                
                let json_str = context.eval(Source::from_bytes("JSON.stringify(obj)"))
                    .map_err(|e| anyhow!("Failed to stringify: {}", e))?;
                
                if let JsValue::String(s) = json_str {
                    let json_string = s.to_std_string_escaped();
                    serde_json::from_str(&json_string)?
                } else {
                    Value::Null
                }
            },
            _ => Value::Null,
        };
        
        Ok(JavaScriptExecutionResult {
            success: true,
            output: result,
            error: None,
        })
    }
}