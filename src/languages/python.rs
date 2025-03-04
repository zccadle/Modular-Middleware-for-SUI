use anyhow::{Result, anyhow};
use pyo3::{prelude::*, types::{PyDict, PyTuple}};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Execution results from Python code
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PythonExecutionResult {
    pub success: bool,
    pub output: Value,
    pub error: Option<String>,
}

pub struct PythonExecutor;

impl PythonExecutor {
    /// Execute Python code with parameters
    pub fn execute(code: &str, params: Option<Value>) -> Result<PythonExecutionResult> {
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            
            // Add parameters to Python context if provided
            if let Some(param_value) = params {
                // Convert params to Python dict or value
                let param_obj = match param_value {
                    Value::Object(map) => {
                        let dict = PyDict::new(py);
                        for (key, value) in map {
                            dict.set_item(key, Self::value_to_py_object(py, &value)?)?;
                        }
                        dict.into()
                    },
                    Value::Array(arr) => {
                        let list = PyTuple::new(
                            py, 
                            arr.iter().map(|v| Self::value_to_py_object(py, v))
                               .collect::<Result<Vec<_>, _>>()?
                        );
                        list.into()
                    },
                    _ => Self::value_to_py_object(py, &param_value)?,
                };
                
                locals.set_item("params", param_obj)?;
            }
            
            // Execute the code
            match py.run(code, None, Some(locals)) {
                Ok(_) => {
                    // Check for return value in locals
                    // Fix: Handle Option correctly instead of Result
                    if let Some(result) = locals.get_item("result") {
                        // Convert Python object back to Rust
                        if let Ok(value) = Self::py_object_to_value(py, result.into()) {
                            Ok(PythonExecutionResult {
                                success: true,
                                output: value,
                                error: None,
                            })
                        } else {
                            // If we can't convert to JSON, stringify the result
                            match result.str() {
                                Ok(str_result) => Ok(PythonExecutionResult {
                                    success: true,
                                    output: Value::String(str_result.to_string()),
                                    error: None,
                                }),
                                Err(_) => Ok(PythonExecutionResult {
                                    success: true,
                                    output: Value::String("[Non-serializable Python object]".to_string()),
                                    error: None,
                                }),
                            }
                        }
                    } else {
                        // No result defined
                        Ok(PythonExecutionResult {
                            success: true,
                            output: Value::Null,
                            error: None,
                        })
                    }
                },
                Err(e) => {
                    Ok(PythonExecutionResult {
                        success: false,
                        output: Value::Null,
                        error: Some(e.to_string()),
                    })
                }
            }
        })
    }
    
    // Helper function to convert a JSON value to a Python object
    fn value_to_py_object(py: Python, value: &Value) -> Result<PyObject> {
        match value {
            Value::Null => Ok(py.None()),
            Value::Bool(b) => Ok(b.to_object(py)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(i.to_object(py))
                } else if let Some(f) = n.as_f64() {
                    Ok(f.to_object(py))
                } else {
                    Err(anyhow!("Unsupported number type"))
                }
            },
            Value::String(s) => Ok(s.to_object(py)),
            Value::Array(arr) => {
                let list = PyTuple::new(
                    py,
                    arr.iter()
                        .map(|v| Self::value_to_py_object(py, v))
                        .collect::<Result<Vec<_>, _>>()?
                );
                Ok(list.into())
            },
            Value::Object(map) => {
                let dict = PyDict::new(py);
                for (key, value) in map {
                    dict.set_item(key, Self::value_to_py_object(py, value)?)?;
                }
                Ok(dict.into())
            }
        }
    }
    
    // Helper function to convert a Python object to a JSON value
    fn py_object_to_value(py: Python, obj: PyObject) -> Result<Value> {
        if obj.is_none(py) {
            return Ok(Value::Null);
        }
        
        if let Ok(b) = obj.extract::<bool>(py) {
            return Ok(Value::Bool(b));
        }
        
        if let Ok(i) = obj.extract::<i64>(py) {
            return Ok(Value::Number(i.into()));
        }
        
        if let Ok(f) = obj.extract::<f64>(py) {
            // Check if it's a valid float (not NaN or Infinity)
            if f.is_finite() {
                return Ok(Value::Number(serde_json::Number::from_f64(f).unwrap()));
            } else {
                return Err(anyhow!("Non-finite float value"));
            }
        }
        
        if let Ok(s) = obj.extract::<String>(py) {
            return Ok(Value::String(s));
        }
        
        if let Ok(list) = obj.extract::<Vec<PyObject>>(py) {
            let mut values = Vec::new();
            for item in list {
                values.push(Self::py_object_to_value(py, item)?);
            }
            return Ok(Value::Array(values));
        }
        
        if let Ok(dict) = obj.extract::<HashMap<String, PyObject>>(py) {
            let mut map = serde_json::Map::new();
            for (key, value) in dict {
                map.insert(key, Self::py_object_to_value(py, value)?);
            }
            return Ok(Value::Object(map));
        }
        
        // If we can't convert it directly, try to use str() on it
        let str_result = obj.call_method0(py, "__str__")?;
        if let Ok(s) = str_result.extract::<String>(py) {
            Ok(Value::String(s))
        } else {
            Err(anyhow!("Could not convert Python object to JSON"))
        }
    }
}