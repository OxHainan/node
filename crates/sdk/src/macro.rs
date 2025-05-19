//! Procedural macros for simplified blockchain application development

/// Marks a module as an execution module
/// 
/// This macro automatically generates the necessary boilerplate code to handle
/// communication with the blockchain network, state management, and metadata generation.
/// 
/// # Example
/// 
/// ```rust
/// #[execution_module]
/// struct Counter {
///     value: u64
/// }
/// 
/// #[transaction]
/// impl Counter {
///     fn increment(&mut self, by: u64) -> Result<u64> {
///         self.value += by;
///         Ok(self.value)
///     }
/// }
/// ```
#[macro_export]
macro_rules! execution_module {
    ($struct_name:ident) => {
        impl $struct_name {
            /// Create a new instance of this module
            pub fn new() -> Self {
                Self::default()
            }
            
            /// Process an execution request from the blockchain
            pub fn process_request(
                &mut self,
                request: &serde_json::Value,
            ) -> Result<serde_json::Value, String> {
                let handler = request
                    .get("handler")
                    .and_then(|h| h.as_str())
                    .ok_or_else(|| "Missing handler".to_string())?;
                
                let params = request
                    .get("params")
                    .cloned()
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                
                match handler {
                    // Handler methods will be added by the transaction macro
                    _ => Err(format!("Unknown handler: {}", handler)),
                }
            }
        }
    };
}

/// Marks a method as a transaction handler
/// 
/// This macro automatically generates code to:
/// 1. Deserialize parameters from JSON
/// 2. Execute the handler method
/// 3. Serialize the result to JSON
/// 4. Record state changes for blockchain synchronization
/// 
/// # Example
/// 
/// ```rust
/// #[transaction]
/// fn process_payment(&mut self, sender: String, amount: u64) -> Result<PaymentReceipt> {
///     // Implementation...
/// }
/// ```
#[macro_export]
macro_rules! transaction {
    (
        fn $method_name:ident(&mut $self:ident, $($param_name:ident: $param_type:ty),*) -> $result_type:ty $body:block
    ) => {
        fn $method_name(&mut $self, $($param_name: $param_type),*) -> $result_type $body
        
        // Add handler to match in process_request
        #[allow(unreachable_patterns)]
        match handler {
            stringify!($method_name) => {
                $(
                    let $param_name = params
                        .get(stringify!($param_name))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| format!("Missing or invalid parameter: {}", stringify!($param_name)))?;
                )*
                
                match $self.$method_name($($param_name),*) {
                    Ok(result) => Ok(serde_json::to_value(result).unwrap_or_default()),
                    Err(e) => Err(format!("Execution error: {:?}", e)),
                }
            }
        }
    };
}
