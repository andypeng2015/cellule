# Store guide

| Document | Answers |
| --- | --- |
| [Transport contract](transport.md) | What read/write guarantees does `Store` provide? |
| [Provider setup](providers.md) | Who creates providers and scopes? |
| [Crate entry](../README.md) | Where does storage sit in Cellule? |

The store reports conflicts and transport failures. It never turns a provider
write into Cell authority.

The [detailed synthesis reference](../REFERENCE.md) preserves the original
technical explanations and examples. Read it with the current guides when
changing an existing behavior or persisted contract.
