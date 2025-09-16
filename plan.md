# RMCP server conversions

* We are converting mcp servers in the goose-mcp crate to use the rmcp crate instead of the internal mcp server one
* The developer server has already been converted - read crates/goose-mcp/src/developer/rmcp_developer.rs
* Now help me convert crates/goose-mcp/src/autovisualizer in a similar way but I want to do one thing differently: do not make a separate rmcp_ file. just leave it in mod.rs
* Make sure to preserve all test cases
* Make sure to preserve all input schemas
