//! mp SDK for interacting with the mp blockchain
//!
//! This SDK provides a simple interface for interacting with a mp blockchain node.
//! It allows developers to submit transactions, query transaction status, and interact
//! with smart contracts running on the blockchain.
//!
//! ## Features
//!
//! * Procedural macros for simplified smart contract development
//! * Client for interacting with a mp node
//! * Local development and testing tools
//! * State management utilities

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, ItemFn};

pub mod client;
mod error;
pub mod module;
mod transaction;

pub use client::mpClient;
pub use error::Error;
pub use module::{ExecutionModule, Transaction as ModuleTransaction};
pub use transaction::{TransactionBuilder, TransactionStatus};

// Re-export common types
pub use mp_common::types::{Transaction, TransactionType};

/// Marks a struct as an execution module
///
/// This macro generates the necessary boilerplate code for a module that can be executed
/// on the mp blockchain.
///
/// # Example
///
/// ```rust
/// use mp_sdk::execution_module;
///
/// #[execution_module]
/// struct Counter {
///     value: u64
/// }
/// ```
// Commented out proc_macro_attribute since we're not using proc-macro crate type
// #[proc_macro_attribute]
pub fn execution_module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let output = quote! {
        #input

        impl ExecutionModule for #name {
            fn new() -> Self {
                Self::default()
            }
        }
    };

    output.into()
}

/// Marks a method as a transaction handler
///
/// This macro generates the necessary code to handle a transaction in an execution module.
///
/// # Example
///
/// ```rust
/// use mp_sdk::transaction;
///
/// impl Counter {
///     #[transaction]
///     fn increment(&mut self, amount: u64) -> Result<(), Error> {
///         self.value += amount;
///         Ok(())
///     }
/// }
/// ```
// Commented out proc_macro_attribute since we're not using proc-macro crate type
// #[proc_macro_attribute]
pub fn transaction(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let _block = &input.block;

    let output = quote! {
        #input

        impl ModuleTransaction for #name {
            fn execute(&self, module: &mut dyn ExecutionModule, args: &[u8]) -> Result<Vec<u8>, Error> {
                // Deserialize arguments
                let args: serde_json::Value = serde_json::from_slice(args)?;

                // Call the implementation
                let result = self.#name(module, args);

                // Serialize the result
                let result = serde_json::to_vec(&result)?;

                Ok(result)
            }
        }
    };

    output.into()
}
