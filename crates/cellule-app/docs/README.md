# Application guide

| Document | For |
| --- | --- |
| [Cellule API guide](../../../docs/api.md) | Public entry points, typed capabilities, receipts, and outcome handling. |
| [Topology](topology.md) | Define identity, partitioning, and descriptor stability. |
| [Invocations](invocation.md) | Send typed commands and receipt-bound queries. |
| [Examples and tests](examples.md) | Run the authoring and end-to-end paths. |
| [Crate entry](../README.md) | Minimal compiling declaration. |

Applications are statically linked Rust modules. They receive typed execution
capabilities, not raw authority, storage credentials, or network access.

The [detailed synthesis reference](../REFERENCE.md) preserves the original
technical explanations and examples. Read it with the current guides when
changing an existing behavior or persisted contract.
